//! Runtime configuration: repository-root resolution, secret denylist, git scope,
//! and sandbox settings.
//!
//! This module ports the upstream `config.ts` behavior into typed Rust. It resolves
//! the governed repository root, exposes the secret denylist that keeps credential
//! files out of produced output, pins the git scope to the two cockpit directories,
//! and defines the sandbox contract for gate execution.
//!
//! Requirements: 1.4, 1.5, 1.6, 1.7, 1.8. Design: Part II §1 (Config), §5 (Sandbox).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use thiserror::Error;

/// Environment variable that names an explicit repository root.
const REPO_ROOT_ENV: &str = "TRUENORTH_ROOT";

/// Marker directories that identify a valid repository root. A candidate is valid only
/// when it directly contains all three of these directories (Requirement 2.7). The
/// `.agent/` marker is the machine-facing workspace, `specs/` is the human-facing
/// narrative, and `skills/` holds the skill sources.
const MARKER_DIRS: [&str; 3] = [".agent", "specs", "skills"];

/// Git scope directories (ports `GIT_SCOPE_DIRS`). Git status, log, and diff output is
/// scoped to these three directories (Requirement 2.8). `specs/` stays in scope because
/// the runtime reads ADR content there.
pub const GIT_SCOPE_DIRS: [&str; 3] = [".agent", "specs", "skills"];

/// Maximum byte size of a single skill file read into memory.
pub const MAX_READ_SKILL_BYTES: usize = 512 * 1024;

/// Default wall-clock timeout for a sandboxed gate command (Requirement 3.2, §5).
pub const DEFAULT_GATE_TIMEOUT: Duration = Duration::from_secs(300);

/// An error from repository-root resolution.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// No candidate directory contained all three marker directories (Requirement 1.6).
    #[error(
        "no valid repository root among the evaluated candidates ({candidates}). \
         A valid root must directly contain a `.agent/`, a `specs/`, and a `skills/` \
         directory. \
         Set the `TRUENORTH_ROOT` environment variable to the repository root, \
         or run the server from inside the repository."
    )]
    NoValidRoot {
        /// The candidate paths that were evaluated, for the operator to inspect.
        candidates: String,
    },
}

/// Resolve the repository root (Requirement 1.4).
///
/// Candidates are evaluated in order. The first candidate that is a valid root wins:
///
/// 1. The directory named by `TRUENORTH_ROOT`, when the variable is set and non-empty.
/// 2. The current working directory.
/// 3. The parent of the package directory (the parent of the running binary's directory).
///
/// A candidate is a valid root only when it directly contains a `.agent/`, a `specs/`,
/// and a `skills/` directory (Requirement 2.7).
///
/// # Errors
///
/// Returns [`ConfigError::NoValidRoot`] when no candidate is a valid root
/// (Requirement 1.6). The caller (`main`) terminates with a non-zero exit status.
///
/// # Example
///
/// ```ignore
/// let root = config::get_repo_root()?;
/// ```
pub fn get_repo_root() -> Result<PathBuf, ConfigError> {
    select_repo_root(&repo_root_candidates())
}

/// Select the first valid repository root from an ordered candidate list.
///
/// This is the pure decision behind [`get_repo_root`]. It reads no process state, so
/// tests drive it with fixed candidates.
///
/// # Errors
///
/// Returns [`ConfigError::NoValidRoot`] when no candidate is a valid root.
fn select_repo_root(candidates: &[PathBuf]) -> Result<PathBuf, ConfigError> {
    for candidate in candidates {
        if is_valid_repo_root(candidate) {
            return Ok(candidate.clone());
        }
    }

    Err(ConfigError::NoValidRoot {
        candidates: format_candidates(candidates),
    })
}

/// Build the ordered candidate list for repository-root resolution.
///
/// The order is `TRUENORTH_ROOT` (when set and non-empty), then the current working
/// directory, then the parent of the package directory. Candidates that cannot be
/// determined are dropped from the list.
fn repo_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::with_capacity(3);

    if let Some(env_root) = env_root() {
        candidates.push(env_root);
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }

    if let Some(parent) = parent_of_package_dir() {
        candidates.push(parent);
    }

    candidates
}

/// Read the `TRUENORTH_ROOT` candidate. Returns `None` when the variable is unset or
/// empty, so an empty value does not shadow the other candidates (Requirement 1.4).
fn env_root() -> Option<PathBuf> {
    match std::env::var(REPO_ROOT_ENV) {
        Ok(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// The parent of the package directory, derived from the running binary's location.
///
/// The binary sits in a package directory. Its parent is the third resolution
/// candidate. Returns `None` when the binary path or its parents cannot be read.
fn parent_of_package_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let package_dir = exe.parent()?;
    package_dir.parent().map(Path::to_path_buf)
}

/// Report whether a candidate directory is a valid repository root (Requirement 1.5).
///
/// The candidate is valid only when it directly contains every marker directory.
fn is_valid_repo_root(candidate: &Path) -> bool {
    MARKER_DIRS
        .iter()
        .all(|marker| candidate.join(marker).is_dir())
}

/// Render the candidate list for the no-valid-root error message.
fn format_candidates(candidates: &[PathBuf]) -> String {
    if candidates.is_empty() {
        return "none".to_string();
    }
    candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The compiled secret denylist (Requirement 1.7).
///
/// A path that matches any of these patterns is excluded from produced output and its
/// environment value is dropped before a gate subprocess spawns. The denylist covers
/// environment files (`.env`), PEM files (`*.pem`), and any path that contains a
/// `secret` or `credentials` marker.
///
/// The patterns are case-insensitive and match against the full path. The set is
/// compiled once and reused.
pub fn secret_denylist() -> &'static [Regex] {
    static DENYLIST: OnceLock<Vec<Regex>> = OnceLock::new();
    DENYLIST.get_or_init(|| {
        // Every pattern is a fixed, valid regex. Compilation cannot fail at runtime,
        // so `expect` here is a programmer-error assertion, not a runtime error path.
        [
            // `.env` and dotted variants such as `.env.local`, as a full path segment.
            r"(?i)(^|/)\.env(\.[^/]+)?$",
            // PEM files by extension.
            r"(?i)\.pem$",
            // Any path containing a `secret` marker.
            r"(?i)secret",
            // Any path containing a `credentials` marker.
            r"(?i)credentials",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("secret denylist pattern must compile"))
        .collect()
    })
}

/// Report whether a path matches the secret denylist (Requirement 1.7).
///
/// The comparison uses the path's string form, so it matches path segments and file
/// extensions across platforms.
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
/// assert!(config::is_secret_path(Path::new("config/.env")));
/// assert!(!config::is_secret_path(Path::new("skills/deploy/SKILL.md")));
/// ```
pub fn is_secret_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    secret_denylist()
        .iter()
        .any(|pattern| pattern.is_match(&text))
}

/// Sandbox configuration for gate execution (ADR-1, §5).
///
/// The gate runner reads this contract to bound a subprocess: a wall-clock timeout with
/// a hard kill, a working directory pinned under the repository root, an allowlist that
/// gates the command's first token, and a flag that disables execution for evidence-only
/// deployments.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Wall-clock timeout. The runner hard-kills a command that exceeds it.
    pub timeout: Duration,
    /// Working directory for the spawned command. It must be under the repository root.
    pub working_dir: PathBuf,
    /// Permitted command binaries, matched against the command's first token.
    pub allowlist: Vec<String>,
    /// When `false`, the runner skips execution and requires caller-supplied evidence.
    pub execution_enabled: bool,
}

impl SandboxConfig {
    /// Build a sandbox config rooted at `working_dir` with the default gate timeout and
    /// execution enabled.
    ///
    /// The caller supplies the allowlist. An empty allowlist rejects every command.
    /// The gate runner surfaces this with a remediation hint (§5, task 5).
    pub fn new(working_dir: PathBuf, allowlist: Vec<String>) -> Self {
        Self {
            timeout: DEFAULT_GATE_TIMEOUT,
            working_dir,
            allowlist,
            execution_enabled: true,
        }
    }
}

// Unit tests live in a sibling file to hold this module under the ~300-line guidance.
// The `#[path]` include keeps them a child module of `config`, so they reach private
// items (`select_repo_root`, `is_valid_repo_root`) through `use super::*`.
#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
