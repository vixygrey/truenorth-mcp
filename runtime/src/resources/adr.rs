//! ADR resource: `truenorth://adr`, a read-only view over `specs/adr/`.
//!
//! The resource concatenates the human-authored ADR files under `specs/adr/` on a
//! `resources/read`, in a stable filename order (Requirement 9.2). The directory is
//! human-authored and git-tracked (Requirement 9.1). The resource is read-only: the
//! write guard rejects any `specs/` target, and the server exposes no write path for it
//! (Requirement 9.4).
//!
//! An absent `specs/adr/` directory returns a read error naming the directory, and the
//! other resources keep serving (Requirement 9.5). A file that fails to read returns a
//! read error naming the file, and the last successfully read content is retained in the
//! resource cache (Requirement 9.6).
//!
//! Requirements: 9.1, 9.2, 9.4, 9.5, 9.6. Design: agent-workspace-profiles §8.

use std::path::Path;

use crate::engine::agent_ws::fs_boundary::RepositoryFs;

use super::ResourceReadError;

/// The file extension the ADR reader includes.
const ADR_EXTENSION: &str = "md";

/// Read and concatenate the ADR files under `dir` in stable filename order.
///
/// # Errors
///
/// Returns [`ResourceReadError::NotFound`] when `dir` is absent (Requirement 9.5), and
/// [`ResourceReadError::Invalid`] when a file under `dir` cannot be read (Requirement
/// 9.6).
pub fn read_adr_dir(dir: &Path) -> Result<String, ResourceReadError> {
    let root = dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| ResourceReadError::Invalid("invalid specs/adr/ path".to_string()))?;
    let fs = RepositoryFs::open(root)
        .map_err(|e| ResourceReadError::Invalid(format!("could not open repository: {e}")))?;
    let relative_dir = Path::new("specs/adr");
    fs.validate_directory(relative_dir)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ResourceReadError::NotFound("specs/adr/".to_string()),
            _ => ResourceReadError::Invalid(format!("could not read specs/adr/: {e}")),
        })?;

    // Sort eligible filenames before opening them; every entry is checked by the same
    // descriptor-relative guard as a direct resource read.
    let mut files = fs
        .read_dir_names(relative_dir)
        .map_err(|e| ResourceReadError::Invalid(format!("could not list specs/adr/: {e}")))?;
    files.retain(|name| {
        Path::new(name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(ADR_EXTENSION))
    });
    files.sort();

    let mut sections = Vec::with_capacity(files.len());
    for name in files {
        let relative = relative_dir.join(&name);
        let bytes = fs.read(&relative).map_err(|e| {
            ResourceReadError::Invalid(format!(
                "could not read ADR file `{}`: {e}",
                relative.display()
            ))
        })?;
        let text = String::from_utf8(bytes).map_err(|e| {
            ResourceReadError::Invalid(format!(
                "could not read ADR file `{}`: {e}",
                relative.display()
            ))
        })?;
        sections.push(text);
    }

    Ok(sections.join("\n\n"))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `adr`.
#[cfg(test)]
#[path = "adr_tests.rs"]
mod tests;
