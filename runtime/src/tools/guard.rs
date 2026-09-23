//! The active guardrail tool: `truenorth_guard_change` (jev-active-guardrail R1).
//!
//! An agent calls this tool with a proposed change before it writes. The tool runs the
//! guardrail decision function and maps the decision to a tool result: an `Allow` or an
//! `Annotate` is a success result carrying the notes, and a `Block` is an MCP error carrying
//! the neutralization packet (R1.4). A malformed input is a typed invalid-params error naming
//! the offending field, with no check run (R1.5).
//!
//! The tool resolves the protected paths and the config from `.agent/config/rules.yml`, reads
//! the `jev` feature flag from the server context, and reads the API key from the environment.
//! Under the default build it runs the deterministic layer and the fail-open probabilistic
//! layer with no client, so a protected-path or a secret block still holds offline. Under the
//! `jev-http` feature, and only when the flag is on and the key is present, it builds the real
//! client and scores the change (R8.4).
//!
//! Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 8.2, 8.4.

use crate::tools::result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::jev::config::{resolve, resolve_protected_paths};
use crate::engine::jev::guard::evaluate::evaluate_guard;
use crate::engine::jev::guard::{GuardDecision, ProposedChange};
use crate::server::TrueNorthServer;

/// The maximum number of target paths a single proposed change may carry.
const MAX_PATHS: usize = 100;

/// The maximum content length, in bytes, a single proposed change may carry.
const MAX_CONTENT_BYTES: usize = 1_000_000;

/// The `truenorth_guard_change` input contract (R1.1, R1.2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GuardChangeArgs {
    /// The target paths the change writes (at least one, at most 100).
    pub paths: Vec<String>,
    /// The new content the change writes (at most 1,000,000 bytes).
    pub content: String,
    /// The task the change serves, as a short description (optional). When present, the drift
    /// scope check judges whether the change stays within this task. When absent, the check
    /// falls back to the cockpit active task, then to a protected-path-only judgment.
    #[serde(default)]
    pub task: Option<String>,
}

#[tool_router(router = guard_router, vis = "pub")]
impl TrueNorthServer {
    /// Evaluate a proposed change and return allow, annotate, or block.
    #[tool(
        description = "Evaluate a proposed change before a write. Returns allow or annotate as a \
                       success result, or blocks with a structured reason. Runs a deterministic \
                       protected-path and secret check always, and a Jev-scored check when the \
                       jev feature is on."
    )]
    pub async fn truenorth_guard_change(
        &self,
        params: Parameters<GuardChangeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate the input before any check runs (R1.5).
        if args.paths.is_empty() {
            return Err(ErrorData::invalid_params(
                "`paths` must contain at least one entry".to_string(),
                None,
            ));
        }
        if args.paths.len() > MAX_PATHS {
            return Err(ErrorData::invalid_params(
                format!("`paths` must contain at most {MAX_PATHS} entries"),
                None,
            ));
        }
        if let Some(pos) = args.paths.iter().position(|p| p.is_empty()) {
            return Err(ErrorData::invalid_params(
                format!("`paths[{pos}]` must not be empty"),
                None,
            ));
        }
        if args.content.len() > MAX_CONTENT_BYTES {
            return Err(ErrorData::invalid_params(
                format!("`content` must be at most {MAX_CONTENT_BYTES} bytes"),
                None,
            ));
        }

        let change = ProposedChange {
            paths: args.paths,
            content: args.content,
            task: args.task,
        };

        let decision = self.evaluate_change(&change).await?;
        decision_to_result(decision, self.ctx.token_caps)
    }
}

impl TrueNorthServer {
    /// Resolve the config and the protected paths, then run the guardrail decision.
    ///
    /// The config and the protected paths come from `.agent/config/rules.yml`. A config read or
    /// parse failure is an MCP internal error, because the guardrail cannot decide without the
    /// thresholds. The feature flag comes from the server context. The client and the key are
    /// resolved by the build-specific `run_decision` helper.
    async fn evaluate_change(&self, change: &ProposedChange) -> Result<GuardDecision, ErrorData> {
        let config = resolve(&self.ctx.repo_root).map_err(config_error)?;
        let protected = resolve_protected_paths(&self.ctx.repo_root).map_err(config_error)?;
        let jev_enabled = self.ctx.features.jev;

        Ok(run_decision(
            change,
            &protected,
            &config,
            jev_enabled,
            &self.ctx.repo_root,
        )
        .await)
    }
}

/// Map a config error to an MCP internal error naming the cause.
fn config_error(error: crate::engine::jev::JevError) -> ErrorData {
    ErrorData::internal_error(
        format!("could not resolve the guardrail config: {error}"),
        None,
    )
}

/// Run the guardrail decision with the resolved inputs (default build: no client).
///
/// Under the default build there is no HTTP client, so the guardrail runs the deterministic
/// layer and the fail-open probabilistic layer with no client. A deterministic block still
/// holds; the probabilistic layer returns an allow with a note (R8.4).
#[cfg(not(feature = "jev-http"))]
async fn run_decision(
    change: &ProposedChange,
    protected: &[String],
    config: &crate::engine::jev::config::JevConfig,
    jev_enabled: bool,
    repo_root: &std::path::Path,
) -> GuardDecision {
    use crate::engine::jev::client_fake::FakeClient;

    // The default build compiles no HTTP client, so the type parameter is the fake and the
    // argument is None. The probabilistic layer sees no client and fails open (R8.4).
    evaluate_guard::<FakeClient>(
        change,
        protected,
        config,
        jev_enabled,
        false,
        None,
        repo_root,
    )
    .await
}

/// Run the guardrail decision with the resolved inputs (jev-http build: real client).
///
/// The client is built only when the flag is on and the key is present, so a flag-off or a
/// keyless project makes no network call. The key presence is read once here and passed to
/// `evaluate_guard`, so the decision function holds no environment read.
#[cfg(feature = "jev-http")]
async fn run_decision(
    change: &ProposedChange,
    protected: &[String],
    config: &crate::engine::jev::config::JevConfig,
    jev_enabled: bool,
    repo_root: &std::path::Path,
) -> GuardDecision {
    use crate::engine::jev::client_fake::FakeClient;
    use crate::engine::jev::client_http::HttpClient;
    use crate::engine::jev::client_http::JEV_API_KEY_VAR;

    /// The environment variable that holds the Jev endpoint URL for the tool's client.
    const JEV_ENDPOINT_VAR: &str = "TRUENORTH_JEV_ENDPOINT";

    let api_key_present = std::env::var(JEV_API_KEY_VAR)
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    // Build the client only when the flag is on and the key is present, so a flag-off or a
    // keyless project makes no network call and needs no endpoint.
    if jev_enabled && api_key_present {
        let endpoint = std::env::var(JEV_ENDPOINT_VAR).unwrap_or_default();
        let client = HttpClient::new(endpoint, config);
        return evaluate_guard(
            change,
            protected,
            config,
            jev_enabled,
            api_key_present,
            Some(&client),
            repo_root,
        )
        .await;
    }

    // Flag off or key absent: no client, so the probabilistic layer fails open with a note.
    evaluate_guard::<FakeClient>(
        change,
        protected,
        config,
        jev_enabled,
        api_key_present,
        None,
        repo_root,
    )
    .await
}

/// Map a guard decision to a tool result (R1.4).
///
/// An `Allow` and an `Annotate` are success results carrying the notes. A `Block` is an MCP
/// error carrying the neutralization packet as structured data. The packet carries no secret
/// value and no API key (R6.3).
fn decision_to_result(
    decision: GuardDecision,
    caps: crate::engine::features::TokenCaps,
) -> Result<CallToolResult, ErrorData> {
    match decision {
        GuardDecision::Allow { notes } => result::success(
            vec![ContentBlock::text(
                serde_json::json!({ "decision": "allow", "notes": notes }).to_string(),
            )],
            caps,
        ),
        GuardDecision::Annotate { notes } => result::success(
            vec![ContentBlock::text(
                serde_json::json!({ "decision": "annotate", "notes": notes }).to_string(),
            )],
            caps,
        ),
        GuardDecision::Block(packet) => {
            let data = serde_json::to_value(&packet).unwrap_or(serde_json::Value::Null);
            Err(ErrorData::invalid_request(
                format!("the guardrail blocked the change: {}", packet.remediation),
                Some(data),
            ))
        }
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `guard`.
#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
