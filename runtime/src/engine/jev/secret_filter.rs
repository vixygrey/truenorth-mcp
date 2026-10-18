//! Secret-filtered, descriptor-relative Jev state builders.
//!
//! The shared file-set builder retains its standalone benchmark shape. The guard builder
//! sends a separately labeled proposal plus bounded, redacted existing context. No
//! optional context file is opened by following a symlink or walking a repository tree.
//! Every assembled state is checked against the shared denylist before a provider call.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Maximum bytes from any one existing file sent as model context.
const MAX_CONTEXT_BYTES: u64 = 32_768;

/// Filter a proposed change and bounded, explicitly identified existing context.
///
/// The proposal body belongs to the logical change, never implicitly to each path.
/// Context comes only from the selected target files, root conventions, and one
/// applicable manifest at the target's root or nearest ancestor. There is no tree scan.
pub(crate) fn build_guard_rigor_state(
    repo_root: &Path,
    paths: &[PathBuf],
    files: &mut [Option<File>],
    content: &str,
) -> Result<(Value, Vec<String>), super::JevError> {
    let mut unavailable = Vec::new();
    let mut existing = Vec::new();
    for (path, file) in paths.iter().zip(files.iter_mut()) {
        let Some(file) = file else {
            continue;
        };
        let mut bytes = Vec::new();
        file.take(MAX_CONTEXT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| path_error(path, error))?;
        if bytes.len() as u64 > MAX_CONTEXT_BYTES {
            unavailable.push(format!(
                "existing context for {} unavailable: exceeds 32768 bytes",
                path.display()
            ));
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            unavailable.push(format!(
                "existing context for {} unavailable: non-UTF-8",
                path.display()
            ));
            continue;
        };
        existing.push(json!({"path": path.to_string_lossy(), "content": filter_lines(&text)}));
    }

    let mut context = Vec::new();
    let mut candidates = vec![PathBuf::from("CONVENTIONS.md")];
    let manifest = paths.iter().find_map(|path| {
        let names: &[&str] = match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => &["Cargo.toml"],
            Some("js" | "jsx" | "ts" | "tsx") => &["package.json"],
            Some("py") => &["pyproject.toml", "requirements.txt"],
            Some("go") => &["go.mod"],
            _ => return None,
        };
        // Inspect only path ancestors, nearest first, then the repository root.
        let mut candidates = Vec::new();
        for ancestor in path.parent().into_iter().flat_map(Path::ancestors).take(8) {
            if ancestor.as_os_str().is_empty() {
                break;
            }
            candidates.extend(names.iter().map(|name| ancestor.join(name)));
        }
        candidates.extend(names.iter().map(|name| PathBuf::from(*name)));
        Some(candidates)
    });
    if let Some(manifests) = manifest.as_ref() {
        candidates.extend(manifests.iter().cloned());
    }

    // Reuse the descriptor-relative boundary for both path validation and symlink-safe opens.
    // An inaccessible optional context is reported, not replaced with untrusted disk reads.
    let mut has_manifest = false;
    for candidate in candidates {
        let name = candidate.to_string_lossy().into_owned();
        match open_state_files(repo_root, std::slice::from_ref(&candidate)) {
            Ok(mut opened) => {
                let Some(file) = opened.pop().flatten() else {
                    continue;
                };
                let mut bytes = Vec::new();
                if file
                    .take(MAX_CONTEXT_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .is_err()
                {
                    unavailable.push(format!("{name} unavailable: unreadable"));
                    continue;
                }
                if bytes.len() as u64 > MAX_CONTEXT_BYTES {
                    unavailable.push(format!("{name} unavailable: exceeds 32768 bytes"));
                    continue;
                }
                let Ok(text) = String::from_utf8(bytes) else {
                    unavailable.push(format!("{name} unavailable: non-UTF-8"));
                    continue;
                };
                context.push(json!({"path": name, "content": filter_lines(&text)}));
                if candidate != Path::new("CONVENTIONS.md") {
                    has_manifest = true;
                    break;
                }
            }
            Err(_) => unavailable.push(format!("{name} unavailable: unsafe or unreadable")),
        }
    }
    if !context
        .iter()
        .any(|entry| entry["path"] == "CONVENTIONS.md")
    {
        unavailable.push("CONVENTIONS.md unavailable".to_string());
    }
    if manifest.is_some() && !has_manifest {
        unavailable.push("dependency manifest unavailable".to_string());
    }
    if manifest.is_none() {
        unavailable.push("dependency manifest unavailable: unsupported source type".to_string());
    }
    let state = json!({
        "proposal": {"paths": paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
                     "content": filter_lines(content)},
        "existing_context": existing,
        "project_context": context,
        "unavailable_context": unavailable,
    });
    check_residual(&state)?;
    Ok((state, unavailable))
}

fn filter_lines(text: &str) -> String {
    let denylist = crate::config::secret_denylist();
    text.lines()
        .filter(|line| !denylist.iter().any(|pattern| pattern.is_match(line)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn check_residual(state: &Value) -> Result<(), super::JevError> {
    if crate::config::secret_denylist()
        .iter()
        .any(|pattern| pattern.is_match(&state.to_string()))
    {
        return Err(super::JevError::SecretResidual);
    }
    Ok(())
}

/// Validate guard CLI paths without reading content or invoking Jev.
///
/// # Errors
///
/// Returns an invalid-path error for unsafe or inaccessible repository targets.
pub fn validate_guard_paths(repo_root: &Path, paths: &[String]) -> Result<(), super::JevError> {
    open_state_files(repo_root, paths).map(|_| ())
}

/// Validate Git diff paths without opening file contents or applying read exclusions.
///
/// # Errors
///
/// Returns an invalid-path error for traversal, symlinks, or inaccessible targets.
pub fn validate_guard_metadata_paths(
    repo_root: &Path,
    paths: &[String],
) -> Result<(), super::JevError> {
    use crate::engine::agent_ws::fs_boundary::RepositoryFs;

    let fs = RepositoryFs::open(repo_root).map_err(|error| path_error(repo_root, error))?;
    for path in paths {
        fs.validate_metadata_path(Path::new(path))
            .map_err(|error| path_error(Path::new(path), error))?;
    }
    Ok(())
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
        "A change is out of scope when it writes a protected path, or when the \
         changed paths do work beyond the stated task. Judge the changed paths \
         against that task."
    } else {
        "No task was supplied. Judge only against protected paths; do not \
         infer a scope violation from an absent task."
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
