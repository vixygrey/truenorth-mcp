//! Cockpit read and write orchestration for the lifecycle tools.
//!
//! These helpers read a cockpit file, apply a mutation, validate the result, and write it
//! back atomically. A write goes to a temp file in the same directory and then renames
//! over the target, so a failure leaves the target in its pre-invocation state
//! (Requirement 2.12). A mutation preserves every other field (Requirement 9.3), since
//! the models are map-backed.
//!
//! Requirements: 2.3, 2.5, 2.12, 9.3. Design: Part II §2, §8.

use std::path::{Path, PathBuf};

use serde_yaml::Value;
use thiserror::Error;

use crate::engine::agent_ws::write_under_agent;
use crate::engine::spec::{Phase, ReleasePlanFile, StateFile};
use crate::engine::tdd::TddStep;
use crate::engine::validate::{
    ValidationError, map_legacy_phase, validate_release_plan, validate_release_plan_for_write,
    validate_state, validate_state_for_write,
};

/// An error advancing a phase or recording a task.
#[derive(Debug, Error)]
pub enum CockpitError {
    /// The existing file failed validation on read (Requirement 9.2).
    #[error(transparent)]
    Validation(#[from] ValidationError),

    /// The requested transition disagrees with the recorded lifecycle state.
    #[error(
        "cannot advance lifecycle from `{from}` to `{to}`: state records `{current}` and \
         the next phase must be `{expected}`. The state file was left unchanged."
    )]
    Transition {
        /// The recorded current phase.
        current: &'static str,
        /// The phase claimed by the caller.
        from: &'static str,
        /// The requested target phase.
        to: &'static str,
        /// The only valid successor of the recorded phase.
        expected: &'static str,
    },

    /// The recorded phase has no supported lifecycle representation.
    #[error(
        "cannot advance lifecycle: `phase` must be a string or null, but was `{actual}`. \
         The state file was left unchanged."
    )]
    InvalidPhase {
        /// The unsupported YAML value.
        actual: String,
    },

    /// A write through the single guard failed. The target is unchanged (ADR-6, #187).
    #[error(transparent)]
    Write(#[from] crate::engine::agent_ws::WriteGuardError),

    /// The file could not be read. The target is unchanged (Requirement 2.12).
    #[error("could not access {file}: {source}. The file was left unchanged.")]
    Io {
        /// The cockpit file path.
        file: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

/// The path to the relocated state file under a repository root (Requirement 2.1).
pub fn state_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".agent").join(STATE_REL)
}

/// The path to the relocated release-plan file under a repository root (Requirement 2.2).
pub fn release_plan_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".agent").join(RELEASE_PLAN_REL)
}

/// The state file path relative to `.agent/`, for the write guard.
const STATE_REL: &str = "tasks/state.yml";

/// The release-plan file path relative to `.agent/`, for the write guard.
const RELEASE_PLAN_REL: &str = "tasks/release-plan.yml";

/// The legacy bigpowers state path under `specs/`, for the fallback read (Requirement 2.9).
fn legacy_state_path(repo_root: &Path) -> PathBuf {
    repo_root.join("specs").join("state.yaml")
}

/// The legacy bigpowers release-plan path under `specs/`, for the fallback read
/// (Requirement 2.9).
fn legacy_release_plan_path(repo_root: &Path) -> PathBuf {
    repo_root.join("specs").join("release-plan.yaml")
}

/// Resolve a cockpit read path, preferring the `.agent/` file and falling back to a legacy
/// `specs/` file when the `.agent/` file is absent (Requirement 2.9).
///
/// A write always targets the `.agent/` path, so the legacy file is never mutated
/// (Requirement 1.4).
fn read_source(agent_path: &Path, legacy_path: &Path) -> Option<PathBuf> {
    if agent_path.is_file() {
        return Some(agent_path.to_path_buf());
    }
    if legacy_path.is_file() {
        return Some(legacy_path.to_path_buf());
    }
    None
}

/// Advance the lifecycle phase in `state.yaml` and record the git context
/// (Requirement 2.3).
///
/// The requested source must match the recorded phase, and the target must be its immediate
/// successor. An absent or null recorded phase bootstraps as Discover for legacy compatibility.
/// The write records the target phase, the artifacts summary, and the git-scoped context while
/// preserving every other field. On any failure the file is left unchanged.
///
/// # Errors
///
/// Returns [`CockpitError`] on a read, transition, validation, or write failure.
pub fn advance_phase(
    repo_root: &Path,
    from_phase: Phase,
    to_phase: Phase,
    artifacts_summary: &str,
    git_context: &str,
) -> Result<(), CockpitError> {
    let mut state = read_state(repo_root)?;
    let current = current_phase(&state)?;
    let expected = current.successor();
    if current != from_phase || expected != to_phase {
        return Err(CockpitError::Transition {
            current: current.as_str(),
            from: from_phase.as_str(),
            to: to_phase.as_str(),
            expected: expected.as_str(),
        });
    }

    apply_phase(&mut state, to_phase, artifacts_summary, git_context);

    let yaml = validate_state_for_write(&state)?;
    write_under_agent(repo_root, Path::new(STATE_REL), &yaml)?;
    Ok(())
}

/// Append a task to the release plan with its neutral grouping key (Requirement 2.5, 4.9).
///
/// The task is appended to a `tasks` sequence under the release plan, carrying the
/// optional `group_id` and `group_kind` in place of the legacy `epic_id`, and preserving
/// every other field. On any failure the file is left unchanged.
///
/// # Errors
///
/// Returns [`CockpitError`] on a read, validation, or write failure.
pub fn record_task(
    repo_root: &Path,
    group_id: Option<&str>,
    group_kind: Option<&str>,
    task_name: &str,
    verify_command: &str,
) -> Result<(), CockpitError> {
    let mut plan = read_release_plan(repo_root)?;

    apply_task(&mut plan, group_id, group_kind, task_name, verify_command);

    let yaml = validate_release_plan_for_write(&plan)?;
    write_under_agent(repo_root, Path::new(RELEASE_PLAN_REL), &yaml)?;
    Ok(())
}

/// Read the recorded TDD step from `state.yaml`, or `None` when unset (Requirement 2.8).
///
/// # Errors
///
/// Returns [`CockpitError`] when the file exists but fails validation.
pub fn read_tdd_step(repo_root: &Path) -> Result<Option<TddStep>, CockpitError> {
    let state = read_state(repo_root)?;
    let step = state
        .get("tdd")
        .and_then(|v| v.get("step"))
        .and_then(|v| v.as_str())
        .and_then(TddStep::parse);
    Ok(step)
}

/// Read the active task from the state cockpit, or `None` when it is absent or empty (#331).
///
/// The drift guard uses this as a fallback scope definition when the caller supplies no task
/// on the change. An absent state file, an absent `active_task` field, a null value, or an
/// empty string all resolve to `None`, so the caller treats a missing task the same way in
/// every case.
///
/// # Errors
///
/// Returns [`CockpitError`] when the state file exists but fails validation.
pub fn read_active_task(repo_root: &Path) -> Result<Option<String>, CockpitError> {
    let state = read_state(repo_root)?;
    let task = state
        .get("active_task")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(task)
}

/// Write the recorded TDD step into `state.yaml`, preserving every other field
/// (Requirements 2.8, 9.3).
///
/// On any failure the file is left unchanged (Requirement 2.12).
///
/// # Errors
///
/// Returns [`CockpitError`] on a read, validation, or write failure.
pub fn write_tdd_step(repo_root: &Path, step: TddStep) -> Result<(), CockpitError> {
    let mut state = read_state(repo_root)?;

    let mut tdd = state
        .get("tdd")
        .and_then(|v| v.as_mapping().cloned())
        .unwrap_or_default();
    tdd.insert(
        Value::String("step".to_string()),
        Value::String(step.as_str().to_string()),
    );
    state.set("tdd", Value::Mapping(tdd));

    let yaml = validate_state_for_write(&state)?;
    write_under_agent(repo_root, Path::new(STATE_REL), &yaml)?;
    Ok(())
}

/// Read and validate the state cockpit, or start from an empty state when it is absent.
///
/// The read prefers `.agent/tasks/state.yml` and falls back to a legacy `specs/state.yaml`
/// when the `.agent/` file is absent (Requirement 2.9). The map-backed model preserves
/// every unknown field and the `bigpowers_version` value (Requirements 2.11, 2.13).
fn read_state(repo_root: &Path) -> Result<StateFile, CockpitError> {
    let agent = state_path(repo_root);
    let legacy = legacy_state_path(repo_root);
    let Some(path) = read_source(&agent, &legacy) else {
        return Ok(StateFile::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(validate_state(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StateFile::default()),
        Err(source) => Err(CockpitError::Io {
            file: ".agent/tasks/state.yml".to_string(),
            source,
        }),
    }
}

/// Resolve the recorded phase, treating an absent or null value as Discover.
fn current_phase(state: &StateFile) -> Result<Phase, CockpitError> {
    match state.get("phase") {
        None | Some(Value::Null) => Ok(Phase::Discover),
        Some(value) => {
            let Some(phase) = value.as_str() else {
                return Err(CockpitError::InvalidPhase {
                    actual: serde_yaml::to_string(value)
                        .unwrap_or_else(|_| "<unserializable YAML value>".to_string())
                        .trim()
                        .to_string(),
                });
            };
            map_legacy_phase(phase).map_err(CockpitError::Validation)
        }
    }
}

/// Read and validate the release plan, or start from an empty plan when it is absent.
///
/// The read prefers `.agent/tasks/release-plan.yml` and falls back to a legacy
/// `specs/release-plan.yaml` when the `.agent/` file is absent (Requirement 2.9). The
/// map-backed model preserves every unknown field (Requirement 2.11).
fn read_release_plan(repo_root: &Path) -> Result<ReleasePlanFile, CockpitError> {
    let agent = release_plan_path(repo_root);
    let legacy = legacy_release_plan_path(repo_root);
    let Some(path) = read_source(&agent, &legacy) else {
        return Ok(ReleasePlanFile::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(validate_release_plan(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ReleasePlanFile::default())
        }
        Err(source) => Err(CockpitError::Io {
            file: ".agent/tasks/release-plan.yml".to_string(),
            source,
        }),
    }
}

/// Record the phase transition into the state map, preserving every other field.
fn apply_phase(state: &mut StateFile, to_phase: Phase, artifacts_summary: &str, git_context: &str) {
    state.set("phase", phase_value(to_phase));

    // Record the artifacts summary and git context under the handoff block, preserving any
    // existing handoff fields.
    let mut handoff = state
        .get("handoff")
        .and_then(|v| v.as_mapping().cloned())
        .unwrap_or_default();
    handoff.insert(
        Value::String("artifacts_summary".to_string()),
        Value::String(artifacts_summary.to_string()),
    );
    handoff.insert(
        Value::String("git_context".to_string()),
        Value::String(git_context.to_string()),
    );
    state.set("handoff", Value::Mapping(handoff));
}

/// Append a task entry to the release plan's `tasks` sequence, preserving other fields.
///
/// The task carries the neutral grouping key: `group_id` and `group_kind` are written
/// only when present (Requirement 4.9). An absent grouping key writes no grouping field.
fn apply_task(
    plan: &mut ReleasePlanFile,
    group_id: Option<&str>,
    group_kind: Option<&str>,
    task_name: &str,
    verify_command: &str,
) {
    let mut task = serde_yaml::Mapping::new();
    if let Some(id) = group_id {
        task.insert(
            Value::String("group_id".to_string()),
            Value::String(id.to_string()),
        );
    }
    if let Some(kind) = group_kind {
        task.insert(
            Value::String("group_kind".to_string()),
            Value::String(kind.to_string()),
        );
    }
    task.insert(
        Value::String("task_name".to_string()),
        Value::String(task_name.to_string()),
    );
    task.insert(
        Value::String("verify_command".to_string()),
        Value::String(verify_command.to_string()),
    );

    let key = Value::String("tasks".to_string());
    match plan.root.get_mut(&key).and_then(Value::as_sequence_mut) {
        Some(tasks) => tasks.push(Value::Mapping(task)),
        None => {
            plan.root
                .insert(key, Value::Sequence(vec![Value::Mapping(task)]));
        }
    }
}

/// The kebab-case string value for a phase.
fn phase_value(phase: Phase) -> Value {
    Value::String(phase.as_str().to_string())
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `cockpit`.
#[cfg(test)]
#[path = "cockpit_tests.rs"]
mod tests;
