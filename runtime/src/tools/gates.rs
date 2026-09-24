//! Gate tool: `truenorth_verify_gate` (ADR-1).
//!
//! The tool runs the project's configured verify or test command in the sandbox and
//! passes only on a real exit-0 observation (Requirement 3.1, Property 2).
//!
//! The verify command and the command allowlist come from the environment, so an operator
//! configures them per project without a rebuild. The command sits behind
//! `engine::gate_runner`, so the sandbox enforces the timeout, the working directory, the
//! allowlist, and the environment sanitization.
//!
//! Requirements: 3.1. Design: Part II §2, §5.

use crate::tools::result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::config::{SandboxConfig, VERIFY_CMD_ENV, verify_command};
use crate::engine::gate_runner::{GateOutcome, SystemCommandRunner, run_gate};
use crate::server::TrueNorthServer;

/// The environment variable that extends the gate command allowlist (comma-separated).
const ALLOWLIST_ENV: &str = "TRUENORTH_GATE_ALLOWLIST";

/// The `truenorth_verify_gate` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyGateArgs {
    /// The lifecycle phase the gate runs for.
    pub phase: String,
}

#[tool_router(router = gates_router, vis = "pub")]
impl TrueNorthServer {
    /// Run the configured project verify gate in its sandbox.
    #[tool(description = "Run the configured project verify/test command in a sandbox.")]
    pub async fn truenorth_verify_gate(
        &self,
        params: Parameters<VerifyGateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Requirement 3.1: run the configured verify command in the sandbox.
        let command = verify_command().ok_or_else(|| {
            ErrorData::invalid_params(
                format!(
                    "no verify command configured. Set the `{VERIFY_CMD_ENV}` \
                     environment variable to the project's verify or test command."
                ),
                None,
            )
        })?;

        let cfg = SandboxConfig::new(self.ctx.repo_root.clone(), allowlist(&command));
        let outcome = run_gate(&command, &cfg, &SystemCommandRunner);
        gate_result(&outcome, &args.phase, self.ctx.token_caps)
    }
}

/// The command allowlist for a verify command.
///
/// The command's own first token is always allowed, since the operator authorized it by
/// configuring the command. Extra binaries come from the `TRUENORTH_GATE_ALLOWLIST`
/// environment variable, comma-separated.
fn allowlist(command: &str) -> Vec<String> {
    let mut allow: Vec<String> = command
        .split_whitespace()
        .next()
        .map(|token| vec![token.to_string()])
        .unwrap_or_default();

    if let Ok(extra) = std::env::var(ALLOWLIST_ENV) {
        for binary in extra.split(',') {
            let trimmed = binary.trim();
            if !trimmed.is_empty() {
                allow.push(trimmed.to_string());
            }
        }
    }
    allow
}

/// Render a gate outcome as a tool result. A pass is a success result; a failure is an
/// MCP error carrying the message and remediation hints (Property 2).
fn gate_result(
    outcome: &GateOutcome,
    phase: &str,
    caps: crate::engine::features::TokenCaps,
) -> Result<CallToolResult, ErrorData> {
    if outcome.passed {
        return result::success(
            vec![ContentBlock::text(
                serde_json::json!({ "passed": true, "phase": phase, "mode": "execute" })
                    .to_string(),
            )],
            caps,
        );
    }
    let message = outcome
        .error
        .clone()
        .unwrap_or_else(|| "the gate failed".to_string());
    let data = serde_json::json!({ "remediation_hints": outcome.remediation_hints });
    Err(ErrorData::invalid_request(message, Some(data)))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `gates`.
#[cfg(test)]
#[path = "gates_tests.rs"]
mod tests;
