use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use super::bundle::DesiredFile;
use super::manifest::{MANIFEST_VERSION, ManagedFile, WorkspaceManifest, sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanAction {
    Add,
    Update,
    Preserve,
    Conflict,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanEntry {
    pub path: String,
    pub action: PlanAction,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanCounts {
    pub add: usize,
    pub update: usize,
    pub preserve: usize,
    pub conflict: usize,
    pub remove: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpgradePlan {
    pub schema_version: u32,
    pub from_bundle_version: Option<String>,
    pub to_bundle_version: String,
    pub workspace_schema_version: String,
    pub actions: Vec<PlanEntry>,
    pub counts: PlanCounts,
    #[serde(skip)]
    pub next_manifest: WorkspaceManifest,
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("could not inspect workspace path `{path}`: {source}")]
    Inspect {
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug)]
struct LocalFile {
    hash: String,
    mode: String,
    regular: bool,
}

pub fn build_plan(
    repo_root: &Path,
    desired: &BTreeMap<String, DesiredFile>,
    installed: Option<&WorkspaceManifest>,
    target_bundle_version: &str,
    workspace_schema_version: &str,
    profile: &str,
    skill_sets: Vec<String>,
) -> Result<UpgradePlan, PlanError> {
    let old_managed = installed
        .map(|manifest| &manifest.managed)
        .cloned()
        .unwrap_or_default();
    let paths: BTreeSet<String> = desired.keys().chain(old_managed.keys()).cloned().collect();
    let mut actions = Vec::with_capacity(paths.len());
    let mut next_managed = BTreeMap::new();

    for path in paths {
        let local = inspect_local(repo_root, &path)?;
        let old = old_managed.get(&path);
        let new = desired.get(&path);
        let entry = classify(&path, old, local.as_ref(), new);

        if let Some(new_file) = new {
            let owned_before = old.is_some();
            let unowned_collision = !owned_before && local.is_some();
            if !unowned_collision {
                next_managed.insert(
                    path.clone(),
                    ManagedFile {
                        source_sha256: new_file.source_hash(),
                        mode: new_file.mode.clone(),
                    },
                );
            }
        }
        actions.push(entry);
    }

    let counts = PlanCounts {
        add: count(&actions, PlanAction::Add),
        update: count(&actions, PlanAction::Update),
        preserve: count(&actions, PlanAction::Preserve),
        conflict: count(&actions, PlanAction::Conflict),
        remove: count(&actions, PlanAction::Remove),
    };
    Ok(UpgradePlan {
        schema_version: 1,
        from_bundle_version: installed.map(|manifest| manifest.bundle_version.clone()),
        to_bundle_version: target_bundle_version.to_string(),
        workspace_schema_version: workspace_schema_version.to_string(),
        actions,
        counts,
        next_manifest: WorkspaceManifest {
            manifest_version: MANIFEST_VERSION.to_string(),
            bundle_version: target_bundle_version.to_string(),
            workspace_schema_version: workspace_schema_version.to_string(),
            profile: profile.to_string(),
            skill_sets,
            managed: next_managed,
        },
    })
}

fn classify(
    path: &str,
    old: Option<&ManagedFile>,
    local: Option<&LocalFile>,
    new: Option<&DesiredFile>,
) -> PlanEntry {
    let (action, reason) = match (old, local, new) {
        (None, None, Some(_)) => (PlanAction::Add, "new_managed_path"),
        (None, Some(_), Some(_)) => (PlanAction::Preserve, "unowned_local_path"),
        (Some(_), None, None) => (PlanAction::Remove, "already_absent"),
        (Some(old), Some(local), None) if matches_source(old, local) => {
            (PlanAction::Remove, "removed_unchanged_managed_file")
        }
        (Some(_), Some(_), None) => (PlanAction::Conflict, "removed_locally_modified_file"),
        (Some(old), None, Some(new)) if old.source_sha256 == new.source_hash() => {
            (PlanAction::Preserve, "local_deletion")
        }
        (Some(_), None, Some(_)) => (PlanAction::Conflict, "local_deletion_and_bundle_change"),
        (Some(old), Some(local), Some(new)) if !local.regular => {
            let _ = (old, new);
            (PlanAction::Conflict, "managed_path_is_not_a_regular_file")
        }
        (Some(old), Some(local), Some(new)) if local_matches(new, local) => {
            if old.source_sha256 == new.source_hash() && old.mode == new.mode {
                (PlanAction::Preserve, "unchanged_managed_file")
            } else {
                (PlanAction::Preserve, "already_matches_new_source")
            }
        }
        (Some(old), Some(local), Some(new)) if matches_source(old, local) => {
            let _ = new;
            (PlanAction::Update, "unchanged_locally_bundle_changed")
        }
        (Some(old), Some(_), Some(new))
            if old.source_sha256 == new.source_hash() && old.mode == new.mode =>
        {
            (PlanAction::Preserve, "locally_modified_bundle_unchanged")
        }
        (Some(_), Some(_), Some(_)) => (PlanAction::Conflict, "local_and_bundle_changed"),
        (None, _, None) => (PlanAction::Preserve, "unmanaged_absent_path"),
    };
    PlanEntry {
        path: path.to_string(),
        action,
        reason,
    }
}

fn matches_source(old: &ManagedFile, local: &LocalFile) -> bool {
    local.regular && local.hash == old.source_sha256 && local.mode == old.mode
}

fn local_matches(new: &DesiredFile, local: &LocalFile) -> bool {
    local.regular && local.hash == new.source_hash() && local.mode == new.mode
}

fn inspect_local(repo_root: &Path, relative: &str) -> Result<Option<LocalFile>, PlanError> {
    let path = repo_root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PlanError::Inspect {
                path: relative.to_string(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(Some(LocalFile {
            hash: String::new(),
            mode: String::new(),
            regular: false,
        }));
    }
    let bytes = fs::read(&path).map_err(|source| PlanError::Inspect {
        path: relative.to_string(),
        source,
    })?;
    Ok(Some(LocalFile {
        hash: sha256(&bytes),
        mode: normalized_mode(&metadata),
        regular: true,
    }))
}

fn count(actions: &[PlanEntry], action: PlanAction) -> usize {
    actions
        .iter()
        .filter(|entry| entry.action == action)
        .count()
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

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
