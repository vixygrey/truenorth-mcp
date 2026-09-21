//! The secret-filtered state builder: assemble the evaluation state from a file set while
//! keeping every secret out.
//!
//! A caller passes a set of paths to include in the Jev evaluation state. The builder reads
//! each file, excludes any file whose path matches the secret denylist, redacts any content
//! line that matches the denylist, and assembles a JSON array of `{ path, content }`
//! objects. Before it returns, the builder re-checks the whole assembled state against the
//! denylist and fails closed if any secret survived.
//!
//! The builder reads files but makes no network call and never logs content. It reuses the
//! shared denylist from [`crate::config`], so the path check and the content check match the
//! same patterns as the rest of the runtime. No API key appears anywhere in this module.
//!
//! Requirements: 9.1, 9.2, 9.6, 9.7. Design: jev-integration-eval, the secret-filter step,
//! Property 31 (secret exclusion).

// The builder has no non-test caller until the client and aspect modules land (later
// issues). It carries a narrow non-test `allow` with this reason, per the repo dead-code
// policy (main.rs). The task that adds the first caller removes the attribute. The test
// build exercises the exclusion, redaction, and fail-closed paths through the sibling
// property tests.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Build the evaluation state from a file set, keeping every secret out (Requirement 9.1,
/// 9.2, 9.7).
///
/// For each path: a path that matches the secret denylist is excluded (Requirement 9.1). A
/// file that cannot be read is skipped, so one unreadable file does not fail the whole build.
/// A content line that matches the denylist is dropped (Requirement 9.2). The builder then
/// assembles a JSON array of `{ path, content }` objects.
///
/// After assembly, the builder serializes the state and re-checks it against the denylist. A
/// residual match returns [`super::JevError::SecretResidual`] and makes no call (Requirement
/// 9.7). This is fail-closed: the builder prefers a rejection over a leak.
///
/// # Errors
///
/// Returns [`super::JevError::SecretResidual`] when a secret survives the exclusion and
/// redaction steps.
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_state_with_secret_filter(
    repo_root: &Path,
    paths: &[PathBuf],
) -> Result<Value, super::JevError> {
    let denylist = crate::config::secret_denylist();
    let mut entries: Vec<Value> = Vec::new();

    for path in paths {
        // Exclude a secret path outright (Requirement 9.1).
        if crate::config::is_secret_path(path) {
            continue;
        }

        let absolute = repo_root.join(path);
        // A read error skips the file rather than failing the whole build.
        let Ok(content) = std::fs::read_to_string(&absolute) else {
            continue;
        };

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
