//! Content revisions used for optimistic workspace writes.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// The observed content state of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedFile {
    path: PathBuf,
    revision: ContentRevision,
}

/// A content-derived file revision.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentRevision {
    Missing,
    Present([u8; 32]),
}

/// Every observation that must still hold when an atomic write commits.
#[derive(Debug, Clone)]
pub struct WritePrecondition {
    target: ObservedFile,
    dependencies: Vec<ObservedFile>,
}

/// A changed file found while validating a write precondition.
#[derive(Debug, Error)]
#[error(
    "stale write refused for `{target}`: `{changed}` changed from {expected} to {actual}. Read the current file and retry."
)]
pub struct RevisionConflict {
    /// The file that the caller intended to write.
    pub target: String,
    /// The observed source or destination that changed.
    pub changed: String,
    /// The revision captured with the read.
    pub expected: String,
    /// The current revision at commit time.
    pub actual: String,
}

#[derive(Debug, Error)]
pub(crate) enum RevisionCheckError {
    #[error(transparent)]
    Conflict(#[from] RevisionConflict),
    #[error("could not read `{path}` while checking a write revision: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

impl ObservedFile {
    /// Observe `path` without retaining its content.
    pub fn observe(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let revision = revision_for_path(&path)?;
        Ok(Self { path, revision })
    }

    /// Read UTF-8 content and capture the revision from those exact bytes.
    pub fn read_string(path: impl Into<PathBuf>) -> std::io::Result<(Option<String>, Self)> {
        let path = path.into();
        match std::fs::read(&path) {
            Ok(bytes) => {
                let revision = ContentRevision::from_bytes(&bytes);
                let text = String::from_utf8(bytes)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
                Ok((Some(text), Self { path, revision }))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((
                None,
                Self {
                    path,
                    revision: ContentRevision::Missing,
                },
            )),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl WritePrecondition {
    /// Require the destination to retain the observed revision.
    pub fn new(target: ObservedFile) -> Self {
        Self {
            target,
            dependencies: Vec::new(),
        }
    }

    /// Also require a source file to retain its observed revision.
    pub fn with_dependency(mut self, dependency: ObservedFile) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub(crate) fn target_path(&self) -> &Path {
        self.target.path()
    }

    pub(crate) fn verify(&self) -> Result<(), RevisionCheckError> {
        self.verify_one(&self.target)?;
        for dependency in &self.dependencies {
            self.verify_one(dependency)?;
        }
        Ok(())
    }

    fn verify_one(&self, expected: &ObservedFile) -> Result<(), RevisionCheckError> {
        let actual =
            revision_for_path(expected.path()).map_err(|source| RevisionCheckError::Io {
                path: expected.path.display().to_string(),
                source,
            })?;
        if actual == expected.revision {
            return Ok(());
        }
        Err(RevisionConflict {
            target: self.target.path.display().to_string(),
            changed: expected.path.display().to_string(),
            expected: expected.revision.display(),
            actual: actual.display(),
        }
        .into())
    }
}

impl ContentRevision {
    fn from_bytes(bytes: &[u8]) -> Self {
        let hash: [u8; 32] = Sha256::digest(bytes).into();
        Self::Present(hash)
    }

    fn display(&self) -> String {
        match self {
            Self::Missing => "missing".to_string(),
            Self::Present(hash) => format!("sha256:{}", encode_hex(hash)),
        }
    }
}

fn revision_for_path(path: &Path) -> std::io::Result<ContentRevision> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(ContentRevision::from_bytes(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ContentRevision::Missing),
        Err(error) => Err(error),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
