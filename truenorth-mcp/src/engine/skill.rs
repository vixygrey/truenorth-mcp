//! Skill discovery and file access (ports `paths.ts` and part of `skill-parser.ts`).
//!
//! A skill lives at `skills/<name>/SKILL.md` under the repository root. This module
//! discovers skills, resolves a skill name to its file under a path guard, and reads a
//! skill's raw markdown with a byte cap. The tier transforms and the parser build on the
//! raw markdown this module returns.
//!
//! Requirements: 6.1, 6.3, 7.1. Design: Part II §1.

// The skill helpers are consumed by the skills tools (task 8). They are unused until the
// tools wire them, so the module-scoped allow prevents a premature dead-code error under
// `clippy -D warnings`. Remove this allow once task 8b wires the last consumer.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::config::MAX_READ_SKILL_BYTES;
use crate::engine::phase::phase_for_skill;

/// A skill index entry (ports `SkillIndexEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillIndexEntry {
    /// The skill directory name.
    pub name: String,
    /// The repo-relative path to the skill's `SKILL.md`.
    pub path: PathBuf,
    /// The lifecycle phase for the skill.
    pub phase: String,
}

/// The raw markdown of a skill, with a flag for byte-cap truncation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSkill {
    /// The skill directory name.
    pub name: String,
    /// The repo-relative path to the skill's `SKILL.md`.
    pub path: PathBuf,
    /// The raw markdown, truncated to the byte cap when needed.
    pub markdown: String,
    /// Whether the content was truncated at the byte cap.
    pub truncated: bool,
}

/// An error resolving or reading a skill.
#[derive(Debug, Error)]
pub enum SkillError {
    /// The skill name is invalid or the path escapes the skills directory.
    #[error("Skill not found or invalid: {0}")]
    InvalidName(String),

    /// The skill name resolves to a path with no `SKILL.md`.
    #[error("Skill not found: {0}")]
    NotFound(String),

    /// The skill file could not be read.
    #[error("could not read skill `{name}`: {source}")]
    Read {
        /// The skill name.
        name: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

/// Discover every skill under `skills/`, sorted by name (ports `discoverSkills`).
///
/// A directory without a `SKILL.md` is skipped. When `skills/` is absent, the result is
/// empty.
pub fn discover_skills(repo_root: &Path) -> Vec<SkillIndexEntry> {
    let skills_dir = repo_root.join("skills");
    let Ok(entries) = std::fs::read_dir(&skills_dir) else {
        return Vec::new();
    };

    let mut skills: Vec<SkillIndexEntry> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let skill_file = entry.path().join("SKILL.md");
            if !skill_file.is_file() {
                return None;
            }
            Some(SkillIndexEntry {
                path: relative_to(repo_root, &skill_file),
                phase: phase_for_skill(&name),
                name,
            })
        })
        .collect();

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Resolve a skill name to its `SKILL.md` path under a path guard (ports
/// `resolveSkillPath`).
///
/// The name must be a single directory segment. A name with `/`, `\`, or `..`, or an
/// empty name, is rejected, so a caller cannot traverse outside `skills/`.
///
/// # Errors
///
/// Returns [`SkillError::InvalidName`] for a name that fails the guard.
pub fn resolve_skill_path(repo_root: &Path, name: &str) -> Result<PathBuf, SkillError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        return Err(SkillError::InvalidName(name.to_string()));
    }
    Ok(repo_root.join("skills").join(trimmed).join("SKILL.md"))
}

/// Read a skill's raw markdown with the byte cap (ports the read half of
/// `readSkillFile`).
///
/// The content is truncated to [`MAX_READ_SKILL_BYTES`] on a UTF-8 boundary when the file
/// is larger, and `truncated` reports it.
///
/// # Errors
///
/// Returns [`SkillError::InvalidName`] for a guarded name, [`SkillError::NotFound`] when
/// no `SKILL.md` exists, or [`SkillError::Read`] on an I/O failure.
pub fn read_skill_raw(repo_root: &Path, name: &str) -> Result<RawSkill, SkillError> {
    let path = resolve_skill_path(repo_root, name)?;
    if !path.is_file() {
        return Err(SkillError::NotFound(name.to_string()));
    }

    let bytes = std::fs::read(&path).map_err(|source| SkillError::Read {
        name: name.to_string(),
        source,
    })?;

    let (markdown, truncated) = cap_bytes(&bytes, MAX_READ_SKILL_BYTES);
    Ok(RawSkill {
        name: name.to_string(),
        path: relative_to(repo_root, &path),
        markdown,
        truncated,
    })
}

/// Cap a byte buffer to `max_bytes` on a UTF-8 boundary.
///
/// When the buffer is within the cap, the whole content is decoded lossily. When it
/// exceeds the cap, the cut backs off to the last valid UTF-8 boundary at or before the
/// cap, so the returned string is always valid.
fn cap_bytes(bytes: &[u8], max_bytes: usize) -> (String, bool) {
    if bytes.len() <= max_bytes {
        return (String::from_utf8_lossy(bytes).into_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !bytes.is_char_boundary_lossy(end) {
        end -= 1;
    }
    (String::from_utf8_lossy(&bytes[..end]).into_owned(), true)
}

/// The path of `target` relative to `repo_root`, or `target` itself when it is not under
/// the root.
fn relative_to(repo_root: &Path, target: &Path) -> PathBuf {
    target
        .strip_prefix(repo_root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| target.to_path_buf())
}

/// A byte-slice extension that reports a UTF-8 boundary. `str::is_char_boundary` needs a
/// `str`, so this checks the raw bytes for a non-continuation byte.
trait CharBoundary {
    fn is_char_boundary_lossy(&self, index: usize) -> bool;
}

impl CharBoundary for [u8] {
    fn is_char_boundary_lossy(&self, index: usize) -> bool {
        if index == 0 || index == self.len() {
            return true;
        }
        // A UTF-8 continuation byte has the top bits `10`. A boundary is any byte that is
        // not a continuation byte.
        self.get(index).is_some_and(|&b| (b & 0xC0) != 0x80)
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `skill`.
#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
