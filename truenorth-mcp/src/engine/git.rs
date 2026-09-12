//! Git context scoped to the cockpit directories (design §1, ports `git-context.ts`).
//!
//! Every git operation is scoped to `skills/` and `specs/` through a pathspec, so status,
//! log, and diff output excludes changes outside those two directories (Requirement 1.8).
//! `changed_files_in_scope` is the default file set for the ontology gate (Requirement
//! 4.7).
//!
//! Requirements: 1.8, 4.7. Design: Part II §1.

// The git helpers are consumed by the git-context and lifecycle tools (tasks 8, 9) and by
// the ontology tool (task 12). They are unused until those tasks land, so the
// module-scoped allow prevents a premature dead-code error under `clippy -D warnings`.
// Remove this allow once task 12 wires the last consumer.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::config::GIT_SCOPE_DIRS;

/// A git action for the `get_git_context` tool (design §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitAction {
    /// The working-tree status.
    Status,
    /// The commit log.
    Log,
    /// The working-tree diff.
    Diff,
}

impl Default for GitAction {
    /// The default action is `status` (Requirement 7.6).
    fn default() -> Self {
        Self::Status
    }
}

/// An error from a git operation.
#[derive(Debug, Error)]
pub enum GitError {
    /// The `git` command could not be spawned.
    #[error("could not run git: {0}. Make sure that git is installed and on PATH.")]
    Spawn(#[from] std::io::Error),

    /// The `git` command ran but exited non-zero.
    #[error("git {action} failed: {stderr}")]
    Command {
        /// The action that failed.
        action: String,
        /// The trimmed stderr from git.
        stderr: String,
    },
}

/// Run a git action scoped to the cockpit directories (Requirement 1.8).
///
/// The output covers only `skills/` and `specs/`. A change outside those directories is
/// excluded.
///
/// # Errors
///
/// Returns [`GitError`] when git cannot run or exits non-zero.
pub fn git_context(repo_root: &Path, action: GitAction) -> Result<String, GitError> {
    match action {
        GitAction::Status => status(repo_root),
        GitAction::Log => log(repo_root),
        GitAction::Diff => diff(repo_root),
    }
}

/// The working-tree status, scoped to the cockpit directories.
///
/// `--untracked-files=all` lists each untracked file, so an untracked directory does not
/// collapse to a single directory entry. The scan needs the individual file paths.
pub fn status(repo_root: &Path) -> Result<String, GitError> {
    run(
        repo_root,
        "status",
        &["status", "--porcelain", "--untracked-files=all", "--"],
    )
}

/// The commit log, scoped to the cockpit directories.
pub fn log(repo_root: &Path) -> Result<String, GitError> {
    run(repo_root, "log", &["log", "--oneline", "-n", "20", "--"])
}

/// The working-tree diff, scoped to the cockpit directories.
pub fn diff(repo_root: &Path) -> Result<String, GitError> {
    run(repo_root, "diff", &["diff", "--"])
}

/// The files changed in scope, as repo-relative paths (Requirement 4.7).
///
/// The set is the default ontology scan scope. It covers tracked modifications and
/// untracked files under `skills/` and `specs/`, drawn from the porcelain status.
///
/// # Errors
///
/// Returns [`GitError`] when git cannot run or exits non-zero.
pub fn changed_files_in_scope(repo_root: &Path) -> Result<Vec<PathBuf>, GitError> {
    let porcelain = status(repo_root)?;
    Ok(parse_porcelain_paths(&porcelain))
}

/// Run a scoped git command, appending the cockpit directories as the pathspec.
///
/// The `base_args` end with `--`, so the scope directories become the pathspec and git
/// limits its output to them.
fn run(repo_root: &Path, action: &str, base_args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(base_args)
        .args(GIT_SCOPE_DIRS)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        return Err(GitError::Command {
            action: action.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parse repo-relative paths from `git status --porcelain` output.
///
/// Each porcelain line is `XY <path>`, where `XY` is the two-column status code. A rename
/// line carries `old -> new`; the new path is taken. Every path is already within scope,
/// because the status was produced with the scope pathspec.
fn parse_porcelain_paths(porcelain: &str) -> Vec<PathBuf> {
    porcelain
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            // Columns 0..2 are the status code, column 2 is a space, path starts at 3.
            let path_part = line[3..].trim();
            let path = match path_part.rsplit_once(" -> ") {
                Some((_, new_path)) => new_path,
                None => path_part,
            };
            Some(PathBuf::from(path))
        })
        .collect()
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `git`.
#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
