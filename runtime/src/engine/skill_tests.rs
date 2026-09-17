//! Tests for skill discovery and file access (task 8a).
//!
//! Included from `skill.rs` via `#[path]`, so `super` is the skill module.
//!
//! Requirements: 6.1, 6.3, 7.1.

use super::*;
use std::fs;
use tempfile::tempdir;

/// Create a skill at `skills/<name>/SKILL.md` with the given body.
fn make_skill(root: &Path, name: &str, body: &str) {
    let dir = root.join("skills").join(name);
    fs::create_dir_all(&dir).expect("create skill dir");
    fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
}

#[test]
fn discovers_skills_sorted_by_name() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    make_skill(root, "verify-work", "# Verify\n");
    make_skill(root, "develop-tdd", "# TDD\n");
    // A directory without a SKILL.md is skipped.
    fs::create_dir_all(root.join("skills/empty-dir")).expect("empty dir");

    let skills = discover_skills(root);
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["develop-tdd", "verify-work"]);
    assert_eq!(skills[0].path, PathBuf::from("skills/develop-tdd/SKILL.md"));
    assert_eq!(skills[0].phase, "Build");
    assert_eq!(skills[1].phase, "Verify");
}

#[test]
fn discovers_nothing_when_skills_dir_absent() {
    let dir = tempdir().expect("temp dir");
    assert!(discover_skills(dir.path()).is_empty());
}

#[test]
fn resolve_rejects_traversal_names_as_path_escape() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    // A name with a separator or `..` is a traversal attempt, reported as PathEscape.
    for bad in ["../etc", "a/b", "a\\b", ".."] {
        assert!(
            matches!(
                resolve_skill_path(root, bad),
                Err(SkillError::PathEscape(_))
            ),
            "traversal name `{bad}` must be a PathEscape"
        );
    }
}

#[test]
fn resolve_rejects_an_empty_name_as_invalid() {
    let dir = tempdir().expect("temp dir");
    // An empty name is a plain validation failure, not a traversal attempt.
    let error = resolve_skill_path(dir.path(), "").expect_err("empty name");
    assert!(matches!(error, SkillError::InvalidName(_)));
}

#[test]
fn resolve_builds_the_skill_path() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let path = resolve_skill_path(root, "develop-tdd").expect("valid name");
    assert_eq!(path, root.join("skills/develop-tdd/SKILL.md"));
}

#[test]
fn read_raw_returns_markdown() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    make_skill(root, "develop-tdd", "# TDD\n\nBody text.\n");
    let raw = read_skill_raw(root, "develop-tdd").expect("read skill");
    assert_eq!(raw.name, "develop-tdd");
    assert!(raw.markdown.contains("# TDD"));
    assert!(!raw.truncated);
}

#[test]
fn read_raw_errors_on_missing_skill() {
    let dir = tempdir().expect("temp dir");
    let error = read_skill_raw(dir.path(), "absent").expect_err("missing skill");
    assert!(matches!(error, SkillError::NotFound(_)));
    let message = error.to_string();
    // The message names the offending skill, the expected path shape, and the remediation.
    assert!(message.contains("absent"), "names the skill: {message}");
    assert!(
        message.contains("skills/absent/SKILL.md"),
        "names the expected path: {message}"
    );
    assert!(
        message.contains("index_skills"),
        "gives a remediation hint: {message}"
    );
}

#[test]
fn read_raw_errors_on_a_traversal_name() {
    let dir = tempdir().expect("temp dir");
    let error = read_skill_raw(dir.path(), "../secret").expect_err("traversal name");
    assert!(matches!(error, SkillError::PathEscape(_)));
    // The message names the offending value and the boundary it protects.
    let message = error.to_string();
    assert!(message.contains("../secret"), "names the value: {message}");
    assert!(message.contains("skills/"), "names the boundary: {message}");
}

#[test]
fn read_raw_truncates_at_the_byte_cap() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    // A body larger than the cap. Use ASCII so byte length equals char count.
    let big = "x".repeat(MAX_READ_SKILL_BYTES + 100);
    make_skill(root, "big-skill", &big);
    let raw = read_skill_raw(root, "big-skill").expect("read big skill");
    assert!(raw.truncated);
    assert!(raw.markdown.len() <= MAX_READ_SKILL_BYTES);
}

#[test]
fn cap_bytes_backs_off_to_utf8_boundary() {
    // A multi-byte char straddling the cap must not split. `é` is two bytes.
    let text = "aé".repeat(10); // 30 bytes
    let (capped, truncated) = cap_bytes(text.as_bytes(), 4);
    assert!(truncated);
    // The cap at 4 bytes lands mid-`é` at byte 4; it backs off to a boundary.
    assert!(capped.is_char_boundary(capped.len()));
    assert!(capped.starts_with("aé"));
}
