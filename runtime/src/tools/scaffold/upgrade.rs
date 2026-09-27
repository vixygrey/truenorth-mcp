use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::scaffold_sources;
use crate::engine::agent_ws::read_layout;
use crate::engine::git;
use crate::engine::mutation::acquire_worktree_lease;
use crate::engine::profile;
use crate::engine::workspace_upgrade::bundle::{DesiredFile, PackageBundle};
use crate::engine::workspace_upgrade::manifest::{
    MANIFEST_VERSION, ManagedFile, WorkspaceManifest, sha256,
};
use crate::engine::workspace_upgrade::plan::{UpgradePlan, build_plan};
use crate::engine::workspace_upgrade::transaction;

pub fn plan_workspace_upgrade(repo_root: &Path, bundle_root: &Path) -> Result<UpgradePlan, String> {
    let bundle = PackageBundle::load(bundle_root).map_err(|error| error.to_string())?;
    let profile = profile::resolve_active(repo_root).map_err(|error| error.to_string())?;
    let installed = match WorkspaceManifest::read_optional(repo_root).map_err(|e| e.to_string())? {
        Some(manifest) => {
            if manifest.bundle_version != bundle.manifest.bundle_version
                && !bundle
                    .manifest
                    .supported_from
                    .iter()
                    .any(|version| version == &manifest.bundle_version)
            {
                return Err(format!(
                    "installed bundle version `{}` is not supported by target bundle `{}`",
                    manifest.bundle_version, bundle.manifest.bundle_version
                ));
            }
            manifest
        }
        None => adopt_legacy(repo_root, &bundle, profile)?,
    };
    let desired = desired_files(&bundle, profile);
    build_plan(
        repo_root,
        &desired,
        Some(&installed),
        &bundle.manifest.bundle_version,
        &bundle.manifest.workspace_schema_version,
        profile.name,
    )
    .map_err(|error| error.to_string())
}

pub fn apply_workspace_upgrade(
    repo_root: &Path,
    bundle_root: &Path,
) -> Result<UpgradePlan, String> {
    if transaction::transaction_exists(repo_root) {
        let _lease = acquire_worktree_lease(repo_root).map_err(|error| error.to_string())?;
        transaction::resume(repo_root).map_err(|error| error.to_string())?;
        if let Err(error) = validate_result(repo_root) {
            if let Err(rollback) = transaction::rollback(repo_root) {
                return Err(format!("{error}; rollback also failed: {rollback}"));
            }
            return Err(error);
        }
        transaction::cleanup(repo_root).map_err(|error| error.to_string())?;
        return plan_workspace_upgrade(repo_root, bundle_root);
    }

    let status = git::worktree_status(repo_root).map_err(|error| error.to_string())?;
    if !status.is_empty() {
        return Err("workspace upgrade requires a clean Git worktree and index".to_string());
    }
    let _lease = acquire_worktree_lease(repo_root).map_err(|error| error.to_string())?;

    let bundle = PackageBundle::load(bundle_root).map_err(|error| error.to_string())?;
    let profile = profile::resolve_active(repo_root).map_err(|error| error.to_string())?;
    let desired = desired_files(&bundle, profile);
    let plan = plan_workspace_upgrade(repo_root, bundle_root)?;
    transaction::prepare(repo_root, &plan, &desired).map_err(|error| error.to_string())?;
    if let Err(error) = transaction::resume(repo_root).map_err(|error| error.to_string()) {
        if let Err(rollback) = transaction::rollback(repo_root) {
            return Err(format!("{error}; rollback also failed: {rollback}"));
        }
        return Err(error);
    }
    if let Err(error) = validate_result(repo_root) {
        if let Err(rollback) = transaction::rollback(repo_root) {
            return Err(format!("{error}; rollback also failed: {rollback}"));
        }
        return Err(error);
    }
    transaction::cleanup(repo_root).map_err(|error| error.to_string())?;
    Ok(plan)
}

fn validate_result(repo_root: &Path) -> Result<(), String> {
    read_layout(repo_root).map_err(|error| error.to_string())?;
    WorkspaceManifest::read_optional(repo_root)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "upgrade completed without a workspace manifest".to_string())?;
    Ok(())
}

pub fn desired_files(
    bundle: &PackageBundle,
    profile: profile::Profile,
) -> BTreeMap<String, DesiredFile> {
    let mut desired = bundle.files.clone();
    desired.extend(scaffold_sources(profile).into_iter().map(|source| {
        let path = source.path;
        (
            path.clone(),
            DesiredFile {
                path,
                bytes: source.body.into_bytes(),
                mode: "0644".to_string(),
            },
        )
    }));
    desired
}

fn adopt_legacy(
    repo_root: &Path,
    bundle: &PackageBundle,
    profile: profile::Profile,
) -> Result<WorkspaceManifest, String> {
    let Some(version) = bundle.manifest.supported_from.first() else {
        return Err(
            "workspace has no `.agent/workspace-manifest.yml` and this bundle has no supported migration baseline"
                .to_string(),
        );
    };
    let history = bundle
        .load_history(version)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("bundle has no historical manifest for `{version}`"))?;
    let mut managed = BTreeMap::new();
    for record in history.files {
        let path = repo_root.join(&record.path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("could not inspect `{}`: {error}", record.path)),
        };
        let bytes = fs::read(&path)
            .map_err(|error| format!("could not read `{}`: {error}", record.path))?;
        if sha256(&bytes) == record.sha256 && normalized_mode(&metadata) == record.mode {
            managed.insert(
                record.path,
                ManagedFile {
                    source_sha256: record.sha256,
                    mode: record.mode,
                },
            );
        }
    }
    for source in scaffold_sources(profile) {
        let record = ManagedFile {
            source_sha256: sha256(source.body.as_bytes()),
            mode: "0644".to_string(),
        };
        adopt_exact(repo_root, &source.path, record, &mut managed)?;
    }
    Ok(WorkspaceManifest {
        manifest_version: MANIFEST_VERSION.to_string(),
        bundle_version: version.clone(),
        workspace_schema_version: history.workspace_schema_version,
        profile: profile.name.to_string(),
        managed,
    })
}

fn adopt_exact(
    repo_root: &Path,
    relative: &str,
    record: ManagedFile,
    managed: &mut BTreeMap<String, ManagedFile>,
) -> Result<(), String> {
    let path = repo_root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("could not inspect `{relative}`: {error}")),
    };
    let bytes = fs::read(&path).map_err(|error| format!("could not read `{relative}`: {error}"))?;
    if sha256(&bytes) == record.source_sha256 && normalized_mode(&metadata) == record.mode {
        managed.insert(relative.to_string(), record);
    }
    Ok(())
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
