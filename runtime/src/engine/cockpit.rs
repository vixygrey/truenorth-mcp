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

use crate::engine::spec::{Phase, ReleasePlanFile, StateFile};
use crate::engine::tdd::TddStep;
use crate::engine::validate::{
    ValidationError, validate_release_plan, validate_release_plan_for_write, validate_state,
    validate_state_for_write,
};

/// An error advancing a phase or recording a task.
#[derive(Debug, Error)]
pub enum CockpitError {
    /// The existing file failed validation on read (Requirement 9.2).
    #[error(transparent)]
    Validation(#[from] ValidationError),

    /// The file could not be read or written. The target is unchanged (Requirement 2.12).
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
    repo_root.join(".agent").join("tasks").join("state.yml")
}

/// The path to the relocated release-plan file under a repository root (Requirement 2.2).
pub fn release_plan_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".agent")
        .join("tasks")
        .join("release-plan.yml")
}

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
/// The write records the target phase, the artifacts summary, and the git-scoped context
/// string, and preserves every other field. On any failure the file is left unchanged.
///
/// # Errors
///
/// Returns [`CockpitError`] on a read, validation, or write failure.
pub fn advance_phase(
    repo_root: &Path,
    to_phase: Phase,
    artifacts_summary: &str,
    git_context: &str,
) -> Result<(), CockpitError> {
    let mut state = read_state(repo_root)?;

    apply_phase(&mut state, to_phase, artifacts_summary, git_context);

    let yaml = validate_state_for_write(&state)?;
    write_atomic(&state_path(repo_root), &yaml).map_err(|source| CockpitError::Io {
        file: ".agent/tasks/state.yml".to_string(),
        source,
    })
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
    write_atomic(&release_plan_path(repo_root), &yaml).map_err(|source| CockpitError::Io {
        file: ".agent/tasks/release-plan.yml".to_string(),
        source,
    })
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
    write_atomic(&state_path(repo_root), &yaml).map_err(|source| CockpitError::Io {
        file: ".agent/tasks/state.yml".to_string(),
        source,
    })
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
    let text = serde_yaml::to_value(phase)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    Value::String(text)
}

/// Write `contents` to `path` atomically (Requirement 2.12).
///
/// The write goes to a temp file in the same directory, then renames over the target. A
/// same-directory rename is atomic on the same filesystem, so a reader sees either the old
/// or the new file, never a partial one. On any failure the target is unchanged.
///
/// Crate-visible so the single write guard in [`crate::engine::agent_ws`] delegates the
/// byte write here, keeping one atomic-write implementation (ADR-6).
pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let temp = temp_sibling(path);
    std::fs::write(&temp, contents)?;
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Clean up the temp file so a failed write leaves no residue.
            let _ = std::fs::remove_file(&temp);
            Err(error)
        }
    }
}

/// A temp sibling path for the atomic write, unique to the process and target.
fn temp_sibling(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pid = std::process::id();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".{name}.{pid}.{unique}.tmp"))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `cockpit`.
#[cfg(test)]
#[path = "cockpit_tests.rs"]
mod tests;
