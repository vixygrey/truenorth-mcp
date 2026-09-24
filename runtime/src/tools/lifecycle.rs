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

use crate::tools::result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ResourceUpdatedNotificationParam};
use rmcp::service::{Peer, RoleServer};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use std::path::Path;

use crate::engine::cockpit::{self, CockpitError};
use crate::engine::git;
use crate::engine::profile::{self, GroupingRule};
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

/// The maximum length of a grouping id (Requirement 4.1, 4.4).
const MAX_GROUP_ID: usize = 200;
/// The allowed `group_kind` values (Requirement 4.5).
const GROUP_KINDS: [&str; 4] = ["epic", "sprint", "milestone", "ticket"];

/// The `truenorth_record_task` input contract (design §4.1).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordTaskArgs {
    /// The optional grouping id (1 to 200 chars). Its presence rule depends on the active
    /// profile (Requirements 4.2, 4.3).
    #[serde(default)]
    pub group_id: Option<String>,
    /// The optional group kind, one of epic, sprint, milestone, ticket (Requirements 4.1,
    /// 4.5).
    #[serde(default)]
    pub group_kind: Option<String>,
    /// Legacy field. When present, maps to `group_kind = epic` and `group_id = epic_id`
    /// (Requirement 4.7).
    #[serde(default)]
    pub epic_id: Option<String>,
    /// The task name (1 to 200 chars).
    pub task_name: String,
    /// The shell command that proves the task done (1 to 1000 chars).
    pub verify_command: String,
}

/// The resolved grouping key after legacy mapping and validation.
#[derive(Debug)]
struct Grouping {
    id: Option<String>,
    kind: Option<String>,
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
        let from = parse_phase(&args.from_phase, "from_phase")?;
        let to = parse_phase(&args.to_phase, "to_phase")?;
        check_len(
            &args.artifacts_summary,
            1,
            MAX_ARTIFACTS_SUMMARY,
            "artifacts_summary",
        )?;
        let response = result::success(
            vec![ContentBlock::text(
                serde_json::json!({ "advanced_to": args.to_phase }).to_string(),
            )],
            self.ctx.token_caps,
        )?;

        // Capture the git-scoped context. A non-git repo yields an empty context rather
        // than failing the advance.
        let git_context = git::status(&self.ctx.repo_root).unwrap_or_default();

        cockpit::advance_phase(
            &self.ctx.repo_root,
            from,
            to,
            &args.artifacts_summary,
            &git_context,
        )
        .map_err(cockpit_error)?;

        notify_updated(&peer, ResourceUri::State).await;

        Ok(response)
    }

    /// Register a task with its optional grouping key and a verify command.
    #[tool(
        description = "Record a task with an optional grouping key and its verify command in the release plan."
    )]
    pub async fn truenorth_record_task(
        &self,
        params: Parameters<RecordTaskArgs>,
        peer: Peer<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate every field before any mutation, preserving the pre-call state on any
        // reject (Requirement 2.11, 4.2 to 4.8).
        check_len(&args.task_name, 1, MAX_TASK_NAME, "task_name")?;
        check_len(
            &args.verify_command,
            1,
            MAX_VERIFY_COMMAND,
            "verify_command",
        )?;
        let grouping = resolve_grouping(&self.ctx.repo_root, &args)?;
        let response = result::success(
            vec![ContentBlock::text(
                serde_json::json!({
                    "recorded_task": args.task_name,
                    "group_id": grouping.id,
                    "group_kind": grouping.kind,
                })
                .to_string(),
            )],
            self.ctx.token_caps,
        )?;

        cockpit::record_task(
            &self.ctx.repo_root,
            grouping.id.as_deref(),
            grouping.kind.as_deref(),
            &args.task_name,
            &args.verify_command,
        )
        .map_err(cockpit_error)?;

        notify_updated(&peer, ResourceUri::Cockpit).await;

        Ok(response)
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

/// Resolve and validate the neutral grouping key (Requirements 4.2 to 4.8).
///
/// Validation runs before any mutation, so a reject preserves the pre-call state:
///
/// - A caller-supplied `group_kind` must be in the allowed set (Requirement 4.5) and the
///   active profile vocabulary (Requirement 4.6).
/// - A caller-supplied `group_id` must be 1 to 200 characters (Requirement 4.4).
/// - A legacy `epic_id` maps to `group_kind = epic`, `group_id = epic_id` when the neutral
///   fields are absent, accepted under any profile (Requirement 4.7).
/// - A profile that requires grouping needs a `group_id` (Requirement 4.3). An optional
///   profile accepts the omission (Requirement 4.2).
fn resolve_grouping(repo_root: &Path, args: &RecordTaskArgs) -> Result<Grouping, ErrorData> {
    let active = profile::resolve_active(repo_root)
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;

    // A caller-supplied group_kind is checked against the allowed set (Requirement 4.5)
    // and the active profile vocabulary (Requirement 4.6). A group_kind synthesized from a
    // legacy epic_id is not caller-supplied, so it is accepted unconditionally
    // (Requirement 4.7), which keeps a legacy call working under any profile.
    if let Some(kind) = &args.group_kind {
        if !GROUP_KINDS.contains(&kind.as_str()) {
            return Err(ErrorData::invalid_params(
                format!("`group_kind` must be one of {GROUP_KINDS:?}, but it was `{kind}`."),
                None,
            ));
        }
        match active.vocab.as_kind_str() {
            Some(vocab) if vocab == kind => {}
            _ => {
                return Err(ErrorData::invalid_params(
                    format!(
                        "`group_kind` `{kind}` does not match the `{}` profile vocabulary `{:?}`.",
                        active.name, active.vocab
                    ),
                    None,
                ));
            }
        }
    }

    // A caller-supplied group_id must be 1 to 200 characters (Requirement 4.4).
    if let Some(id) = &args.group_id {
        check_len(id, 1, MAX_GROUP_ID, "group_id")?;
    }

    // Map a legacy epic_id onto the neutral key when the neutral fields are absent
    // (Requirement 4.7). The mapping is accepted regardless of the profile vocabulary.
    let (mut group_id, mut group_kind) = (args.group_id.clone(), args.group_kind.clone());
    if let Some(epic_id) = &args.epic_id
        && group_id.is_none()
        && group_kind.is_none()
    {
        check_epic_id(epic_id)?;
        group_id = Some(epic_id.clone());
        group_kind = Some("epic".to_string());
    }

    // A profile that requires grouping needs a group_id (Requirement 4.3). An optional
    // profile accepts the omission (Requirement 4.2).
    if active.rule == GroupingRule::Required && group_id.is_none() {
        return Err(ErrorData::invalid_params(
            format!(
                "the `{}` profile requires a grouping key, but `group_id` was absent.",
                active.name
            ),
            None,
        ));
    }

    Ok(Grouping {
        id: group_id,
        kind: group_kind,
    })
}

/// Check the legacy epic-id pattern `^e[0-9]+([a-z0-9-]*)?$` (Requirement 4.7).
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
    RE.get_or_init(|| crate::engine::regex_util::compile_static(r"^e[0-9]+([a-z0-9-]*)?$"))
}

fn cockpit_error(error: CockpitError) -> ErrorData {
    match error {
        CockpitError::Validation(_) => ErrorData::invalid_params(error.to_string(), None),
        CockpitError::Transition { .. } | CockpitError::InvalidPhase { .. } => {
            ErrorData::invalid_request(error.to_string(), None)
        }
        CockpitError::Write(_) | CockpitError::Io { .. } => {
            ErrorData::internal_error(error.to_string(), None)
        }
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `lifecycle`.
#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;

// Property tests (Properties 8 and 9) live in a separate sibling so the example-based
// unit tests stay focused. The `#[path]` include keeps them a child module of `lifecycle`.
#[cfg(test)]
#[path = "lifecycle_prop_tests.rs"]
mod prop_tests;
