//! Gate tool: `truenorth_verify_gate` (ADR-1).
//!
//! In the default execute mode, the tool runs the project's configured verify or test
//! command in the sandbox and passes only on a real exit-0 observation (Requirement 3.1,
//! Property 2). In evidence mode, it skips execution and requires the caller to supply
//! `test_evidence`, rejecting the call when it is absent (Requirements 3.7, 3.8).
//!
//! The verify command and the command allowlist come from the environment, so an operator
//! configures them per project without a rebuild. The command sits behind
//! `engine::gate_runner`, so the sandbox enforces the timeout, the working directory, the
//! allowlist, and the environment sanitization.
//!
//! Requirements: 3.1, 3.7, 3.8. Design: Part II §2, §5.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::config::SandboxConfig;
use crate::engine::gate_runner::{GateOutcome, SystemCommandRunner, run_gate};
use crate::server::TrueNorthServer;

/// The environment variable that holds the project verify or test command.
const VERIFY_CMD_ENV: &str = "TRUENORTH_VERIFY_CMD";
/// The environment variable that extends the gate command allowlist (comma-separated).
const ALLOWLIST_ENV: &str = "TRUENORTH_GATE_ALLOWLIST";

/// The `truenorth_verify_gate` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VerifyGateArgs {
    /// The lifecycle phase the gate runs for.
    pub phase: String,
    /// The test evidence. Required in evidence mode, ignored in execute mode.
    #[serde(default)]
    pub test_evidence: Option<String>,
    /// The gate mode: `execute` (default) or `evidence`.
    #[serde(default)]
    pub mode: Option<String>,
}

/// The resolved gate mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateMode {
    /// Run the verify command in the sandbox.
    Execute,
    /// Skip execution and accept caller-supplied evidence.
    Evidence,
}

#[tool_router(router = gates_router, vis = "pub")]
impl TrueNorthServer {
    /// Run the phase verify gate, or accept evidence in evidence mode.
    #[tool(
        description = "Run the project verify/test command in a sandbox (execute mode), or accept test evidence (evidence mode)."
    )]
    pub async fn truenorth_verify_gate(
        &self,
        params: Parameters<VerifyGateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;
        let mode = parse_mode(args.mode.as_deref())?;

        match mode {
            GateMode::Evidence => {
                // Requirement 3.8: evidence mode requires test_evidence.
                let evidence = args.test_evidence.as_deref().unwrap_or("").trim();
                if evidence.is_empty() {
                    return Err(ErrorData::invalid_params(
                        "`test_evidence` is required in evidence mode".to_string(),
                        None,
                    ));
                }
                // The operator vouches for the phase's quality bar with the evidence.
                Ok(CallToolResult::success(vec![ContentBlock::text(
                    serde_json::json!({ "passed": true, "phase": args.phase, "mode": "evidence" })
                        .to_string(),
                )]))
            }
            GateMode::Execute => {
                // Requirement 3.1: run the configured verify command in the sandbox.
                let command = verify_command().ok_or_else(|| {
                    ErrorData::invalid_params(
                        format!(
                            "no verify command configured. Set the `{VERIFY_CMD_ENV}` \
                             environment variable to the project's verify or test command, \
                             or call with mode=evidence and supply test_evidence."
                        ),
                        None,
                    )
                })?;

                let cfg = SandboxConfig::new(self.ctx.repo_root.clone(), allowlist(&command));
                let outcome = run_gate(&command, &cfg, &SystemCommandRunner);
                gate_result(&outcome, &args.phase)
            }
        }
    }
}

/// Parse the gate mode, defaulting to execute (Requirement 3.1).
///
/// # Errors
///
/// Returns an invalid-params error for an unrecognized mode.
fn parse_mode(mode: Option<&str>) -> Result<GateMode, ErrorData> {
    match mode.unwrap_or("execute") {
        "execute" => Ok(GateMode::Execute),
        "evidence" => Ok(GateMode::Evidence),
        other => Err(ErrorData::invalid_params(
            format!("unrecognized gate mode `{other}`. Use execute or evidence."),
            None,
        )),
    }
}

/// The configured verify command, when set and non-empty.
fn verify_command() -> Option<String> {
    match std::env::var(VERIFY_CMD_ENV) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
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
fn gate_result(outcome: &GateOutcome, phase: &str) -> Result<CallToolResult, ErrorData> {
    if outcome.passed {
        return Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::json!({ "passed": true, "phase": phase, "mode": "execute" }).to_string(),
        )]));
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
