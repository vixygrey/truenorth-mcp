//! The agent workspace guard: the single write path under `.agent/`, the telemetry
//! read exclusion, and the layout-contract read.
//!
//! The runtime writes only under `.agent/`. Every runtime write funnels through
//! [`write_under_agent`], which normalizes the target and rejects any path that escapes
//! `.agent/`, then delegates the byte write to the atomic `write_atomic` helper in
//! [`crate::engine::cockpit`]. This is the load-bearing invariant of the feature
//! (design ADR-6, Property 6). The scaffold is the one authorized exception: it seeds
//! files outside `.agent/` through the audited [`write_repo_seed`] path, which the
//! runtime tools never call.
//!
//! [`is_excluded_read`] reports the telemetry read exclusion. [`read_layout`] parses
//! `.agent/layout.yml` and confirms every required area and file is present, retaining
//! the last valid contract in a [`LayoutCache`] so a later broken read still serves the
//! last good state.
//!
//! Requirements: 1.1, 1.2, 1.3, 1.9, 1.10, 1.11, 1.12, 5.5, 5.6, 5.8, 5.9.
//! Design: agent-workspace-profiles §1, ADR-6.

// The guard is a public API surface consumed incrementally by later tasks: the cockpit
// relocation (task 4), the bug-reference tool (task 7), and the scaffold (task 9). The
// items are unused until those tasks wire them, so the module-scoped allow prevents a
// premature dead-code error under `clippy -D warnings`. Remove this allow once task 9
// wires the last consumer.
#![allow(dead_code)]

use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use thiserror::Error;

use crate::engine::cockpit::write_atomic;

/// The agent workspace directory name (Requirement 1.1).
pub const AGENT_DIR: &str = ".agent";

/// The telemetry area, excluded from agent reads (Requirement 1.9, 1.10).
pub const TELEMETRY_AREA: &str = "telemetry";

/// The required areas and files under `.agent/` (Requirement 1.5 through 1.9).
///
/// `read_layout` confirms every entry here is present on disk. Each path is relative to
/// the `.agent/` directory. Directories end without a trailing slash; the presence check
/// distinguishes a directory from a file by its declared kind.
const REQUIRED_ENTRIES: [(&str, EntryKind); 13] = [
    // config/ area (Requirement 1.5).
    ("config", EntryKind::Dir),
    ("config/rules.yml", EntryKind::File),
    // spec/ area (Requirement 1.6).
    ("spec", EntryKind::Dir),
    ("spec/requirements.md", EntryKind::File),
    // tasks/ area (Requirement 1.7).
    ("tasks", EntryKind::Dir),
    ("tasks/state.yml", EntryKind::File),
    // memories/ area (Requirement 1.8).
    ("memories", EntryKind::Dir),
    ("memories/lessons.md", EntryKind::File),
    ("memories/glossary.md", EntryKind::File),
    // telemetry/ area (Requirement 1.9). The area is required, its content is read-excluded.
    ("telemetry", EntryKind::Dir),
    ("telemetry/runs.yml", EntryKind::File),
    // The contract file itself and the active profile name (§1.1).
    ("layout.yml", EntryKind::File),
    ("profile.yml", EntryKind::File),
];

/// Whether a required layout entry is a directory or a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Dir,
    File,
}

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

/// An error from the layout-contract read (Requirement 1.12).
#[derive(Debug, Error)]
pub enum LayoutError {
    /// The contract file `.agent/layout.yml` could not be read.
    #[error("could not read the layout contract `{path}`: {source}")]
    Io {
        /// The contract path.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// A required area or file is absent (Requirement 1.12).
    #[error(
        "the agent workspace layout is incomplete: `{absent}` is absent. \
         The `.agent/` layout must contain the config, spec, tasks, memories, and \
         telemetry areas with their required files."
    )]
    MissingEntry {
        /// The absent path, relative to the repository root.
        absent: String,
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

/// The last-valid layout contract, retained in memory (Requirement 1.12).
///
/// This mirrors the `ResourceCache` last-good pattern in `resources/mod.rs`. A successful
/// [`read_layout`] stores the validated contract. A later read that fails validation
/// leaves the stored contract intact, so the runtime keeps the last valid state.
#[derive(Debug, Default)]
pub struct LayoutCache {
    last_valid: Mutex<Option<Layout>>,
}

impl LayoutCache {
    /// A fresh, empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read and validate the layout, updating the cache on success (Requirement 1.12).
    ///
    /// On a failed read or validation the cache is left intact, so [`last_valid`] still
    /// returns the last valid contract.
    ///
    /// # Errors
    ///
    /// Returns the [`LayoutError`] from [`read_layout`]. The cache is left intact on error.
    ///
    /// [`last_valid`]: LayoutCache::last_valid
    pub fn read(&self, repo_root: &Path) -> Result<Layout, LayoutError> {
        let layout = read_layout(repo_root)?;
        *self.last_valid.lock().expect("layout cache lock") = Some(layout.clone());
        Ok(layout)
    }

    /// The last valid layout, when one has been read successfully.
    pub fn last_valid(&self) -> Option<Layout> {
        self.last_valid.lock().expect("layout cache lock").clone()
    }
}

/// The validated agent workspace layout (Requirement 1.11, 1.12).
///
/// The contract itself is language-agnostic data at `.agent/layout.yml`. The struct holds
/// the resolved agent-workspace root, so a caller reads a validated handle rather than a
/// raw path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The `.agent/` directory, resolved under the repository root.
    pub agent_root: PathBuf,
}

/// Parse `.agent/layout.yml` and confirm every required area and file is present.
///
/// The contract is language-agnostic data (Requirement 1.11). Any reader in any language
/// can parse it. On a missing required entry this returns [`LayoutError::MissingEntry`]
/// naming the absent path (Requirement 1.12). The caller retains the last valid contract
/// through [`LayoutCache`].
///
/// # Errors
///
/// Returns [`LayoutError::Io`] when the contract file cannot be read, and
/// [`LayoutError::MissingEntry`] when a required area or file is absent.
pub fn read_layout(repo_root: &Path) -> Result<Layout, LayoutError> {
    let agent_root = repo_root.join(AGENT_DIR);
    let contract_path = agent_root.join("layout.yml");

    // Read the contract so a reader in any language could parse the same bytes. The read
    // also confirms the contract file itself is present.
    std::fs::read_to_string(&contract_path).map_err(|source| LayoutError::Io {
        path: contract_path.display().to_string(),
        source,
    })?;

    for (entry, kind) in REQUIRED_ENTRIES {
        let path = agent_root.join(entry);
        let present = match kind {
            EntryKind::Dir => path.is_dir(),
            EntryKind::File => path.is_file(),
        };
        if !present {
            return Err(LayoutError::MissingEntry {
                absent: format!("{AGENT_DIR}/{entry}"),
            });
        }
    }

    Ok(Layout { agent_root })
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
