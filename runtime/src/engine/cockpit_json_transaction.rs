//! Recoverable transaction for the explicit cockpit JSON migration.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::engine::agent_ws::fs_boundary::RepositoryFs;
use crate::engine::digest::sha256;

const JOURNAL: &str = ".agent/runtime/cockpit-json-migration/transaction.json";
const JOURNAL_PARENT: &str = ".agent/runtime/cockpit-json-migration";

#[derive(Debug, Clone)]
pub struct Change {
    pub path: String,
    pub after: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    path: String,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
    before_hash: Option<String>,
    after_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    entries: Vec<Entry>,
}

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("cockpit migration transaction already exists")]
    Exists,
    #[error("could not access `{path}`: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("cockpit migration transaction is corrupt: {0}")]
    Corrupt(String),
    #[error("`{path}` changed outside the cockpit migration transaction")]
    Changed { path: String },
}

pub fn prepare(root: &Path, changes: Vec<Change>) -> Result<(), TransactionError> {
    let fs = RepositoryFs::open(root).map_err(|source| io_error(".", source))?;
    match fs.read(Path::new(JOURNAL)) {
        Ok(_) => return Err(TransactionError::Exists),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(io_error(JOURNAL, source)),
    }
    for (index, change) in changes.iter().enumerate() {
        validate_target(&change.path)?;
        if changes[..index]
            .iter()
            .any(|previous| previous.path == change.path)
        {
            return Err(TransactionError::Corrupt(format!(
                "duplicate target `{}`",
                change.path
            )));
        }
    }
    let mut entries = Vec::with_capacity(changes.len());
    for change in changes {
        let before = read_optional(&fs, &change.path)?;
        entries.push(Entry {
            path: change.path,
            before_hash: before.as_deref().map(sha256),
            after_hash: change.after.as_deref().map(sha256),
            before,
            after: change.after,
        });
    }
    let journal = serde_json::to_vec_pretty(&Journal { entries })
        .map_err(|error| TransactionError::Corrupt(error.to_string()))?;
    fs.create_dir_all(Path::new(JOURNAL_PARENT))
        .map_err(|source| io_error(JOURNAL_PARENT, source))?;
    fs.write_new(Path::new(JOURNAL), &journal)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                TransactionError::Exists
            } else {
                io_error(JOURNAL, source)
            }
        })
}

pub fn resume(root: &Path) -> Result<(), TransactionError> {
    apply(root, false)
}
pub fn rollback(root: &Path) -> Result<(), TransactionError> {
    apply(root, true)
}

fn apply(root: &Path, rollback: bool) -> Result<(), TransactionError> {
    let fs = RepositoryFs::open(root).map_err(|source| io_error(".", source))?;
    let journal: Journal = serde_json::from_slice(
        &fs.read(Path::new(JOURNAL))
            .map_err(|source| io_error(JOURNAL, source))?,
    )
    .map_err(|error| TransactionError::Corrupt(error.to_string()))?;

    for (index, entry) in journal.entries.iter().enumerate() {
        validate_target(&entry.path)?;
        if journal.entries[..index]
            .iter()
            .any(|previous| previous.path == entry.path)
        {
            return Err(TransactionError::Corrupt(format!(
                "duplicate target `{}`",
                entry.path
            )));
        }
        validate_revision(&entry.path, "before", &entry.before, &entry.before_hash)?;
        validate_revision(&entry.path, "after", &entry.after, &entry.after_hash)?;
    }
    let mut pending = Vec::with_capacity(journal.entries.len());
    for entry in &journal.entries {
        let current = read_optional(&fs, &entry.path)?;
        let current_hash = current.as_deref().map(sha256);
        if current_hash != entry.before_hash && current_hash != entry.after_hash {
            return Err(TransactionError::Changed {
                path: entry.path.clone(),
            });
        }
        let target_hash = if rollback {
            &entry.before_hash
        } else {
            &entry.after_hash
        };
        pending.push(current_hash != *target_hash);
    }

    for (entry, needs_change) in journal.entries.iter().zip(pending).rev() {
        if !needs_change {
            continue;
        }
        let target = if rollback {
            &entry.before
        } else {
            &entry.after
        };
        if let Some(bytes) = target {
            fs.write_atomic(Path::new(&entry.path), bytes)
                .map_err(|source| io_error(&entry.path, source))?;
        } else {
            fs.remove_file(Path::new(&entry.path))
                .map_err(|source| io_error(&entry.path, source))?;
        }
    }
    if rollback {
        fs.remove_file(Path::new(JOURNAL))
            .map_err(|source| io_error(JOURNAL, source))?;
    }
    Ok(())
}

fn validate_target(path: &str) -> Result<(), TransactionError> {
    RepositoryFs::validate_path(Path::new(path))
        .map_err(|error| TransactionError::Corrupt(format!("invalid target `{path}`: {error}")))?;
    if !matches!(
        path,
        ".agent/tasks/state.yml"
            | ".agent/tasks/state.json"
            | ".agent/tasks/release-plan.yml"
            | ".agent/tasks/release-plan.json"
            | ".agent/tasks/bugs.yml"
            | ".agent/tasks/bugs.json"
    ) {
        return Err(TransactionError::Corrupt(format!(
            "unsupported target `{path}`"
        )));
    }
    Ok(())
}

fn validate_revision(
    path: &str,
    label: &str,
    bytes: &Option<Vec<u8>>,
    hash: &Option<String>,
) -> Result<(), TransactionError> {
    if bytes.as_deref().map(sha256).as_ref() != hash.as_ref() {
        return Err(TransactionError::Corrupt(format!(
            "invalid {label} hash for `{path}`"
        )));
    }
    Ok(())
}

fn read_optional(fs: &RepositoryFs, path: &str) -> Result<Option<Vec<u8>>, TransactionError> {
    fs.validate_directory(Path::new(".agent/tasks"))
        .map_err(|source| io_error(".agent/tasks", source))?;
    match fs.read(Path::new(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(io_error(path, source)),
    }
}

fn io_error(path: &str, source: io::Error) -> TransactionError {
    TransactionError::Io {
        path: path.to_string(),
        source,
    }
}

#[cfg(test)]
#[path = "cockpit_json_transaction_tests.rs"]
mod tests;
