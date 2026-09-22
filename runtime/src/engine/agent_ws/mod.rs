//! The agent workspace guard: the single write path under `.agent/`, the telemetry
//! read exclusion, and the layout-contract read.
//!
//! The runtime writes only under `.agent/`. Every runtime write funnels through
//! [`write_under_agent`], which normalizes the target, rejects any path that escapes
//! `.agent/` (lexically and after symlink resolution), then delegates to the module's
//! private atomic write. This is the load-bearing invariant of the feature (design ADR-6,
//! Property 6). The atomic write is private to this module, so the guard is the only door
//! to a byte write and cannot be bypassed by a direct call (#187). The scaffold is the one
//! authorized exception: it seeds files outside `.agent/` through the audited
//! [`write_repo_seed`] path, which the runtime tools never call.
//!
//! [`is_excluded_read`] reports the telemetry read exclusion. [`read_layout`] parses
//! `.agent/layout.yml` and confirms every required area and file is present, retaining
//! the last valid contract in a [`LayoutCache`] so a later broken read still serves the
//! last good state.
//!
//! Requirements: 1.1, 1.2, 1.3, 1.9, 1.10, 1.11, 1.12, 5.5, 5.6, 5.8, 5.9.
//! Design: agent-workspace-profiles §1, ADR-6.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// The agent workspace directory name (Requirement 1.1).
pub const AGENT_DIR: &str = ".agent";

/// The telemetry area, excluded from agent reads (Requirement 1.9, 1.10).
pub const TELEMETRY_AREA: &str = "telemetry";

mod layout;

pub use layout::{LayoutCache, LayoutError, read_layout};

/// An error from the single write guard (Requirement 1.3).
#[derive(Debug, Error)]
pub enum WriteGuardError {
    /// The target escapes `.agent/`. No file is written (Requirement 1.3).
    #[error(
        "refused to write `{target}`: the runtime writes only under `.agent/`. \
         Move the write target under `.agent/`."
    )]
    OutsideAgent {
        /// The rejected target, as supplied by the caller.
        target: String,
    },

    /// The delegated atomic write failed. The target is unchanged (Requirement 1.3).
    #[error("could not write `{target}`: {source}. The file was left unchanged.")]
    Io {
        /// The resolved target path.
        target: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

/// Write `contents` to `rel_path` under `.agent/`, rejecting any escaping path.
///
/// This is the only write path in runtime library code (ADR-6). It normalizes the joined
/// path and confirms it stays under `repo_root/.agent/`. A `..` traversal or an absolute
/// path that leaves `.agent/` is rejected. On rejection it writes nothing, so every
/// target is left unchanged (Requirement 1.2, 1.3). The byte write delegates to the
/// atomic `write_atomic` helper, so the atomic-rename behavior is unchanged.
///
/// # Errors
///
/// Returns [`WriteGuardError::OutsideAgent`] when `rel_path` escapes `.agent/`, and
/// [`WriteGuardError::Io`] when the delegated atomic write fails.
pub fn write_under_agent(
    repo_root: &Path,
    rel_path: &Path,
    contents: &str,
) -> Result<(), WriteGuardError> {
    let agent_root = repo_root.join(AGENT_DIR);
    let target =
        resolve_under(&agent_root, rel_path).ok_or_else(|| WriteGuardError::OutsideAgent {
            target: rel_path.display().to_string(),
        })?;

    // The lexical check above blocks `..` and absolute paths. Confirm the resolved target
    // also stays under `.agent/` after symlink resolution, so a symlinked directory inside
    // `.agent/` cannot redirect the write outside it (Requirement 1.3, #186).
    if !contained_under(&agent_root, &target) {
        return Err(WriteGuardError::OutsideAgent {
            target: rel_path.display().to_string(),
        });
    }

    write_atomic(&target, contents).map_err(|source| WriteGuardError::Io {
        target: target.display().to_string(),
        source,
    })
}

/// Write `contents` to `rel_path` under the repository root, for the scaffold seed only.
///
/// This is the one authorized write outside `.agent/` (ADR-6, ADR-7). The scaffold emits
/// root docs, `.githooks/`, and `.github/` templates, which live at the repository root.
/// The caller must pass `allow_repo_root_seed = true` to opt in; a `false` value rejects
/// every target, so a runtime tool that reaches this path by mistake writes nothing.
///
/// The target still resolves under `repo_root`, so a `..` traversal or an absolute path
/// that leaves the repository is rejected and nothing is written. The runtime tools never
/// call this path; only the scaffold does (Requirement 5.5, 5.6, 5.8, 5.9).
///
/// # Errors
///
/// Returns [`WriteGuardError::OutsideAgent`] when the seed is not allowed or when
/// `rel_path` escapes the repository root, and [`WriteGuardError::Io`] when the delegated
/// atomic write fails.
pub fn write_repo_seed(
    repo_root: &Path,
    rel_path: &Path,
    contents: &str,
    allow_repo_root_seed: bool,
) -> Result<(), WriteGuardError> {
    if !allow_repo_root_seed {
        return Err(WriteGuardError::OutsideAgent {
            target: rel_path.display().to_string(),
        });
    }

    let target =
        resolve_under(repo_root, rel_path).ok_or_else(|| WriteGuardError::OutsideAgent {
            target: rel_path.display().to_string(),
        })?;

    // Confirm the resolved seed target stays under the repository root after symlink
    // resolution, so a symlinked directory inside the repo cannot redirect the seed write
    // outside it (Requirement 1.3, #186).
    if !contained_under(repo_root, &target) {
        return Err(WriteGuardError::OutsideAgent {
            target: rel_path.display().to_string(),
        });
    }

    write_atomic(&target, contents).map_err(|source| WriteGuardError::Io {
        target: target.display().to_string(),
        source,
    })
}

/// Report whether a read target is excluded (the telemetry area, Requirement 1.10).
///
/// A path is excluded exactly when it sits under the `.agent/telemetry/` area. The check
/// normalizes separators so it holds on every platform. It accepts either a repository
/// relative path (`.agent/telemetry/runs.yml`) or an agent-relative path
/// (`telemetry/runs.yml`).
pub fn is_excluded_read(rel_path: &Path) -> bool {
    let text = rel_path.to_string_lossy().replace('\\', "/");
    let telemetry_root = format!("{AGENT_DIR}/{TELEMETRY_AREA}");
    text == telemetry_root
        || text.starts_with(&format!("{telemetry_root}/"))
        || text == TELEMETRY_AREA
        || text.starts_with(&format!("{TELEMETRY_AREA}/"))
}

/// Confirm that `target` stays under `base` after resolving symlinks (Requirement 1.3).
///
/// The lexical [`resolve_under`] blocks `..` and absolute paths, but it does not follow a
/// symlink. A symlinked directory inside `base` that points outside would let a
/// lexically-valid target escape on the real write. This check closes that gap.
///
/// The target is usually a file that does not exist yet, so it cannot be canonicalized
/// directly. Instead this canonicalizes `base` and the deepest existing ancestor of
/// `target`, then confirms the ancestor stays under the canonical base. A symlinked
/// intermediate directory exists, so it canonicalizes to its real location and is caught.
/// A not-yet-created final component is safe, because creating it later routes through
/// this same guard.
///
/// The lexical [`resolve_under`] already proves `target` is under `base` by path
/// components. This adds the symlink check: the deepest existing ancestor of `target` must
/// canonicalize under the deepest existing ancestor of `base`. Canonicalizing the existing
/// portion of `base` handles the first write, when `.agent/` does not exist yet, and it
/// normalizes a symlinked temp root (for example `/var` to `/private/var` on macOS) on
/// both sides. A symlinked intermediate directory under `base` exists, so it canonicalizes
/// to its real location and fails the containment check.
///
/// Returns `true` when the resolved `target` stays under `base`, and `false` when it
/// escapes or when no existing anchor can be found (fail closed).
fn contained_under(base: &Path, target: &Path) -> bool {
    let (Some(base_anchor), Some(target_anchor)) = (
        canonical_existing_ancestor(base),
        canonical_existing_ancestor(target),
    ) else {
        // Without a real on-disk anchor on both sides, containment cannot be confirmed.
        return false;
    };
    target_anchor.starts_with(&base_anchor)
}

/// Canonicalize the deepest ancestor of `path` that exists on disk.
///
/// The path being written usually does not exist yet, so walk up until a component that
/// does, then canonicalize it. The not-yet-created tail carries no symlink, so only the
/// existing portion needs resolution. Returns `None` when no ancestor exists.
fn canonical_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            return Some(canonical);
        }
        existing = existing.parent()?;
    }
}

/// Resolve `rel_path` under `base`, returning `None` when it escapes `base`.
///
/// The resolution rejects an absolute `rel_path` and any `..` component that would climb
/// above `base`. It is lexical, so it does not touch the filesystem and holds for a target
/// that does not yet exist. A `.` component is dropped; a normal component descends.
fn resolve_under(base: &Path, rel_path: &Path) -> Option<PathBuf> {
    let mut resolved = base.to_path_buf();
    let mut depth: usize = 0;

    for component in rel_path.components() {
        match component {
            // An absolute path or a Windows prefix escapes the base.
            Component::RootDir | Component::Prefix(_) => return None,
            Component::CurDir => {}
            Component::ParentDir => {
                // A `..` may not climb above the base.
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                resolved.pop();
            }
            Component::Normal(part) => {
                depth += 1;
                resolved.push(part);
            }
        }
    }

    // The target must name a file under the base. A path that resolves back to the base
    // itself (for example `a/..`) is not a writable file target.
    if depth == 0 {
        return None;
    }

    Some(resolved)
}

/// Write `contents` to `path` atomically.
///
/// The write goes to a temp file in the same directory, then renames over the target. A
/// same-directory rename is atomic on the same filesystem, so a reader sees either the old
/// or the new file, never a partial one. On any failure the target is unchanged.
///
/// This is private to the write guard. Every runtime write reaches it only through
/// [`write_under_agent`] or [`write_repo_seed`], so the guard is the only door to the byte
/// write and cannot be bypassed by a direct call (ADR-6, #187).
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
// `#[path]` include keeps them a child module of `agent_ws`.
#[cfg(test)]
#[path = "agent_ws_tests.rs"]
mod tests;

// Property tests (Property 6) live in a separate sibling so the example-based unit tests
// stay focused. The `#[path]` include keeps them a child module of `agent_ws`.
#[cfg(test)]
#[path = "agent_ws_prop_tests.rs"]
mod prop_tests;
