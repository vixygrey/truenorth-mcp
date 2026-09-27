use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::manifest::{BundleManifest, ManifestError, sha256, validate_relative_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub mode: String,
}

impl DesiredFile {
    pub fn source_hash(&self) -> String {
        sha256(&self.bytes)
    }
}

#[derive(Debug)]
pub struct PackageBundle {
    pub manifest: BundleManifest,
    pub files: BTreeMap<String, DesiredFile>,
    root: PathBuf,
}

#[derive(Debug, Error)]
pub enum BundleError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("invalid bundle file `{path}`: {detail}")]
    InvalidFile { path: String, detail: String },
    #[error("could not read bundle file `{path}`: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
}

impl PackageBundle {
    pub fn load(root: &Path) -> Result<Self, BundleError> {
        let manifest_path = root.join("bundle/current.json");
        let manifest = BundleManifest::read(&manifest_path)?;
        if manifest.bundle_version != env!("CARGO_PKG_VERSION") {
            return Err(BundleError::InvalidFile {
                path: manifest_path.display().to_string(),
                detail: format!(
                    "bundle version `{}` does not match runtime version `{}`",
                    manifest.bundle_version,
                    env!("CARGO_PKG_VERSION")
                ),
            });
        }
        let files = load_files(root, &manifest)?;
        Ok(Self {
            manifest,
            files,
            root: root.to_path_buf(),
        })
    }

    pub fn load_history(&self, version: &str) -> Result<Option<BundleManifest>, BundleError> {
        if !self
            .manifest
            .supported_from
            .iter()
            .any(|item| item == version)
        {
            return Ok(None);
        }
        let path = self
            .root
            .join("bundle/history")
            .join(format!("{version}.json"));
        Ok(Some(BundleManifest::read(&path)?))
    }
}

fn load_files(
    root: &Path,
    manifest: &BundleManifest,
) -> Result<BTreeMap<String, DesiredFile>, BundleError> {
    let mut files = BTreeMap::new();
    for record in &manifest.files {
        let relative =
            validate_relative_path(&record.path).map_err(|detail| BundleError::InvalidFile {
                path: record.path.clone(),
                detail,
            })?;
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|source| BundleError::Read {
            path: path.display().to_string(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(BundleError::InvalidFile {
                path: record.path.clone(),
                detail: "source must be a regular file, not a symlink".to_string(),
            });
        }
        let bytes = fs::read(&path).map_err(|source| BundleError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let actual_hash = sha256(&bytes);
        if actual_hash != record.sha256 {
            return Err(BundleError::InvalidFile {
                path: record.path.clone(),
                detail: format!(
                    "SHA-256 mismatch: expected {}, found {actual_hash}",
                    record.sha256
                ),
            });
        }
        let actual_mode = normalized_mode(&metadata);
        if actual_mode != record.mode {
            return Err(BundleError::InvalidFile {
                path: record.path.clone(),
                detail: format!(
                    "mode mismatch: expected {}, found {actual_mode}",
                    record.mode
                ),
            });
        }
        files.insert(
            record.path.clone(),
            DesiredFile {
                path: record.path.clone(),
                bytes,
                mode: record.mode.clone(),
            },
        );
    }
    Ok(files)
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
#[path = "bundle_tests.rs"]
mod tests;
