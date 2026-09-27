use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::bundle::DesiredFile;
use super::manifest::{MANIFEST_REL_PATH, sha256, validate_relative_path};
use super::plan::{PlanAction, UpgradePlan};
use crate::engine::agent_ws::{write_repo_seed_bytes, write_under_agent};

const TRANSACTION_REL: &str = "runtime/upgrade/transaction.json";
const TRANSACTION_DIR_REL: &str = "runtime/upgrade";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileState {
    sha256: String,
    mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Operation {
    path: String,
    before: Option<FileState>,
    after: Option<FileState>,
    backup: Option<String>,
    staged: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Journal {
    schema_version: u32,
    target_bundle_version: String,
    operations: Vec<Operation>,
}

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("could not prepare upgrade transaction: {0}")]
    Prepare(String),
    #[error("could not apply upgrade path `{path}`: {detail}")]
    Apply { path: String, detail: String },
    #[error(
        "upgrade path `{path}` changed outside the transaction; expected its preimage or postimage"
    )]
    UnexpectedState { path: String },
    #[error("could not roll back upgrade path `{path}`: {detail}")]
    Rollback { path: String, detail: String },
}

pub fn transaction_exists(repo_root: &Path) -> bool {
    repo_root.join(".agent").join(TRANSACTION_REL).is_file()
}

pub fn prepare(
    repo_root: &Path,
    plan: &UpgradePlan,
    desired: &std::collections::BTreeMap<String, DesiredFile>,
) -> Result<(), TransactionError> {
    let root = transaction_root(repo_root);
    if transaction_exists(repo_root) {
        return Err(TransactionError::Prepare(
            "an upgrade transaction already exists; resume it before preparing another".to_string(),
        ));
    }
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|error| TransactionError::Prepare(error.to_string()))?;
    }
    fs::create_dir_all(root.join("backup"))
        .and_then(|()| fs::create_dir_all(root.join("staged")))
        .map_err(|error| TransactionError::Prepare(error.to_string()))?;

    let mut operations = Vec::new();
    for entry in &plan.actions {
        if !matches!(
            entry.action,
            PlanAction::Add | PlanAction::Update | PlanAction::Remove
        ) {
            continue;
        }
        let index = operations.len();
        operations.push(stage_operation(
            repo_root,
            &root,
            index,
            &entry.path,
            desired.get(&entry.path),
        )?);
    }
    let manifest_yaml = plan
        .next_manifest
        .to_yaml()
        .map_err(|error| TransactionError::Prepare(error.to_string()))?;
    let manifest_desired = DesiredFile {
        path: MANIFEST_REL_PATH.to_string(),
        bytes: manifest_yaml.into_bytes(),
        mode: "0644".to_string(),
    };
    let index = operations.len();
    operations.push(stage_operation(
        repo_root,
        &root,
        index,
        MANIFEST_REL_PATH,
        Some(&manifest_desired),
    )?);

    let journal = Journal {
        schema_version: 1,
        target_bundle_version: plan.to_bundle_version.clone(),
        operations,
    };
    let text = serde_json::to_string_pretty(&journal)
        .map(|value| format!("{value}\n"))
        .map_err(|error| TransactionError::Prepare(error.to_string()))?;
    write_under_agent(repo_root, Path::new(TRANSACTION_REL), &text)
        .map_err(|error| TransactionError::Prepare(error.to_string()))
}

pub fn resume(repo_root: &Path) -> Result<(), TransactionError> {
    let journal = read_journal(repo_root)?;
    let root = transaction_root(repo_root);
    for operation in &journal.operations {
        let current = inspect(repo_root, &operation.path)?;
        if current == operation.after {
            continue;
        }
        if current != operation.before {
            return Err(TransactionError::UnexpectedState {
                path: operation.path.clone(),
            });
        }
        apply_operation(repo_root, &root, operation)?;
    }
    Ok(())
}

pub fn rollback(repo_root: &Path) -> Result<(), TransactionError> {
    let journal = read_journal(repo_root)?;
    let root = transaction_root(repo_root);
    for operation in journal.operations.iter().rev() {
        let current = inspect(repo_root, &operation.path)?;
        if current == operation.before {
            continue;
        }
        if current != operation.after {
            return Err(TransactionError::UnexpectedState {
                path: operation.path.clone(),
            });
        }
        restore_operation(repo_root, &root, operation)?;
    }
    cleanup(repo_root)
}

pub fn cleanup(repo_root: &Path) -> Result<(), TransactionError> {
    let root = transaction_root(repo_root);
    if !root.exists() {
        return Ok(());
    }
    fs::remove_dir_all(&root).map_err(|error| TransactionError::Prepare(error.to_string()))
}

fn stage_operation(
    repo_root: &Path,
    root: &Path,
    index: usize,
    path: &str,
    desired: Option<&DesiredFile>,
) -> Result<Operation, TransactionError> {
    validate_relative_path(path).map_err(TransactionError::Prepare)?;
    let before = inspect(repo_root, path)?;
    let backup = if before.is_some() {
        let name = format!("backup/{index}");
        fs::copy(repo_root.join(path), root.join(&name))
            .map_err(|error| TransactionError::Prepare(error.to_string()))?;
        Some(name)
    } else {
        None
    };
    let (after, staged) = if let Some(file) = desired {
        let name = format!("staged/{index}");
        fs::write(root.join(&name), &file.bytes)
            .map_err(|error| TransactionError::Prepare(error.to_string()))?;
        (
            Some(FileState {
                sha256: file.source_hash(),
                mode: file.mode.clone(),
            }),
            Some(name),
        )
    } else {
        (None, None)
    };
    Ok(Operation {
        path: path.to_string(),
        before,
        after,
        backup,
        staged,
    })
}

fn apply_operation(
    repo_root: &Path,
    root: &Path,
    operation: &Operation,
) -> Result<(), TransactionError> {
    if let Some(staged) = &operation.staged {
        let bytes = fs::read(root.join(staged)).map_err(|error| TransactionError::Apply {
            path: operation.path.clone(),
            detail: error.to_string(),
        })?;
        write_repo_seed_bytes(repo_root, Path::new(&operation.path), &bytes, true).map_err(
            |error| TransactionError::Apply {
                path: operation.path.clone(),
                detail: error.to_string(),
            },
        )?;
        set_mode(
            &repo_root.join(&operation.path),
            operation
                .after
                .as_ref()
                .map(|state| state.mode.as_str())
                .unwrap_or("0644"),
        )
        .map_err(|detail| TransactionError::Apply {
            path: operation.path.clone(),
            detail,
        })?;
    } else {
        fs::remove_file(repo_root.join(&operation.path)).map_err(|error| {
            TransactionError::Apply {
                path: operation.path.clone(),
                detail: error.to_string(),
            }
        })?;
    }
    Ok(())
}

fn restore_operation(
    repo_root: &Path,
    root: &Path,
    operation: &Operation,
) -> Result<(), TransactionError> {
    if let Some(backup) = &operation.backup {
        let bytes = fs::read(root.join(backup)).map_err(|error| TransactionError::Rollback {
            path: operation.path.clone(),
            detail: error.to_string(),
        })?;
        write_repo_seed_bytes(repo_root, Path::new(&operation.path), &bytes, true).map_err(
            |error| TransactionError::Rollback {
                path: operation.path.clone(),
                detail: error.to_string(),
            },
        )?;
        set_mode(
            &repo_root.join(&operation.path),
            operation
                .before
                .as_ref()
                .map(|state| state.mode.as_str())
                .unwrap_or("0644"),
        )
        .map_err(|detail| TransactionError::Rollback {
            path: operation.path.clone(),
            detail,
        })?;
    } else {
        fs::remove_file(repo_root.join(&operation.path)).map_err(|error| {
            TransactionError::Rollback {
                path: operation.path.clone(),
                detail: error.to_string(),
            }
        })?;
    }
    Ok(())
}

fn read_journal(repo_root: &Path) -> Result<Journal, TransactionError> {
    let path = repo_root.join(".agent").join(TRANSACTION_REL);
    let text =
        fs::read_to_string(&path).map_err(|error| TransactionError::Prepare(error.to_string()))?;
    let journal: Journal = serde_json::from_str(&text)
        .map_err(|error| TransactionError::Prepare(error.to_string()))?;
    if journal.schema_version != 1 {
        return Err(TransactionError::Prepare(format!(
            "unsupported transaction schema `{}`",
            journal.schema_version
        )));
    }
    Ok(journal)
}

fn inspect(repo_root: &Path, relative: &str) -> Result<Option<FileState>, TransactionError> {
    let path = repo_root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        Ok(_) => {
            return Err(TransactionError::UnexpectedState {
                path: relative.to_string(),
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(TransactionError::Prepare(error.to_string())),
    };
    let bytes = fs::read(&path).map_err(|error| TransactionError::Prepare(error.to_string()))?;
    Ok(Some(FileState {
        sha256: sha256(&bytes),
        mode: normalized_mode(&metadata),
    }))
}

fn transaction_root(repo_root: &Path) -> PathBuf {
    repo_root.join(".agent").join(TRANSACTION_DIR_REL)
}

#[cfg(unix)]
fn normalized_mode(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o111 == 0 {
        "0644".to_string()
    } else {
        "0755".to_string()
    }
}

#[cfg(not(unix))]
fn normalized_mode(_metadata: &fs::Metadata) -> String {
    "0644".to_string()
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let value = if mode == "0755" { 0o755 } else { 0o644 };
    fs::set_permissions(path, fs::Permissions::from_mode(value)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;
