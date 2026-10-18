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
use crate::engine::jev::guard::evaluate::{GuardObservation, evaluate_guard_observed};
use crate::engine::jev::guard::{GuardDecision, ProposedChange};
use crate::server::TrueNorthServer;

#[path = "guard_accounting.rs"]
mod accounting;
#[cfg(all(test, feature = "jev-http"))]
pub(crate) use accounting::AccountedClient;
pub use accounting::GuardAccounting;

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

        let (decision, accounting, observation) = self.evaluate_change(&change).await?;
        decision_to_result(decision, self.ctx.token_caps, accounting, observation)
    }
}

impl TrueNorthServer {
    /// Resolve the config and the protected paths, then run the guardrail decision.
    ///
    /// The config and protected paths come from `.agent/config/rules.yml`. The production
    /// server validates Jev activation before advertising this tool.
    async fn evaluate_change(
        &self,
        change: &ProposedChange,
    ) -> Result<(GuardDecision, GuardAccounting, GuardObservation), ErrorData> {
        let config = resolve(&self.ctx.repo_root).map_err(config_error)?;
        let protected = resolve_protected_paths(&self.ctx.repo_root).map_err(config_error)?;

        run_accounted_decision(
            change,
            &protected,
            &config,
            self.ctx.features.jev,
            self.ctx.jev_activation.as_ref(),
            &self.ctx.repo_root,
        )
        .await
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
    }
}

/// Map a config error to an MCP internal error naming the cause.
fn config_error(error: crate::engine::jev::JevError) -> ErrorData {
    ErrorData::internal_error(
        format!("could not resolve the guardrail config: {error}"),
        None,
    )
}

/// Run the guardrail with sanitized per-call accounting; the sole decision entry point for
/// both the MCP tool and the stdin CLI (R8.4).
pub async fn run_accounted_decision(
    change: &ProposedChange,
    protected: &[String],
    config: &crate::engine::jev::config::JevConfig,
    jev_enabled: bool,
    activation: Option<&crate::engine::jev::client_http::JevActivation>,
    repo_root: &std::path::Path,
) -> Result<(GuardDecision, GuardAccounting, GuardObservation), crate::engine::jev::JevError> {
    let started = std::time::Instant::now();
    #[cfg(feature = "jev-http")]
    if jev_enabled && let Some(activation) = activation {
        let client = accounting::AccountedClient::new(
            crate::engine::jev::client_http::HttpClient::new(activation.clone(), config),
        );
        let (decision, observation) = evaluate_guard_observed(
            change,
            protected,
            config,
            true,
            true,
            Some(&client),
            repo_root,
        )
        .await?;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        return Ok((decision, client.into_accounting(elapsed_ms), observation));
    }
    let _ = activation;
    let (decision, observation) =
        evaluate_guard_observed::<crate::engine::jev::client_fake::FakeClient>(
            change,
            protected,
            config,
            jev_enabled,
            false,
            None,
            repo_root,
        )
        .await?;
    Ok((
        decision,
        GuardAccounting {
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            ..GuardAccounting::default()
        },
        observation,
    ))
}

/// Map a guard decision to a tool result (R1.4).
///
/// An `Allow` and an `Annotate` are success results carrying the notes. A `Block` is an MCP
/// error carrying the neutralization packet as structured data. The packet carries no secret
/// value and no API key (R6.3).
pub(crate) fn decision_to_result(
    decision: GuardDecision,
    caps: crate::engine::features::TokenCaps,
    accounting: GuardAccounting,
    observation: GuardObservation,
) -> Result<CallToolResult, ErrorData> {
    let findings = accounting::findings(&observation);
    let response = match decision {
        GuardDecision::Allow { notes } => result::success(
            vec![ContentBlock::text(
                serde_json::json!({ "decision": "allow", "notes": notes, "accounting": accounting, "findings": findings }).to_string(),
            )],
            caps,
        ),
        GuardDecision::Annotate { notes } => result::success(
            vec![ContentBlock::text(
                serde_json::json!({ "decision": "annotate", "notes": notes, "accounting": accounting, "findings": findings }).to_string(),
            )],
            caps,
        ),
        GuardDecision::Block(packet) => {
            let mut data = serde_json::to_value(&packet).unwrap_or(serde_json::Value::Null);
            data["accounting"] = serde_json::json!(accounting);
            data["findings"] = findings;
            return Err(ErrorData::invalid_request(
                format!("the guardrail blocked the change: {}", packet.remediation),
                Some(data),
            ))
        }
    };
    response.map_err(|mut error| {
        if error.data.is_none() {
            error.data =
                Some(serde_json::json!({ "accounting": accounting, "findings": findings }));
        }
        error
    })
}

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `guard`.
#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
