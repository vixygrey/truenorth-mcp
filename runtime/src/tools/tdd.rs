//! TDD tool: `truenorth_tdd_cycle`.
//!
//! The tool enforces the Red-Green-Refactor step order (Requirement 2.7). An out-of-order
//! step is rejected and the recorded step is left unchanged (Requirement 2.8). The red
//! step runs the failing test command: a non-zero exit reports red passed (Requirement
//! 2.9), and an exit code 0 reports red failed with a "test did not fail as required"
//! error (Requirement 2.10).
//!
//! Requirements: 2.1, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11. Design: Part II §2.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::config::SandboxConfig;
use crate::engine::cockpit::{self, CockpitError};
use crate::engine::gate_runner::{CommandRunner, SystemCommandRunner};
use crate::engine::tdd::{TddStep, next_step};
use crate::server::TrueNorthServer;

/// The maximum length of the failing test command (Requirement 2.6).
const MAX_FAILING_TEST_CMD: usize = 1000;
/// The maximum number of files to modify (Requirement 2.6).
const MAX_FILES_TO_MODIFY: usize = 100;

/// The `truenorth_tdd_cycle` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TddCycleArgs {
    /// The cycle step: `red`, `green`, or `refactor`.
    pub step: String,
    /// The failing test command (1 to 1000 chars).
    pub failing_test_cmd: String,
    /// The files to modify this step (1 to 100 non-empty entries).
    pub files_to_modify: Vec<String>,
}

#[tool_router(router = tdd_router, vis = "pub")]
impl TrueNorthServer {
    /// Drive one Red, Green, or Refactor step of the TDD cycle.
    #[tool(description = "Enforce the Red-Green-Refactor step order and check the red-stage test.")]
    pub async fn truenorth_tdd_cycle(
        &self,
        params: Parameters<TddCycleArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate every field before any state read or mutation (Requirement 2.11).
        let requested = TddStep::parse(&args.step).ok_or_else(|| {
            ErrorData::invalid_params(
                format!(
                    "`step` must be red, green, or refactor, but it was `{}`",
                    args.step
                ),
                None,
            )
        })?;
        check_failing_test_cmd(&args.failing_test_cmd)?;
        check_files_to_modify(&args.files_to_modify)?;

        // Enforce ordering. An invalid transition leaves the recorded step unchanged,
        // because no write happens (Requirement 2.8).
        let current = cockpit::read_tdd_step(&self.ctx.repo_root).map_err(cockpit_error)?;
        let step = next_step(current, requested)
            .map_err(|error| ErrorData::invalid_request(error.to_string(), None))?;

        // The red step runs the failing test command and checks its exit code.
        if step == TddStep::Red {
            self.run_red_stage(&args.failing_test_cmd)?;
        }

        // Record the step only after the checks pass.
        cockpit::write_tdd_step(&self.ctx.repo_root, step).map_err(cockpit_error)?;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::json!({ "step": step.as_str(), "step_ok": true }).to_string(),
        )]))
    }
}

impl TrueNorthServer {
    /// Run the red-stage failing test command and enforce its exit-code semantics.
    ///
    /// A non-zero exit reports red passed (Requirement 2.9). An exit code 0 reports red
    /// failed, since the test did not fail as required (Requirement 2.10).
    fn run_red_stage(&self, failing_test_cmd: &str) -> Result<(), ErrorData> {
        let binary = failing_test_cmd
            .split_whitespace()
            .next()
            .map(str::to_string)
            .unwrap_or_default();
        let cfg = SandboxConfig::new(self.ctx.repo_root.clone(), vec![binary]);

        let result = SystemCommandRunner
            .run(failing_test_cmd, &cfg)
            .map_err(|error| {
                ErrorData::internal_error(
                    format!("could not run the failing test command: {error}"),
                    None,
                )
            })?;

        if result.exit_code == Some(0) {
            return Err(ErrorData::invalid_request(
                "red step failed: the test did not fail as required (it exited 0). \
                 Write a test that fails before you write the code."
                    .to_string(),
                None,
            ));
        }
        Ok(())
    }
}

/// Check the failing test command length (Requirement 2.6).
fn check_failing_test_cmd(value: &str) -> Result<(), ErrorData> {
    let len = value.chars().count();
    if !(1..=MAX_FAILING_TEST_CMD).contains(&len) {
        return Err(ErrorData::invalid_params(
            format!("`failing_test_cmd` must be 1 to {MAX_FAILING_TEST_CMD} characters"),
            None,
        ));
    }
    Ok(())
}

/// Check the files-to-modify array (Requirement 2.6).
fn check_files_to_modify(files: &[String]) -> Result<(), ErrorData> {
    if files.is_empty() || files.len() > MAX_FILES_TO_MODIFY {
        return Err(ErrorData::invalid_params(
            format!("`files_to_modify` must have 1 to {MAX_FILES_TO_MODIFY} entries"),
            None,
        ));
    }
    if files.iter().any(|f| f.trim().is_empty()) {
        return Err(ErrorData::invalid_params(
            "`files_to_modify` entries must not be empty".to_string(),
            None,
        ));
    }
    Ok(())
}

/// Map a cockpit error to an MCP error.
fn cockpit_error(error: CockpitError) -> ErrorData {
    match error {
        CockpitError::Validation(_) => ErrorData::invalid_params(error.to_string(), None),
        CockpitError::Write(_) | CockpitError::Io { .. } => {
            ErrorData::internal_error(error.to_string(), None)
        }
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `tdd`.
#[cfg(test)]
#[path = "tdd_tests.rs"]
mod tests;
