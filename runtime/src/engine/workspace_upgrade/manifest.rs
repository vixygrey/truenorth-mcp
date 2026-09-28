use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::engine::digest::sha256;

pub const MANIFEST_REL_PATH: &str = ".agent/workspace-manifest.yml";
pub const MANIFEST_VERSION: &str = "2";
pub const BUNDLE_SCHEMA_VERSION: u32 = 2;
pub const CORE_SKILL_SET: &str = "core";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: u32,
    pub bundle_version: String,
    pub workspace_schema_version: String,
    #[serde(default)]
    pub supported_from: Vec<String>,
    pub files: Vec<BundleFile>,
    #[serde(default)]
    pub skill_sets: Vec<SkillSet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFile {
    pub path: String,
    pub sha256: String,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSet {
    pub name: String,
    pub support: String,
    pub description: String,
    pub prerequisites: String,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub manifest_version: String,
    pub bundle_version: String,
    pub workspace_schema_version: String,
    pub profile: String,
    #[serde(default)]
    pub skill_sets: Vec<String>,
    pub managed: BTreeMap<String, ManagedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedFile {
    pub source_sha256: String,
    pub mode: String,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("could not read manifest `{path}`: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("could not parse manifest `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("invalid manifest `{path}`: {detail}")]
    Invalid { path: String, detail: String },
}

impl BundleManifest {
    pub fn read(path: &Path) -> Result<Self, ManifestError> {
        let bytes = fs::read(path).map_err(|source| ManifestError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let value: Self =
            serde_json::from_slice(&bytes).map_err(|source| ManifestError::Parse {
                path: path.display().to_string(),
                detail: source.to_string(),
            })?;
        value.validate(path)?;
        Ok(value)
    }

    pub fn validate(&self, path: &Path) -> Result<(), ManifestError> {
        if !matches!(self.schema_version, 1 | BUNDLE_SCHEMA_VERSION) {
            return invalid(
                path,
                format!("unsupported schema version `{}`", self.schema_version),
            );
        }
        let expected_workspace_schema = if self.schema_version == 1 { "1" } else { "2" };
        if self.bundle_version.trim().is_empty()
            || self.workspace_schema_version != expected_workspace_schema
        {
            return invalid(
                path,
                format!(
                    "bundle version must be nonempty and workspace schema must be `{expected_workspace_schema}`"
                ),
            );
        }
        let mut seen = BTreeSet::new();
        for file in &self.files {
            validate_record(path, &file.path, &file.sha256, &file.mode)?;
            if !seen.insert(file.path.clone()) {
                return invalid(path, format!("duplicate managed path `{}`", file.path));
            }
        }
        if self.schema_version == 1 {
            if !self.skill_sets.is_empty() {
                return invalid(path, "bundle schema v1 cannot declare skill sets");
            }
            return Ok(());
        }
        validate_skill_sets(path, &self.skill_sets, &self.files)
    }

    pub fn skill_set(&self, name: &str) -> Option<&SkillSet> {
        self.skill_sets.iter().find(|set| set.name == name)
    }

    pub fn all_skill_sets(&self) -> Vec<String> {
        self.skill_sets.iter().map(|set| set.name.clone()).collect()
    }

    pub fn set_for_skill(&self, skill: &str) -> Option<&str> {
        self.skill_sets
            .iter()
            .find(|set| set.skills.iter().any(|candidate| candidate == skill))
            .map(|set| set.name.as_str())
    }
}

fn validate_skill_sets(
    path: &Path,
    sets: &[SkillSet],
    files: &[BundleFile],
) -> Result<(), ManifestError> {
    let mut set_names = BTreeSet::new();
    let mut skill_names = BTreeSet::new();
    let mut previous_set = None;
    for set in sets {
        validate_name(path, "skill set", &set.name)?;
        if previous_set.is_some_and(|previous| previous >= set.name.as_str()) {
            return invalid(path, "skill sets must be sorted by name without duplicates");
        }
        previous_set = Some(set.name.as_str());
        if !set_names.insert(set.name.as_str())
            || set.support.trim().is_empty()
            || set.description.trim().is_empty()
            || set.prerequisites.trim().is_empty()
            || set.skills.is_empty()
        {
            return invalid(
                path,
                format!("skill set `{}` is incomplete or duplicated", set.name),
            );
        }
        let mut previous_skill = None;
        for skill in &set.skills {
            validate_name(path, "skill", skill)?;
            if previous_skill.is_some_and(|previous| previous >= skill.as_str())
                || !skill_names.insert(skill.as_str())
            {
                return invalid(path, format!("skill `{skill}` is duplicated or unsorted"));
            }
            previous_skill = Some(skill.as_str());
        }
    }
    if !set_names.contains(CORE_SKILL_SET) {
        return invalid(path, "bundle schema v2 must declare the `core` skill set");
    }
    let file_skills: BTreeSet<&str> = files
        .iter()
        .filter_map(|file| {
            file.path
                .strip_prefix("skills/")
                .and_then(|rest| rest.split_once('/'))
                .map(|(skill, _)| skill)
        })
        .collect();
    if file_skills != skill_names {
        return invalid(
            path,
            "skill set membership must exactly match bundled skill directories",
        );
    }
    Ok(())
}

fn validate_name(path: &Path, kind: &str, value: &str) -> Result<(), ManifestError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (byte == b'-' && index > 0)
        });
    if !valid || value.ends_with('-') {
        return invalid(path, format!("invalid {kind} name `{value}`"));
    }
    Ok(())
}

impl WorkspaceManifest {
    pub fn read_optional(repo_root: &Path) -> Result<Option<Self>, ManifestError> {
        let path = repo_root.join(MANIFEST_REL_PATH);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(ManifestError::Read {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        let value: Self = serde_yaml::from_str(&text).map_err(|source| ManifestError::Parse {
            path: path.display().to_string(),
            detail: source.to_string(),
        })?;
        value.validate(&path)?;
        Ok(Some(value))
    }

    pub fn validate(&self, path: &Path) -> Result<(), ManifestError> {
        if !matches!(self.manifest_version.as_str(), "1" | MANIFEST_VERSION) {
            return invalid(
                path,
                format!("unsupported manifest version `{}`", self.manifest_version),
            );
        }
        let expected_workspace_schema = if self.manifest_version == "1" {
            "1"
        } else {
            "2"
        };
        if self.bundle_version.trim().is_empty()
            || self.workspace_schema_version != expected_workspace_schema
            || self.profile.trim().is_empty()
        {
            return invalid(
                path,
                format!(
                    "bundle version and profile must be nonempty and workspace schema must be `{expected_workspace_schema}`"
                ),
            );
        }
        if self.manifest_version == "1" {
            if !self.skill_sets.is_empty() {
                return invalid(path, "workspace manifest v1 cannot declare skill sets");
            }
        } else {
            let mut previous = None;
            for set in &self.skill_sets {
                validate_name(path, "skill set", set)?;
                if previous.is_some_and(|item| item >= set.as_str()) {
                    return invalid(
                        path,
                        "selected skill sets must be sorted without duplicates",
                    );
                }
                previous = Some(set.as_str());
            }
            if !self.skill_sets.iter().any(|set| set == CORE_SKILL_SET) {
                return invalid(
                    path,
                    "workspace manifest v2 must select the `core` skill set",
                );
            }
        }
        for (managed_path, file) in &self.managed {
            validate_record(path, managed_path, &file.source_sha256, &file.mode)?;
        }
        Ok(())
    }

    pub fn to_yaml(&self) -> Result<String, ManifestError> {
        serde_yaml::to_string(self).map_err(|source| ManifestError::Parse {
            path: MANIFEST_REL_PATH.to_string(),
            detail: source.to_string(),
        })
    }
}

pub fn validate_relative_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains('\\') {
        return Err("path must be a nonempty slash-separated relative path".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || value.starts_with(".agent/runtime/")
        || value == ".agent/runtime"
    {
        return Err(
            "path must stay under the repository and outside `.agent/runtime/`".to_string(),
        );
    }
    Ok(path.to_path_buf())
}

fn validate_record(
    manifest: &Path,
    path: &str,
    hash: &str,
    mode: &str,
) -> Result<(), ManifestError> {
    validate_relative_path(path).map_err(|detail| ManifestError::Invalid {
        path: manifest.display().to_string(),
        detail: format!("invalid managed path `{path}`: {detail}"),
    })?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid(manifest, format!("invalid SHA-256 for `{path}`"));
    }
    if !matches!(mode, "0644" | "0755") {
        return invalid(manifest, format!("unsupported mode `{mode}` for `{path}`"));
    }
    Ok(())
}

fn invalid<T>(path: &Path, detail: impl Into<String>) -> Result<T, ManifestError> {
    Err(ManifestError::Invalid {
        path: path.display().to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
