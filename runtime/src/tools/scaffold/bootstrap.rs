//! Fresh-project bootstrap built on the shared scaffold emission core.
//!
//! The MCP scaffold serves an existing governed repository. This module adds the pre-server
//! bootstrap path: it validates a packaged skill bundle, seeds the language-agnostic workspace,
//! and copies every bundle asset into a fresh project without overwriting user files.

use std::fs;
use std::path::{Path, PathBuf};

use super::{Emission, resolve_scaffold_profile, result_json, scaffold_project};
use crate::engine::agent_ws::{write_repo_seed, write_repo_seed_bytes};

/// Seed a fresh project from `skills_source` and return the same summary shape as the MCP tool.
///
/// The target must not contain any bootstrap-owned path. Validating both the source bundle and
/// every destination before the first write keeps a malformed bundle or an existing project file
/// from producing a partial bootstrap.
pub fn bootstrap_project(
    repo_root: &Path,
    profile_name: Option<&str>,
    skills_source: &Path,
) -> Result<serde_json::Value, String> {
    let profile = resolve_scaffold_profile(profile_name).map_err(|error| error.to_string())?;
    validate_fresh_target(repo_root)?;
    validate_skill_bundle(skills_source)?;

    let mut emissions = scaffold_project(repo_root, profile).map_err(|error| error.to_string())?;
    seed_specs_marker(repo_root, &mut emissions)?;
    copy_skill_bundle(repo_root, skills_source, &mut emissions)?;

    Ok(result_json(profile, &emissions))
}

/// Bootstrap owns these paths. Rejecting them up front prevents an init command from silently
/// mixing generated workflow state with a user's existing state.
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

fn validate_skill_bundle(source: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        format!(
            "skills source `{}` is not a readable directory: {error}",
            source.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "skills source `{}` is not a readable directory",
            source.display()
        ));
    }
    if !source.join("using-truenorth").join("SKILL.md").is_file() {
        return Err(format!(
            "skills source `{}` does not contain `using-truenorth/SKILL.md`",
            source.display()
        ));
    }
    validate_tree(source, source)
}

fn validate_tree(root: &Path, dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|error| format!("could not read skills source `{}`: {error}", dir.display()))?
    {
        let entry = entry.map_err(|error| format!("could not inspect skills source: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect `{}`: {error}", path.display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "skills source `{}` contains unsupported symlink `{}`",
                root.display(),
                path.display()
            ));
        }
        if file_type.is_dir() {
            validate_tree(root, &path)?;
        } else if !file_type.is_file() {
            return Err(format!(
                "skills source `{}` contains unsupported entry `{}`",
                root.display(),
                path.display()
            ));
        }
    }
    Ok(())
}

fn seed_specs_marker(repo_root: &Path, out: &mut Vec<Emission>) -> Result<(), String> {
    const REL: &str = "specs/adr/.gitkeep";
    write_repo_seed(repo_root, Path::new(REL), "", true)
        .map_err(|error| format!("could not seed `{REL}`: {error}"))?;
    out.push(Emission::Wrote(REL.to_string()));
    Ok(())
}

fn copy_skill_bundle(
    repo_root: &Path,
    source: &Path,
    out: &mut Vec<Emission>,
) -> Result<(), String> {
    copy_directory(repo_root, source, source, out)
}

fn copy_directory(
    repo_root: &Path,
    source_root: &Path,
    source_dir: &Path,
    out: &mut Vec<Emission>,
) -> Result<(), String> {
    for entry in fs::read_dir(source_dir).map_err(|error| {
        format!(
            "could not read skills source `{}`: {error}",
            source_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| format!("could not inspect skills source: {error}"))?;
        let source_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect `{}`: {error}", source_path.display()))?;
        if file_type.is_dir() {
            copy_directory(repo_root, source_root, &source_path, out)?;
            continue;
        }

        let relative = source_path
            .strip_prefix(source_root)
            .map_err(|error| format!("could not resolve bundled skill path: {error}"))?;
        let target_rel = PathBuf::from("skills").join(relative);
        let target_display = target_rel.to_string_lossy().replace('\\', "/");
        let body = fs::read(&source_path).map_err(|error| {
            format!(
                "could not read bundled skill `{}`: {error}",
                source_path.display()
            )
        })?;
        write_repo_seed_bytes(repo_root, &target_rel, &body, true)
            .map_err(|error| format!("could not seed `{target_display}`: {error}"))?;
        preserve_permissions(&source_path, &repo_root.join(&target_rel))?;
        out.push(Emission::Wrote(target_display));
    }
    Ok(())
}

#[cfg(unix)]
fn preserve_permissions(source: &Path, target: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(source)
        .map_err(|error| {
            format!(
                "could not inspect bundled skill `{}`: {error}",
                source.display()
            )
        })?
        .permissions()
        .mode();
    fs::set_permissions(target, fs::Permissions::from_mode(mode)).map_err(|error| {
        format!(
            "could not preserve permissions for bundled skill `{}`: {error}",
            target.display()
        )
    })
}

#[cfg(not(unix))]
fn preserve_permissions(_source: &Path, _target: &Path) -> Result<(), String> {
    Ok(())
}
