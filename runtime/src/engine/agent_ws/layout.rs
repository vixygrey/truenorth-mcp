//! The `.agent/` layout contract (Requirement 1.11, 1.12).
//!
//! [`read_layout`] parses `.agent/layout.yml` and confirms every required area and file is
//! present. [`LayoutCache`] retains the last valid contract, so a later broken read still
//! serves the last good state. The write guard lives in the parent module; this submodule
//! is the read-and-validate half of `.agent/`.
//!
//! Requirements: 1.5 through 1.9, 1.11, 1.12. Design: agent-workspace-profiles §1.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::Deserialize;
use thiserror::Error;

use super::AGENT_DIR;

/// The only `.agent/layout.yml` contract version supported by this runtime major version.
const SUPPORTED_LAYOUT_VERSION: &str = "1";

/// The versioned, language-agnostic layout contract.
///
/// The required entries remain fixed by the runtime. The document's `areas` section is
/// intentionally not deserialized here because it communicates the layout to other tools;
/// this runtime validates the required paths directly.
#[derive(Debug, Deserialize)]
struct LayoutContract {
    version: String,
}

/// The required areas and files under `.agent/` (Requirement 1.5 through 1.9).
///
/// [`read_layout`] confirms every entry here is present on disk. Each path is relative to
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

    /// The layout contract could not be parsed.
    #[error("could not parse the layout contract `{path}`: {source}")]
    Parse {
        /// The contract path.
        path: String,
        /// The YAML parse failure.
        source: serde_yaml::Error,
    },

    /// The layout contract declares a version this runtime does not support.
    #[error(
        "the agent workspace layout version `{actual}` is unsupported; \
         supported version: `{SUPPORTED_LAYOUT_VERSION}`."
    )]
    UnsupportedVersion {
        /// The version declared by the layout contract.
        actual: String,
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
        // Recover from a poisoned lock rather than panic. The cached layout is still
        // valid, so a prior panic elsewhere must not take down the layout read.
        *self
            .last_valid
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(layout.clone());
        Ok(layout)
    }

    /// The last valid layout, when one has been read successfully.
    ///
    /// Exercised by the server tests that assert `ServerContext::resolve` caches a complete
    /// contract (Requirement 1.12). The running server validates on resolve but does not
    /// read the cache back today, so this carries a non-test allow rather than deletion.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_valid(&self) -> Option<Layout> {
        // Recover from a poisoned lock rather than panic (see `read`).
        self.last_valid
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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

    let text = std::fs::read_to_string(&contract_path).map_err(|source| LayoutError::Io {
        path: contract_path.display().to_string(),
        source,
    })?;
    let contract: LayoutContract =
        serde_yaml::from_str(&text).map_err(|source| LayoutError::Parse {
            path: contract_path.display().to_string(),
            source,
        })?;
    if contract.version != SUPPORTED_LAYOUT_VERSION {
        return Err(LayoutError::UnsupportedVersion {
            actual: contract.version,
        });
    }

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

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `layout`, so they reach the private
// `REQUIRED_ENTRIES` and `EntryKind` through `use super::*`.
#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
