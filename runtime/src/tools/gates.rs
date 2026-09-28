//! Gate tool: `truenorth_verify_gate` (ADR-1).
//!
//! The tool runs the project's configured verify or test command through the bounded gate
//! executor and passes only on a real exit-0 observation (Requirement 3.1, Property 2).
//!
//! The verify command, command allowlist, and environment-name allowlist come from operator
//! configuration. The executor enforces the timeout, working directory, process-group kill,
//! first-token command guardrail, and exact-name environment inheritance.
//!
//! Requirements: 3.1. Design: Part II §2, §5.

use crate::tools::result;
use rmcp::handler::server::{tool::RequestId, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::config::gate_environment::{GATE_ENV_ALLOWLIST_ENV, GateEnvironmentPolicy};
use crate::config::{GateExecutionConfig, VERIFY_CMD_ENV, verify_command};
use crate::engine::gate_runner::{GateOutcome, GateOutcomeStatus, SystemCommandRunner, run_gate};
use crate::server::TrueNorthServer;
use crate::tools::receipt::{self, GateReceipt, GateStatus, OperationReceipt};

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
    /// Run the configured project verify gate through the bounded executor.
    #[tool(
        description = "Run the configured project verify/test command through the bounded gate executor."
    )]
    pub async fn truenorth_verify_gate(
        &self,
        params: Parameters<VerifyGateArgs>,
        request_id: RequestId,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Requirement 3.1: run the configured verify command through the bounded executor.
        let command = verify_command().ok_or_else(|| {
            ErrorData::invalid_params(
                format!(
                    "no verify command configured. Set the `{VERIFY_CMD_ENV}` \
                     environment variable to the project's verify or test command."
                ),
                None,
            )
        })?;
        let environment = GateEnvironmentPolicy::from_process().map_err(|error| {
            ErrorData::invalid_request(
                format!(
                    "invalid bounded gate environment configuration: {error}. Remove credential-like names or configure safe names in `{GATE_ENV_ALLOWLIST_ENV}`."
                ),
                None,
            )
        })?;
        let cfg = GateExecutionConfig::new(
            self.ctx.repo_root.clone(),
            allowlist(&command),
            environment.allowed_names().to_vec(),
        );
        let outcome = run_gate(&command, &cfg, &SystemCommandRunner);
        gate_result(&outcome, &args.phase, &request_id, self.ctx.token_caps)
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
    request_id: &RequestId,
    caps: crate::engine::features::TokenCaps,
) -> Result<CallToolResult, ErrorData> {
    let status = match outcome.status {
        GateOutcomeStatus::Pass => GateStatus::Pass,
        GateOutcomeStatus::Failure => GateStatus::Failure,
        GateOutcomeStatus::Timeout => GateStatus::Timeout,
    };
    let gate = GateReceipt {
        status,
        expectation: receipt::GateExpectation::Zero,
        duration_ms: outcome.duration_ms,
        exit_code: outcome.exit_code,
    };
    let receipt =
        OperationReceipt::new("truenorth_verify_gate", request_id, Vec::new(), Some(gate))
            .map_err(receipt_error)?;

    if outcome.passed() {
        return result::success(
            vec![ContentBlock::text(
                receipt::attach(
                    serde_json::json!({ "passed": true, "phase": phase, "mode": "execute" }),
                    receipt,
                )
                .to_string(),
            )],
            caps,
        );
    }
    let message = outcome
        .error
        .clone()
        .unwrap_or_else(|| "the gate failed".to_string());
    let data = serde_json::json!({
        "remediation_hints": outcome.remediation_hints,
        "receipt": receipt,
    });
    Err(ErrorData::invalid_request(message, Some(data)))
}

fn receipt_error(error: String) -> ErrorData {
    ErrorData::internal_error(format!("could not build gate receipt: {error}"), None)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `gates`.
#[cfg(test)]
#[path = "gates_tests.rs"]
mod tests;
