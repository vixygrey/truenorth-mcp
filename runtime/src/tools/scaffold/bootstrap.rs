//! Fresh-project bootstrap built on the shared scaffold emission core.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::{Emission, resolve_scaffold_profile, result_json, scaffold_project, scaffold_sources};
use crate::engine::agent_ws::{write_repo_seed, write_repo_seed_bytes, write_under_agent};
use crate::engine::workspace_upgrade::bundle::PackageBundle;
use crate::engine::workspace_upgrade::manifest::{
    MANIFEST_VERSION, ManagedFile, WorkspaceManifest, sha256,
};

/// Seed a fresh project from an installed package bundle.
pub fn bootstrap_project(
    repo_root: &Path,
    profile_name: Option<&str>,
    bundle_root: &Path,
) -> Result<serde_json::Value, String> {
    let profile = resolve_scaffold_profile(profile_name).map_err(|error| error.to_string())?;
    validate_fresh_target(repo_root)?;
    let bundle = PackageBundle::load(bundle_root).map_err(|error| error.to_string())?;
    if !bundle.files.contains_key("skills/using-truenorth/SKILL.md") {
        return Err(format!(
            "bundle `{}` does not contain `skills/using-truenorth/SKILL.md`",
            bundle_root.display()
        ));
    }

    let generated = scaffold_sources(profile);
    let mut emissions = scaffold_project(repo_root, profile).map_err(|error| error.to_string())?;
    seed_specs_marker(repo_root, &mut emissions)?;
    copy_bundle_files(repo_root, &bundle, &mut emissions)?;

    let mut sources: BTreeMap<String, ManagedFile> = generated
        .into_iter()
        .map(|file| {
            (
                file.path,
                ManagedFile {
                    source_sha256: sha256(file.body.as_bytes()),
                    mode: "0644".to_string(),
                },
            )
        })
        .collect();
    sources.extend(bundle.files.values().map(|file| {
        (
            file.path.clone(),
            ManagedFile {
                source_sha256: file.source_hash(),
                mode: file.mode.clone(),
            },
        )
    }));
    let managed = emissions
        .iter()
        .filter_map(|emission| match emission {
            Emission::Wrote(path) => sources.get(path).cloned().map(|file| (path.clone(), file)),
            Emission::Skipped(_) => None,
        })
        .collect();
    let manifest = WorkspaceManifest {
        manifest_version: MANIFEST_VERSION.to_string(),
        bundle_version: bundle.manifest.bundle_version.clone(),
        workspace_schema_version: bundle.manifest.workspace_schema_version.clone(),
        profile: profile.name.to_string(),
        managed,
    };
    let yaml = manifest.to_yaml().map_err(|error| error.to_string())?;
    write_under_agent(repo_root, Path::new("workspace-manifest.yml"), &yaml)
        .map_err(|error| format!("could not seed `.agent/workspace-manifest.yml`: {error}"))?;
    emissions.push(Emission::Wrote(".agent/workspace-manifest.yml".to_string()));

    Ok(result_json(profile, &emissions))
}

const BOOTSTRAP_TARGETS: [&str; 7] = [
    ".agent",
    "specs",
    "skills",
    "AGENTS.md",
    "CONVENTIONS.md",
    ".githooks",
    ".github",
];

fn validate_fresh_target(repo_root: &Path) -> Result<(), String> {
    let conflicts: Vec<String> = BOOTSTRAP_TARGETS
        .iter()
        .filter(|rel| fs::symlink_metadata(repo_root.join(rel)).is_ok())
        .map(|rel| (*rel).to_string())
        .collect();
    if conflicts.is_empty() {
        return Ok(());
    }
    Err(format!(
        "bootstrap requires a fresh project; these paths already exist: {}",
        conflicts.join(", ")
    ))
}

fn seed_specs_marker(repo_root: &Path, out: &mut Vec<Emission>) -> Result<(), String> {
    const REL: &str = "specs/adr/.gitkeep";
    write_repo_seed(repo_root, Path::new(REL), "", true)
        .map_err(|error| format!("could not seed `{REL}`: {error}"))?;
    out.push(Emission::Wrote(REL.to_string()));
    Ok(())
}

fn copy_bundle_files(
    repo_root: &Path,
    bundle: &PackageBundle,
    out: &mut Vec<Emission>,
) -> Result<(), String> {
    for file in bundle.files.values() {
        let relative = Path::new(&file.path);
        write_repo_seed_bytes(repo_root, relative, &file.bytes, true)
            .map_err(|error| format!("could not seed `{}`: {error}", file.path))?;
        set_mode(&repo_root.join(relative), &file.mode)?;
        out.push(Emission::Wrote(file.path.clone()));
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(target: &Path, mode: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let value = if mode == "0755" { 0o755 } else { 0o644 };
    fs::set_permissions(target, fs::Permissions::from_mode(value)).map_err(|error| {
        format!(
            "could not preserve permissions for bundled file `{}`: {error}",
            target.display()
        )
    })
}

#[cfg(not(unix))]
fn set_mode(_target: &Path, _mode: &str) -> Result<(), String> {
    Ok(())
}
