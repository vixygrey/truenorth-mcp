//! Tests for the skills tools (task 8a).
//!
//! Included from `skills.rs` via `#[path]`, so `super` is the skills module. The tier
//! resolution tests drive the pure `parse_tier` and `resolve_tier(Some(..))` paths, which
//! read no process state. One env-backed test runs serially.
//!
//! Requirements: 6.2, 6.3, 6.4, 6.5.

use super::*;
use std::fs;
use tempfile::tempdir;

fn make_skill(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join("skills").join(name);
    fs::create_dir_all(&dir).expect("create skill dir");
    fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
}

#[test]
fn parse_tier_accepts_the_three_tiers() {
    assert_eq!(parse_tier("full").unwrap(), Tier::Full);
    assert_eq!(parse_tier("reasoning").unwrap(), Tier::Reasoning);
    assert_eq!(parse_tier("lean").unwrap(), Tier::Lean);
}

#[test]
fn parse_tier_rejects_unknown_value() {
    // Requirement 6.2: an unrecognized tier is rejected.
    let error = parse_tier("verbose").expect_err("unknown tier is rejected");
    assert!(error.message.contains("unrecognized tier"));
}

#[test]
fn per_call_tier_overrides_and_validates() {
    // Requirement 6.4: the per-call argument is the effective tier.
    assert_eq!(resolve_tier(Some("lean")).unwrap(), Tier::Lean);
    // Requirement 6.2: a bad per-call tier is rejected even when env is unset.
    assert!(resolve_tier(Some("nope")).is_err());
}

#[test]
fn env_and_default_resolution_serialized() {
    // Requirement 6.4, 6.5: the env var sets the default when no argument is supplied.
    // SAFETY: this test owns `TRUENORTH_TIER` and restores it. No other test reads it.
    unsafe {
        std::env::set_var(TIER_ENV, "reasoning");
    }
    assert_eq!(resolve_tier(None).unwrap(), Tier::Reasoning);

    unsafe {
        std::env::set_var(TIER_ENV, "");
    }
    // An empty env value falls back to full.
    assert_eq!(resolve_tier(None).unwrap(), Tier::Full);

    unsafe {
        std::env::remove_var(TIER_ENV);
    }
    // An absent env value falls back to full.
    assert_eq!(resolve_tier(None).unwrap(), Tier::Full);
}

#[test]
fn resolve_reads_the_skill_markdown() {
    // The resolution path a get_skill call takes: read the raw markdown.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    make_skill(root, "develop-tdd", "# TDD\n\nBody.\n");
    let raw = resolve_for_test(root, "develop-tdd").expect("resolve");
    assert!(raw.markdown.contains("# TDD"));
}

#[test]
fn resolve_errors_on_unresolved_name() {
    // Requirement 6.3: an unresolved name yields an error, mapped to invalid params.
    let dir = tempdir().expect("temp dir");
    let error = resolve_for_test(dir.path(), "absent").expect_err("missing skill");
    let mcp = skill_error(error);
    assert!(mcp.message.contains("Skill not found"));
}
