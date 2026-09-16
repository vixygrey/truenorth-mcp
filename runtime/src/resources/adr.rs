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
    if !dir.is_dir() {
        return Err(ResourceReadError::NotFound("specs/adr/".to_string()));
    }

    // Collect the markdown ADR files, then sort by file name for a stable read order.
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| ResourceReadError::Invalid(format!("could not read specs/adr/: {e}")))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(ADR_EXTENSION))
                // Skip any file matching the secret denylist as defense in depth, so a
                // stray secret dropped under specs/adr/ is never served (Requirement 1.7).
                && !crate::config::is_secret_path(path)
        })
        .collect();
    files.sort();

    let mut sections = Vec::with_capacity(files.len());
    for path in &files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(path).map_err(|e| {
            ResourceReadError::Invalid(format!("could not read ADR file `specs/adr/{name}`: {e}"))
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
