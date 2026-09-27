use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MANIFEST_REL_PATH: &str = ".agent/workspace-manifest.yml";
pub const MANIFEST_VERSION: &str = "1";
pub const BUNDLE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: u32,
    pub bundle_version: String,
    pub workspace_schema_version: String,
    #[serde(default)]
    pub supported_from: Vec<String>,
    pub files: Vec<BundleFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFile {
    pub path: String,
    pub sha256: String,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub manifest_version: String,
    pub bundle_version: String,
    pub workspace_schema_version: String,
    pub profile: String,
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
        if self.schema_version != BUNDLE_SCHEMA_VERSION {
            return invalid(
                path,
                format!("unsupported schema version `{}`", self.schema_version),
            );
        }
        if self.bundle_version.trim().is_empty() || self.workspace_schema_version != "1" {
            return invalid(
                path,
                "bundle version must be nonempty and workspace schema must be `1`",
            );
        }
        let mut seen = BTreeSet::new();
        for file in &self.files {
            validate_record(path, &file.path, &file.sha256, &file.mode)?;
            if !seen.insert(file.path.clone()) {
                return invalid(path, format!("duplicate managed path `{}`", file.path));
            }
        }
        Ok(())
    }
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
        if self.manifest_version != MANIFEST_VERSION {
            return invalid(
                path,
                format!("unsupported manifest version `{}`", self.manifest_version),
            );
        }
        if self.bundle_version.trim().is_empty()
            || self.workspace_schema_version != "1"
            || self.profile.trim().is_empty()
        {
            return invalid(
                path,
                "bundle version and profile must be nonempty and workspace schema must be `1`",
            );
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

pub fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
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
