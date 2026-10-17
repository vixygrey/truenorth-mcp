//! Recoverable transaction for the explicit cockpit JSON migration.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::engine::digest::sha256;

const JOURNAL: &str = "runtime/cockpit-json-migration/transaction.json";

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

pub fn exists(root: &Path) -> bool {
    root.join(".agent").join(JOURNAL).is_file()
}

pub fn prepare(root: &Path, changes: Vec<Change>) -> Result<(), TransactionError> {
    if exists(root) {
        return Err(TransactionError::Exists);
    }
    let mut entries = Vec::with_capacity(changes.len());
    for change in changes {
        let path = root.join(&change.path);
        let before = match fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(TransactionError::Io {
                    path: change.path,
                    source,
                });
            }
        };
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
    let path = root.join(".agent").join(JOURNAL);
    fs::create_dir_all(path.parent().expect("journal parent")).map_err(|source| {
        TransactionError::Io {
            path: JOURNAL.to_string(),
            source,
        }
    })?;
    fs::write(&path, journal).map_err(|source| TransactionError::Io {
        path: JOURNAL.to_string(),
        source,
    })
}

pub fn resume(root: &Path) -> Result<(), TransactionError> {
    apply(root, false)
}
pub fn rollback(root: &Path) -> Result<(), TransactionError> {
    apply(root, true)
}

fn apply(root: &Path, rollback: bool) -> Result<(), TransactionError> {
    let journal_path = root.join(".agent").join(JOURNAL);
    let journal: Journal = serde_json::from_slice(&fs::read(&journal_path).map_err(|source| {
        TransactionError::Io {
            path: JOURNAL.to_string(),
            source,
        }
    })?)
    .map_err(|error| TransactionError::Corrupt(error.to_string()))?;
    for entry in journal.entries.iter().rev() {
        let expected = if rollback {
            &entry.after_hash
        } else {
            &entry.before_hash
        };
        let target = if rollback {
            &entry.before
        } else {
            &entry.after
        };
        let current = fs::read(root.join(&entry.path))
            .ok()
            .map(|bytes| sha256(&bytes));
        if current != *expected && current != target.as_deref().map(sha256) {
            return Err(TransactionError::Changed {
                path: entry.path.clone(),
            });
        }
        if let Some(bytes) = target {
            fs::write(root.join(&entry.path), bytes).map_err(|source| TransactionError::Io {
                path: entry.path.clone(),
                source,
            })?;
        } else {
            let _ = fs::remove_file(root.join(&entry.path));
        }
    }
    if rollback {
        fs::remove_file(journal_path).map_err(|source| TransactionError::Io {
            path: JOURNAL.to_string(),
            source,
        })?;
    }
    Ok(())
}
