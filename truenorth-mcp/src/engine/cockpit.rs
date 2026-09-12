//! Cockpit read and write orchestration for the lifecycle tools.
//!
//! These helpers read a cockpit file, apply a mutation, validate the result, and write it
//! back atomically. A write goes to a temp file in the same directory and then renames
//! over the target, so a failure leaves the target in its pre-invocation state
//! (Requirement 2.12). A mutation preserves every other field (Requirement 9.3), since
//! the models are map-backed.
//!
//! Requirements: 2.3, 2.5, 2.12, 9.3. Design: Part II §2, §8.

// These helpers are consumed by the lifecycle tools (task 9). They are unused until the
// tools wire them, so the module-scoped allow prevents a premature dead-code error under
// `clippy -D warnings`. Remove this allow once task 9 wires the consumer.
#![allow(dead_code)]

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

/// The path to `state.yaml` under a repository root.
pub fn state_path(repo_root: &Path) -> PathBuf {
    repo_root.join("specs").join("state.yaml")
}

/// The path to `release-plan.yaml` under a repository root.
pub fn release_plan_path(repo_root: &Path) -> PathBuf {
    repo_root.join("specs").join("release-plan.yaml")
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
    let path = state_path(repo_root);
    let mut state = read_state(&path)?;

    apply_phase(&mut state, to_phase, artifacts_summary, git_context);

    let yaml = validate_state_for_write(&state)?;
    write_atomic(&path, &yaml).map_err(|source| CockpitError::Io {
        file: "state.yaml".to_string(),
        source,
    })
}

/// Append a task to `release-plan.yaml` (Requirement 2.5).
///
/// The task is appended to a `tasks` sequence under the release plan, preserving every
/// other field. On any failure the file is left unchanged.
///
/// # Errors
///
/// Returns [`CockpitError`] on a read, validation, or write failure.
pub fn record_task(
    repo_root: &Path,
    epic_id: &str,
    task_name: &str,
    verify_command: &str,
) -> Result<(), CockpitError> {
    let path = release_plan_path(repo_root);
    let mut plan = read_release_plan(&path)?;

    apply_task(&mut plan, epic_id, task_name, verify_command);

    let yaml = validate_release_plan_for_write(&plan)?;
    write_atomic(&path, &yaml).map_err(|source| CockpitError::Io {
        file: "release-plan.yaml".to_string(),
        source,
    })
}

/// Read the recorded TDD step from `state.yaml`, or `None` when unset (Requirement 2.8).
///
/// # Errors
///
/// Returns [`CockpitError`] when the file exists but fails validation.
pub fn read_tdd_step(repo_root: &Path) -> Result<Option<TddStep>, CockpitError> {
    let state = read_state(&state_path(repo_root))?;
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
    let path = state_path(repo_root);
    let mut state = read_state(&path)?;

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
    write_atomic(&path, &yaml).map_err(|source| CockpitError::Io {
        file: "state.yaml".to_string(),
        source,
    })
}

/// Read and validate `state.yaml`, or start from an empty state when it is absent.
fn read_state(path: &Path) -> Result<StateFile, CockpitError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(validate_state(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StateFile::default()),
        Err(source) => Err(CockpitError::Io {
            file: "state.yaml".to_string(),
            source,
        }),
    }
}

/// Read and validate `release-plan.yaml`, or start from an empty plan when it is absent.
fn read_release_plan(path: &Path) -> Result<ReleasePlanFile, CockpitError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(validate_release_plan(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ReleasePlanFile::default())
        }
        Err(source) => Err(CockpitError::Io {
            file: "release-plan.yaml".to_string(),
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
fn apply_task(plan: &mut ReleasePlanFile, epic_id: &str, task_name: &str, verify_command: &str) {
    let mut task = serde_yaml::Mapping::new();
    task.insert(
        Value::String("epic_id".to_string()),
        Value::String(epic_id.to_string()),
    );
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
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
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
