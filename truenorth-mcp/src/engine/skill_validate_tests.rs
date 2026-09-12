//! Tests for the skill validator (task 8b).
//!
//! Included from `skill_validate.rs` via `#[path]`, so `super` is the skill_validate
//! module.
//!
//! Requirements: 7.1.

use super::*;
use crate::engine::skill::RawSkill;
use crate::engine::skill_parser::parse_skill;
use tempfile::tempdir;

fn parsed(name: &str, markdown: &str) -> crate::engine::skill_parser::ParsedSkill {
    let raw = RawSkill {
        name: name.to_string(),
        path: std::path::PathBuf::from(format!("skills/{name}/SKILL.md")),
        markdown: markdown.to_string(),
        truncated: false,
    };
    parse_skill(&raw)
}

/// Find a check by id.
fn check<'a>(report: &'a ValidationReport, id: &str) -> &'a ValidationCheck {
    report
        .checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("check `{id}` present"))
}

#[test]
fn valid_skill_passes_core_checks() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: TDD loop.\n---\n\n# TDD\n\nverify: `cargo test`\n",
    );
    let report = validate_skill(&skill, dir.path(), 40);
    assert!(
        report.pass,
        "a well-formed skill passes: {:?}",
        report.checks
    );
}

#[test]
fn flags_non_verb_noun_name() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "BadName",
        "---\nname: x\ndescription: y\n---\n\n# X\n\nverify: ok\n",
    );
    let report = validate_skill(&skill, dir.path(), 10);
    assert!(!check(&report, "verb-noun-naming").pass);
}

#[test]
fn flags_missing_frontmatter_and_verify() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed("develop-tdd", "# TDD\n\nNo frontmatter and no verify.\n");
    let report = validate_skill(&skill, dir.path(), 10);
    assert!(!check(&report, "frontmatter-name").pass);
    assert!(!check(&report, "frontmatter-description").pass);
    assert!(!check(&report, "verify-command").pass);
    assert!(!report.pass);
}

#[test]
fn flags_size_cap_over_150() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: y\n---\n\n# X\n\nverify: ok\n",
    );
    let report = validate_skill(&skill, dir.path(), 200);
    assert!(!check(&report, "size-cap").pass);
    assert!(check(&report, "size-cap").message.contains("200/150"));
}

#[test]
fn flags_broken_skill_link() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: y\n---\n\n# X\n\nSee [ref](skills/absent/SKILL.md). verify: ok\n",
    );
    let report = validate_skill(&skill, dir.path(), 10);
    let link_check = report
        .checks
        .iter()
        .find(|c| c.id.starts_with("link-"))
        .expect("a link check exists");
    assert!(!link_check.pass);
    assert!(link_check.message.contains("Broken link"));
}

#[test]
fn resolves_existing_skill_link() {
    let dir = tempdir().expect("temp dir");
    // Create the link target so the check resolves.
    let target = dir.path().join("skills/verify-work");
    std::fs::create_dir_all(&target).expect("create target dir");
    std::fs::write(target.join("SKILL.md"), "# Verify\n").expect("write target");

    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: y\n---\n\n# X\n\nSee [ref](skills/verify-work/SKILL.md). verify: ok\n",
    );
    let report = validate_skill(&skill, dir.path(), 10);
    let link_check = report
        .checks
        .iter()
        .find(|c| c.id == "link-verify-work")
        .expect("link check present");
    assert!(link_check.pass);
}
