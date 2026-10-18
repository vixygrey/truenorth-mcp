//! The secret-filtered state builder: assemble the evaluation state from a file set while
//! keeping every secret out.
//!
//! A caller passes a set of paths to include in the Jev evaluation state. Every path is
//! validated against the repository boundary and every existing file is opened without following
//! symlinks before any content is read. Missing proposed files contribute no content. Secret
//! paths are rejected; secret content lines are redacted from the JSON state. The assembled
//! state is checked again before it can reach Jev.
//!
//! The builder makes no network call and never logs content. It reuses the shared denylist
//! from [`crate::config`]. No API key appears anywhere in this module.
//!
//! Requirements: 9.1, 9.2, 9.6, 9.7. Design: jev-integration-eval, the secret-filter step,
//! Property 31 (secret exclusion).

// The direct convenience builder is exercised by its sibling tests; the guard evaluation
// boundary uses pre-opened file handles to validate the entire set before reading.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Validate guard CLI paths without reading content or invoking Jev.
///
/// # Errors
///
/// Returns an invalid-path error for unsafe or inaccessible repository targets.
pub fn validate_guard_paths(repo_root: &Path, paths: &[String]) -> Result<(), super::JevError> {
    open_state_files(repo_root, paths).map(|_| ())
}

/// Open every existing safe path before any file content is read or sent to Jev.
///
/// A missing file is legitimate for a proposed new file; every other failure is an input
/// error. Lexical validation runs over the entire set before opening any file.
pub(crate) fn open_state_files(
    repo_root: &Path,
    paths: &[impl AsRef<Path>],
) -> Result<Vec<Option<File>>, super::JevError> {
    use crate::engine::agent_ws::fs_boundary::RepositoryFs;

    for path in paths {
        let path = path.as_ref();
        RepositoryFs::validate_path(path).map_err(|error| path_error(path, error))?;
    }
    let fs = RepositoryFs::open(repo_root).map_err(|error| super::JevError::InvalidGuardPath {
        path: "repository root".to_string(),
        detail: error.to_string(),
    })?;
    paths
        .iter()
        .map(|path| {
            let path = path.as_ref();
            match fs.open_read(path) {
                Ok(file) => Ok(Some(file)),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
                Err(error) => Err(path_error(path, error)),
            }
        })
        .collect()
}

fn path_error(path: &Path, error: std::io::Error) -> super::JevError {
    super::JevError::InvalidGuardPath {
        path: path.display().to_string(),
        detail: error.to_string(),
    }
}

/// Build a filtered Jev state from protected file handles.
///
/// Each file handle was opened by [`open_state_files`] before any content read. Missing proposed
/// files carry `None`. Secret content lines are redacted and the assembled state is re-checked.
pub(crate) fn build_state_from_open_files(
    paths: &[PathBuf],
    files: &mut [Option<File>],
) -> Result<Value, super::JevError> {
    let denylist = crate::config::secret_denylist();
    let mut entries: Vec<Value> = Vec::new();

    for (path, file) in paths.iter().zip(files) {
        let Some(file) = file else {
            continue;
        };
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|error| path_error(path, error))?;

        // Drop any line that matches the denylist (Requirement 9.2).
        let redacted: String = content
            .lines()
            .filter(|line| !denylist.iter().any(|pattern| pattern.is_match(line)))
            .collect::<Vec<_>>()
            .join("\n");

        entries.push(json!({
            "path": path.to_string_lossy(),
            "content": redacted,
        }));
    }

    let state = Value::Array(entries);

    // Post-exclusion re-check: fail closed on any residual secret (Requirement 9.7).
    let serialized = state.to_string();
    if denylist.iter().any(|pattern| pattern.is_match(&serialized)) {
        return Err(super::JevError::SecretResidual);
    }

    Ok(state)
}

/// Build the evaluation state from a file set using protected repository reads.
///
/// Missing proposed files contribute no content. Unsafe paths and read errors are rejected
/// before any Jev request; callers that evaluate changes should use [`open_state_files`] first
/// so every file is opened before any content is read.
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_state_with_secret_filter(
    repo_root: &Path,
    paths: &[PathBuf],
) -> Result<Value, super::JevError> {
    let mut files = open_state_files(repo_root, paths)?;
    build_state_from_open_files(paths, &mut files)
}

/// Build a lightweight drift-scope state from the changed paths and the protected set (#328).
///
/// The drift Noul question judges whether a change is out of scope, which the paths, the
/// protected-path list, and the task answer, not the file content. This builder assembles only
/// what that question needs: the changed paths, the protected-path list, an optional task
/// definition, and a short scope hint. It carries no file content, so it costs far fewer tokens
/// than [`build_state_with_secret_filter`].
///
/// When `task` is present, the state carries it and the scope hint tells the model to judge the
/// change against the task, so a change that sprawls beyond the task is out of scope (#331).
/// When `task` is absent, the state carries no task and the hint judges against the protected
/// paths only, exactly as before.
///
/// The secret boundary still holds. A changed path that matches the secret denylist is
/// excluded, so a secret path never reaches the model (Requirement 9.1). After assembly, the
/// builder re-checks the serialized state against the denylist and fails closed on any
/// residual match (Requirement 9.7). The task string is part of the assembled state, so it
/// passes through the same residual check.
///
/// # Errors
///
/// Returns [`super::JevError::SecretResidual`] when a secret survives the exclusion step.
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_drift_scope_state(
    changed_paths: &[PathBuf],
    protected_paths: &[String],
    task: Option<&str>,
) -> Result<Value, super::JevError> {
    let denylist = crate::config::secret_denylist();

    // Exclude a secret path outright, so no secret path reaches the model (Requirement 9.1).
    let paths: Vec<String> = changed_paths
        .iter()
        .filter(|path| !crate::config::is_secret_path(path))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();

    // The scope hint depends on whether a task is present. With a task, the model judges the
    // change against the task; without one, it judges against the protected paths only.
    let scope_note = if task.is_some() {
        "A change is out of scope when it writes a protected path, or when it does work \
         beyond the stated task. Judge the changed paths against the task and the protected \
         paths."
    } else {
        "A change is out of scope when it writes a protected path or a path unrelated to the \
         changed set. Judge the changed paths against the protected paths."
    };

    let mut state = json!({
        "changed_paths": paths,
        "protected_paths": protected_paths,
        "scope_note": scope_note,
    });
    // Add the task only when present, so an absent task leaves the state shape unchanged from
    // before this field existed.
    if let Some(task) = task {
        state["task"] = Value::String(task.to_string());
    }

    // Post-exclusion re-check: fail closed on any residual secret (Requirement 9.7). A
    // protected entry, a path, or the task could still carry a denylisted marker.
    let serialized = state.to_string();
    if denylist.iter().any(|pattern| pattern.is_match(&serialized)) {
        return Err(super::JevError::SecretResidual);
    }

    Ok(state)
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `secret_filter`.
#[cfg(test)]
#[path = "secret_filter_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "secret_filter_prop_tests.rs"]
mod prop_tests;
