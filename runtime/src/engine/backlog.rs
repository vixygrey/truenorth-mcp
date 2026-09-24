//! Backlog ownership validation.
//!
//! Methodology and backlog storage are independent. A project can keep an authoritative
//! local list or point at an external tracker without caching remote issue state. The
//! runtime validates this optional declaration but never contacts the external provider.

use std::fs;
use std::path::Path;

use serde_yaml::{Mapping, Value};
use thiserror::Error;

const BACKLOG_PATH: &str = ".agent/tasks/backlog.yml";

/// An invalid optional backlog ownership declaration.
#[derive(Debug, Error)]
pub enum BacklogError {
    /// The backlog file could not be read.
    #[error("could not read `{path}`: {source}")]
    Io {
        /// The path that could not be read.
        path: String,
        /// The underlying filesystem error.
        source: std::io::Error,
    },
    /// The backlog file is not valid YAML.
    #[error("could not parse `{path}`: {source}")]
    Parse {
        /// The malformed file.
        path: String,
        /// The YAML parser error.
        source: serde_yaml::Error,
    },
    /// The ownership declaration violates its mode contract.
    #[error("invalid `{path}`: {message}")]
    Contract {
        /// The invalid file.
        path: String,
        /// The violated ownership rule and remediation.
        message: String,
    },
}

/// Validate the optional backlog ownership declaration under a repository root.
///
/// A missing file and a legacy file without `ownership` remain valid. Explicit local
/// ownership requires a backlog list. Explicit external ownership requires provider and
/// URL metadata and forbids a cached backlog field.
pub fn validate_optional(root: &Path) -> Result<(), BacklogError> {
    let path = root.join(BACKLOG_PATH);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(BacklogError::Io {
                path: path.display().to_string(),
                source,
            });
        }
    };
    let value: Value = serde_yaml::from_str(&content).map_err(|source| BacklogError::Parse {
        path: path.display().to_string(),
        source,
    })?;
    let document = value
        .as_mapping()
        .ok_or_else(|| contract(&path, "expected a YAML mapping"))?;
    let Some(ownership) = mapping(document, "ownership") else {
        return Ok(());
    };

    require_version(document, &path)?;
    match string(ownership, "mode") {
        Some("local") => validate_local(document, &path),
        Some("external") => validate_external(document, ownership, &path),
        Some(mode) => Err(contract(
            &path,
            &format!("ownership.mode is `{mode}`; expected `local` or `external`"),
        )),
        None => Err(contract(
            &path,
            "ownership.mode is required; set it to `local` or `external`",
        )),
    }
}

fn validate_local(document: &Mapping, path: &Path) -> Result<(), BacklogError> {
    match document.get(Value::from("backlog")) {
        Some(Value::Sequence(_)) => Ok(()),
        _ => Err(contract(path, "local ownership requires a `backlog` list")),
    }
}

fn validate_external(
    document: &Mapping,
    ownership: &Mapping,
    path: &Path,
) -> Result<(), BacklogError> {
    if document.contains_key(Value::from("backlog")) {
        return Err(contract(
            path,
            "external ownership must not contain a cached `backlog`; read current work from the provider",
        ));
    }
    require_nonempty(ownership, "provider", path)?;
    require_nonempty(ownership, "url", path)
}

fn require_version(document: &Mapping, path: &Path) -> Result<(), BacklogError> {
    match string(document, "version") {
        Some("1") => Ok(()),
        Some(version) => Err(contract(
            path,
            &format!("version is `{version}`; expected `1`"),
        )),
        None => Err(contract(
            path,
            "explicit ownership requires `version: \"1\"`",
        )),
    }
}

fn require_nonempty(mapping: &Mapping, key: &str, path: &Path) -> Result<(), BacklogError> {
    match string(mapping, key) {
        Some(value) if !value.trim().is_empty() => Ok(()),
        _ => Err(contract(
            path,
            &format!("external ownership requires a non-empty ownership.{key}"),
        )),
    }
}

fn mapping<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    mapping.get(Value::from(key))?.as_mapping()
}

fn string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping.get(Value::from(key))?.as_str()
}

fn contract(path: &Path, message: &str) -> BacklogError {
    BacklogError::Contract {
        path: path.display().to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn validate(content: &str) -> Result<(), BacklogError> {
        let root = TempDir::new().expect("temp repo");
        let tasks = root.path().join(".agent/tasks");
        fs::create_dir_all(&tasks).expect("create tasks");
        fs::write(tasks.join("backlog.yml"), content).expect("write backlog");
        validate_optional(root.path())
    }

    #[test]
    fn accepts_local_external_legacy_and_missing_backlogs() {
        let root = TempDir::new().expect("temp repo");
        assert!(validate_optional(root.path()).is_ok());
        assert!(validate("backlog: []\n").is_ok());
        assert!(validate("version: '1'\nownership:\n  mode: local\nbacklog: []\n").is_ok());
        assert!(
            validate(
                "version: '1'\nownership:\n  mode: external\n  provider: jira\n  url: https://example.invalid/issues\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_cached_entries_for_external_ownership() {
        let error = validate(
            "version: '1'\nownership:\n  mode: external\n  provider: github\n  url: https://example.invalid/issues\nbacklog: []\n",
        )
        .expect_err("external cache must fail");
        assert!(
            error
                .to_string()
                .contains("must not contain a cached `backlog`")
        );
    }

    #[test]
    fn rejects_incomplete_explicit_ownership() {
        assert!(validate("ownership:\n  mode: local\nbacklog: []\n").is_err());
        assert!(validate("version: '1'\nownership:\n  mode: local\n").is_err());
        assert!(
            validate("version: '1'\nownership:\n  mode: external\n  provider: github\n").is_err()
        );
        assert!(validate("version: '1'\nownership:\n  mode: shared\n").is_err());
    }
}
