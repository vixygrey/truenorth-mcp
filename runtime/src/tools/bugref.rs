//! External-tracker bug reference tool: `truenorth_record_bug`.
//!
//! The tool inserts or updates a lean bug reference in `.agent/tasks/bugs.yml` through
//! the single write guard (Requirement 8.1). An existing id keeps its external link while
//! its lifecycle fields can change. The external tracker is the source of truth: the
//! runtime stores no bug narrative directory or tracker credential, makes no network
//! request, and integrates no tracker API (Requirements 8.2, 8.4).
//!
//! Validation runs before any write (Requirements 8.6, 8.7): the id is 1 to 200 chars,
//! the external link is an absolute URL, the status is one of the enum, and the linked
//! reference resolves to an existing task or group id under `.agent/tasks/`. A missing or
//! invalid field returns an error naming the offending field, and `.agent/tasks/bugs.yml`
//! stays in its pre-invocation state.
//!
//! Requirements: 8.1, 8.2, 8.3, 8.4, 8.6, 8.7. Design: agent-workspace-profiles §9.

use std::path::Path;

use crate::tools::result;
use rmcp::handler::server::{tool::RequestId, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::engine::agent_ws::{ObservedFile, WritePrecondition, write_under_agent_if_unchanged};
use crate::engine::cockpit::release_plan_path;
use crate::server::TrueNorthServer;
use crate::tools::mutation_error::{mutation_error, write_error};
use crate::tools::receipt::{self, ChangedContent, OperationReceipt};

/// The maximum length of a bug id (Requirement 8.6).
const MAX_BUG_ID: usize = 200;
/// The `bugs.yml` path relative to `.agent/`, for the write guard.
const BUGS_REL_PATH: &str = "tasks/bugs.yml";
const BUGS_REPO_PATH: &str = ".agent/tasks/bugs.yml";

/// The bounded bug status enumeration (Requirement 8.6, design §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BugStatus {
    /// Reported, not yet triaged.
    Open,
    /// Triaged and accepted.
    Triaged,
    /// Being worked.
    InProgress,
    /// Fixed, pending verification.
    Resolved,
    /// Closed.
    Closed,
}

impl BugStatus {
    /// The kebab-case label stored in `bugs.yml`.
    fn as_str(self) -> &'static str {
        match self {
            BugStatus::Open => "open",
            BugStatus::Triaged => "triaged",
            BugStatus::InProgress => "in-progress",
            BugStatus::Resolved => "resolved",
            BugStatus::Closed => "closed",
        }
    }
}

/// The `truenorth_record_bug` input contract (design §9).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordBugArgs {
    /// The bug id, a non-empty string of 1 to 200 chars (Requirement 8.6).
    pub id: String,
    /// The external tracker link, an absolute URL (Requirement 8.6).
    pub external_link: String,
    /// The bug status (Requirement 8.6).
    pub status: BugStatus,
    /// The linked task or group id; must reference an existing id (Requirement 8.6).
    pub linked_ref: String,
    /// Optional caller-supplied tags, stored as given (Requirement 8.3).
    #[serde(default)]
    pub tags: Vec<String>,
}

#[tool_router(router = bugref_router, vis = "pub")]
impl TrueNorthServer {
    /// Record or update a lean bug reference backed by an external tracker.
    #[tool(
        description = "Record or update a bug reference (id, external link, status, linked task or group, tags) in .agent/tasks/bugs.yml."
    )]
    pub async fn truenorth_record_bug(
        &self,
        params: Parameters<RecordBugArgs>,
        request_id: RequestId,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate every field before any write, preserving the pre-invocation state on a
        // reject (Requirements 8.6, 8.7).
        let id_len = args.id.chars().count();
        if !(1..=MAX_BUG_ID).contains(&id_len) {
            return Err(ErrorData::invalid_params(
                format!("`id` must be 1 to {MAX_BUG_ID} characters, but it was {id_len}"),
                None,
            ));
        }
        if !is_absolute_url(&args.external_link) {
            return Err(ErrorData::invalid_params(
                format!(
                    "`external_link` must be an absolute URL (for example https://tracker/123), \
                     but it was `{}`",
                    args.external_link
                ),
                None,
            ));
        }
        let permit = self.ctx.mutations.begin().await.map_err(mutation_error)?;
        if !self.linked_ref_resolves(&args.linked_ref) {
            return Err(ErrorData::invalid_params(
                format!(
                    "`linked_ref` `{}` does not resolve to an existing task or group id in \
                     .agent/tasks/",
                    args.linked_ref
                ),
                None,
            ));
        }
        // Insert a new reference or update the lifecycle fields of the existing reference.
        // The external link is the stable identity boundary: reusing an id for another
        // external issue is rejected before the guarded write.
        let (mut doc, precondition) = self.read_bugs_for_write()?;
        upsert_bug(&mut doc, &args)?;
        let yaml = serde_yaml::to_string(&doc).map_err(|e| {
            ErrorData::internal_error(format!("could not serialize the bug references: {e}"), None)
        })?;
        let change = ChangedContent::new(BUGS_REPO_PATH, yaml.as_bytes()).map_err(receipt_error)?;
        let receipt =
            OperationReceipt::new("truenorth_record_bug", &request_id, vec![change], None)
                .map_err(receipt_error)?;
        let response = result::success(
            vec![ContentBlock::text(
                receipt::attach(
                    serde_json::json!({
                        "recorded_bug": args.id,
                        "status": args.status.as_str(),
                        "linked_ref": args.linked_ref,
                    }),
                    receipt,
                )
                .to_string(),
            )],
            self.ctx.token_caps,
        )?;
        write_under_agent_if_unchanged(
            &self.ctx.repo_root,
            Path::new(BUGS_REL_PATH),
            &yaml,
            &precondition,
        )
        .map_err(write_error)?;
        drop(permit);

        Ok(response)
    }
}

fn receipt_error(error: String) -> ErrorData {
    ErrorData::internal_error(format!("could not build mutation receipt: {error}"), None)
}

impl TrueNorthServer {
    /// Read the current `bugs.yml` document, or an empty document when it is absent.
    fn read_bugs(&self) -> Value {
        let path = self.ctx.repo_root.join(".agent").join(BUGS_REL_PATH);
        let parsed = match std::fs::read_to_string(&path) {
            Ok(text) => serde_yaml::from_str(&text).unwrap_or_else(|_| empty_bugs_doc()),
            Err(_) => empty_bugs_doc(),
        };
        // Guarantee a mapping shape. A hand-edited `bugs.yml` that parses as a non-mapping
        // (a bare list or a scalar) falls back to an empty document, so the append path
        // never needs to unwrap the shape.
        if parsed.is_mapping() {
            parsed
        } else {
            empty_bugs_doc()
        }
    }

    fn read_bugs_for_write(&self) -> Result<(Value, WritePrecondition), ErrorData> {
        let path = self.ctx.repo_root.join(".agent").join(BUGS_REL_PATH);
        let (text, observed) = ObservedFile::read_string(path).map_err(|error| {
            ErrorData::internal_error(
                format!("could not read .agent/tasks/bugs.yml: {error}"),
                None,
            )
        })?;
        let parsed = text
            .as_deref()
            .map(serde_yaml::from_str)
            .transpose()
            .map_err(|error| {
                ErrorData::invalid_request(
                    format!(
                        ".agent/tasks/bugs.yml failed to parse: {error}. Fix the YAML and retry."
                    ),
                    None,
                )
            })?
            .unwrap_or_else(empty_bugs_doc);
        if !parsed.is_mapping() {
            return Err(ErrorData::invalid_request(
                ".agent/tasks/bugs.yml must contain a YAML mapping. Fix the file and retry."
                    .to_string(),
                None,
            ));
        }
        Ok((parsed, WritePrecondition::new(observed)))
    }

    /// Report whether `linked_ref` resolves to an existing task or group id.
    ///
    /// The reference resolves when it matches a `group_id` or a `task_name` in the release
    /// plan under `.agent/tasks/`, or an existing bug id in `bugs.yml`. This keeps the
    /// reference honest without a tracker call (Requirement 8.6).
    fn linked_ref_resolves(&self, linked_ref: &str) -> bool {
        self.release_plan_has_ref(linked_ref) || self.bugs_have_id(linked_ref)
    }

    /// Whether the release plan carries a matching `group_id` or `task_name`.
    fn release_plan_has_ref(&self, needle: &str) -> bool {
        let path = release_plan_path(&self.ctx.repo_root);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        let Ok(value) = serde_yaml::from_str::<Value>(&text) else {
            return false;
        };
        let Some(tasks) = value.get("tasks").and_then(Value::as_sequence) else {
            return false;
        };
        tasks
            .iter()
            .any(|task| field_eq(task, "group_id", needle) || field_eq(task, "task_name", needle))
    }

    /// Whether `bugs.yml` already carries a bug with the given id.
    fn bugs_have_id(&self, needle: &str) -> bool {
        let doc = self.read_bugs();
        doc.get("bugs")
            .and_then(Value::as_sequence)
            .is_some_and(|bugs| bugs.iter().any(|bug| field_eq(bug, "id", needle)))
    }
}

/// An empty bug-references document: `{ bugs: [] }`.
fn empty_bugs_doc() -> Value {
    let mut map = serde_yaml::Mapping::new();
    map.insert(Value::String("bugs".to_string()), Value::Sequence(vec![]));
    Value::Mapping(map)
}

/// Insert a bug reference or update its lifecycle fields when the id already exists.
///
/// Unknown document and entry fields survive an update. An id cannot be rebound to a
/// different external issue because recurrence and resolution must target the original
/// tracker record.
fn upsert_bug(doc: &mut Value, args: &RecordBugArgs) -> Result<(), ErrorData> {
    if !doc.is_mapping() {
        *doc = empty_bugs_doc();
    }
    let key = Value::String("bugs".to_string());
    let Some(mapping) = doc.as_mapping_mut() else {
        return Ok(());
    };
    let bugs = mapping
        .entry(key)
        .or_insert_with(|| Value::Sequence(Vec::new()));
    if !bugs.is_sequence() {
        *bugs = Value::Sequence(Vec::new());
    }
    let Some(bugs) = bugs.as_sequence_mut() else {
        return Ok(());
    };

    if let Some(existing) = bugs.iter_mut().find(|bug| field_eq(bug, "id", &args.id)) {
        let existing_link = existing
            .get("external_link")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if existing_link != args.external_link {
            return Err(ErrorData::invalid_params(
                format!(
                    "`external_link` `{}` conflicts with existing bug id `{}` linked to \
                     `{existing_link}`. Use the existing link or record a new bug id.",
                    args.external_link, args.id
                ),
                None,
            ));
        }
        if let Some(entry) = existing.as_mapping_mut() {
            entry.insert("status".into(), args.status.as_str().into());
            entry.insert("linked_ref".into(), args.linked_ref.clone().into());
            entry.insert(
                "tags".into(),
                Value::Sequence(args.tags.iter().cloned().map(Value::String).collect()),
            );
        }
        return Ok(());
    }

    let mut entry = serde_yaml::Mapping::new();
    entry.insert("id".into(), args.id.clone().into());
    entry.insert("external_link".into(), args.external_link.clone().into());
    entry.insert("status".into(), args.status.as_str().into());
    entry.insert("linked_ref".into(), args.linked_ref.clone().into());
    entry.insert(
        "tags".into(),
        Value::Sequence(args.tags.iter().cloned().map(Value::String).collect()),
    );
    bugs.push(Value::Mapping(entry));
    Ok(())
}

/// Whether a mapping value's string field equals `needle`.
fn field_eq(value: &Value, field: &str, needle: &str) -> bool {
    value.get(field).and_then(Value::as_str) == Some(needle)
}

/// Report whether `link` is an absolute URL.
///
/// An absolute URL has a scheme of one or more ASCII letters, then `://`, then a non-empty
/// authority. This is a dependency-free check that rejects a relative path or a bare word
/// without making a network request (Requirement 8.4).
fn is_absolute_url(link: &str) -> bool {
    let Some((scheme, rest)) = link.split_once("://") else {
        return false;
    };
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    // The authority (host, path, ...) after the scheme must be non-empty.
    !rest.is_empty()
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `bugref`.
#[cfg(test)]
#[path = "bugref_tests.rs"]
mod tests;

// Property tests (Property 14) live in a separate sibling so the example-based unit tests
// stay focused. The `#[path]` include keeps them a child module of `bugref`.
#[cfg(test)]
#[path = "bugref_prop_tests.rs"]
mod prop_tests;
