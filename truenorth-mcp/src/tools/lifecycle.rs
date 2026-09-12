//! Lifecycle tools: `truenorth_advance_phase` and `truenorth_record_task`.
//!
//! `truenorth_advance_phase` moves the lifecycle phase, writes `state.yaml`, records the
//! git-scoped context, and emits `resources/updated` for `truenorth://state`
//! (Requirements 2.2, 2.3). `truenorth_record_task` registers a task under an epic,
//! appends it to `release-plan.yaml`, and emits `resources/updated` for
//! `truenorth://cockpit` (Requirements 2.4, 2.5).
//!
//! Both reject schema-violating input by naming the offending field, without a partial
//! mutation (Requirement 2.11). On a write failure the target file is left in its
//! pre-invocation state (Requirement 2.12).
//!
//! Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.11, 2.12. Design: Part II §2.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ResourceUpdatedNotificationParam};
use rmcp::service::{Peer, RoleServer};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::cockpit::{self, CockpitError};
use crate::engine::git;
use crate::engine::spec::Phase;
use crate::engine::validate::{ValidationError, map_legacy_phase};
use crate::engine::watcher::ResourceUri;
use crate::server::TrueNorthServer;

/// The maximum length of an artifacts summary (Requirement 2.2).
const MAX_ARTIFACTS_SUMMARY: usize = 4000;
/// The maximum length of a task name (Requirement 2.4).
const MAX_TASK_NAME: usize = 200;
/// The maximum length of a verify command (Requirement 2.4).
const MAX_VERIFY_COMMAND: usize = 1000;

/// The `truenorth_advance_phase` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AdvancePhaseArgs {
    /// The phase to move from. One of the six lifecycle phases.
    pub from_phase: String,
    /// The phase to move to. One of the six lifecycle phases.
    pub to_phase: String,
    /// A one-paragraph summary of the artifacts produced this phase (1 to 4000 chars).
    pub artifacts_summary: String,
}

/// The `truenorth_record_task` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordTaskArgs {
    /// The epic id, matching `^e[0-9]+([a-z0-9-]*)?$`.
    pub epic_id: String,
    /// The task name (1 to 200 chars).
    pub task_name: String,
    /// The shell command that proves the task done (1 to 1000 chars).
    pub verify_command: String,
}

#[tool_router(router = lifecycle_router, vis = "pub")]
impl TrueNorthServer {
    /// Advance the lifecycle phase and record the artifacts summary.
    #[tool(description = "Advance the lifecycle phase, write state.yaml, and record git context.")]
    pub async fn truenorth_advance_phase(
        &self,
        params: Parameters<AdvancePhaseArgs>,
        peer: Peer<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate every field before any mutation (Requirement 2.11).
        let _from = parse_phase(&args.from_phase, "from_phase")?;
        let to = parse_phase(&args.to_phase, "to_phase")?;
        check_len(
            &args.artifacts_summary,
            1,
            MAX_ARTIFACTS_SUMMARY,
            "artifacts_summary",
        )?;

        // Capture the git-scoped context. A non-git repo yields an empty context rather
        // than failing the advance.
        let git_context = git::status(&self.ctx.repo_root).unwrap_or_default();

        cockpit::advance_phase(
            &self.ctx.repo_root,
            to,
            &args.artifacts_summary,
            &git_context,
        )
        .map_err(cockpit_error)?;

        notify_updated(&peer, ResourceUri::State).await;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::json!({ "advanced_to": args.to_phase }).to_string(),
        )]))
    }

    /// Register a task under an epic with a verify command.
    #[tool(
        description = "Record a task under an epic with its verify command in release-plan.yaml."
    )]
    pub async fn truenorth_record_task(
        &self,
        params: Parameters<RecordTaskArgs>,
        peer: Peer<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate every field before any mutation (Requirement 2.11).
        check_epic_id(&args.epic_id)?;
        check_len(&args.task_name, 1, MAX_TASK_NAME, "task_name")?;
        check_len(
            &args.verify_command,
            1,
            MAX_VERIFY_COMMAND,
            "verify_command",
        )?;

        cockpit::record_task(
            &self.ctx.repo_root,
            &args.epic_id,
            &args.task_name,
            &args.verify_command,
        )
        .map_err(cockpit_error)?;

        notify_updated(&peer, ResourceUri::Cockpit).await;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::json!({ "recorded_task": args.task_name, "epic_id": args.epic_id })
                .to_string(),
        )]))
    }
}

/// Emit a best-effort `resources/updated` notification. A send failure is non-fatal,
/// because the write already succeeded and the notification is advisory.
async fn notify_updated(peer: &Peer<RoleServer>, uri: ResourceUri) {
    let param = ResourceUpdatedNotificationParam::new(uri.as_str());
    if let Err(error) = peer.notify_resource_updated(param).await {
        tracing::debug!(%error, uri = uri.as_str(), "resources/updated notification was not delivered");
    }
}

/// Parse a phase name into a [`Phase`], naming the field on failure (Requirement 2.11).
///
/// Legacy phase names are accepted and mapped, so an existing flow keeps working.
fn parse_phase(value: &str, field: &str) -> Result<Phase, ErrorData> {
    map_legacy_phase(value).map_err(|error| match error {
        ValidationError::UnknownPhase { .. } => ErrorData::invalid_params(
            format!(
                "`{field}` is not a valid phase: {value}. \
                 Use one of: discover, design, plan, execute, review, integrate."
            ),
            None,
        ),
        other => ErrorData::invalid_params(other.to_string(), None),
    })
}

/// Check a string field's length bounds, naming the field on failure (Requirement 2.11).
fn check_len(value: &str, min: usize, max: usize, field: &str) -> Result<(), ErrorData> {
    let len = value.chars().count();
    if len < min || len > max {
        return Err(ErrorData::invalid_params(
            format!("`{field}` must be {min} to {max} characters, but it was {len}"),
            None,
        ));
    }
    Ok(())
}

/// Check the epic-id pattern `^e[0-9]+([a-z0-9-]*)?$` (Requirement 2.4).
fn check_epic_id(value: &str) -> Result<(), ErrorData> {
    if epic_id_re().is_match(value) {
        Ok(())
    } else {
        Err(ErrorData::invalid_params(
            format!("`epic_id` must match ^e[0-9]+([a-z0-9-]*)?$, but it was `{value}`"),
            None,
        ))
    }
}

/// The compiled epic-id pattern.
fn epic_id_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^e[0-9]+([a-z0-9-]*)?$").expect("epic-id regex compiles"))
}

/// Map a cockpit write error to an MCP error (Requirements 2.11, 2.12).
fn cockpit_error(error: CockpitError) -> ErrorData {
    match error {
        CockpitError::Validation(_) => ErrorData::invalid_params(error.to_string(), None),
        CockpitError::Io { .. } => ErrorData::internal_error(error.to_string(), None),
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `lifecycle`.
#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
