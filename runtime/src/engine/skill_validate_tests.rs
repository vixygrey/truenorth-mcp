//! Tests for the skill validator (task 8b).
//!
//! Included from `skill_validate.rs` via `#[path]`, so `super` is the skill_validate
//! module.
//!
//! Requirements: 7.1.

use super::*;
use crate::engine::skill::{RawSkill, discover_skills, read_skill_raw};
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
        "---\nname: develop-tdd\ndescription: TDD loop.\nkind: scripted\nverify: cargo test\n---\n\n# TDD\n",
    );
    let report = validate_skill(&skill, dir.path(), 40);
    assert!(
        report.pass,
        "a well-formed skill passes: {:?}",
        report.checks
    );
}

#[test]
fn prose_skill_needs_kind_but_not_a_verify_command() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: TDD loop.\nkind: prose\n---\n\n# TDD\n",
    );
    let report = validate_skill(&skill, dir.path(), 40);
    assert!(report.pass, "prose skills do not require a shell command");
    assert!(check(&report, "skill-kind").pass);
    assert!(check(&report, "verify-command").pass);
}

#[test]
fn scripted_skill_requires_frontmatter_verify_command() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: TDD loop.\nkind: scripted\n---\n\n# TDD\n\nverify: cargo test\n",
    );
    let report = validate_skill(&skill, dir.path(), 40);
    assert!(!check(&report, "verify-command").pass);
    assert!(!report.pass);
}

#[test]
fn documented_name_exception_is_supported() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "context7-mcp",
        "---\nname: context7-mcp\ndescription: External documentation service.\nkind: prose\nname_exception: External service name.\n---\n\n# Context7\n",
    );
    let report = validate_skill(&skill, dir.path(), 40);
    assert!(report.pass, "a documented name exception is valid");
    assert!(check(&report, "verb-noun-naming").pass);
}

#[test]
fn flags_non_verb_noun_name() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "BadName",
        "---\nname: BadName\ndescription: y\nkind: prose\n---\n\n# X\n",
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
    assert!(!check(&report, "skill-kind").pass);
    assert!(!check(&report, "verify-command").pass);
    assert!(!report.pass);
}

#[test]
fn flags_size_cap_over_150() {
    let dir = tempdir().expect("temp dir");
    let skill = parsed(
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: y\nkind: prose\n---\n\n# X\n",
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
        "---\nname: develop-tdd\ndescription: y\nkind: prose\n---\n\n# X\n\nSee [ref](skills/absent/SKILL.md).",
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
        "---\nname: develop-tdd\ndescription: y\nkind: prose\n---\n\n# X\n\nSee [ref](skills/verify-work/SKILL.md).",
    );
    let report = validate_skill(&skill, dir.path(), 10);
    let link_check = report
        .checks
        .iter()
        .find(|c| c.id == "link-verify-work")
        .expect("link check present");
    assert!(link_check.pass);
}

#[test]
fn checked_in_skill_catalog_matches_the_two_tier_contract() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root");
    let failures: Vec<_> = discover_skills(repo_root)
        .into_iter()
        .filter_map(|entry| {
            let raw = read_skill_raw(repo_root, &entry.name).expect("read checked-in skill");
            let line_count = raw.markdown.lines().count();
            let report = validate_skill(&parse_skill(&raw), repo_root, line_count);
            (!report.pass).then_some(format!("{}: {:?}", entry.name, report.checks))
        })
        .collect();

    assert!(
        failures.is_empty(),
        "invalid checked-in skills: {failures:#?}"
    );
}
