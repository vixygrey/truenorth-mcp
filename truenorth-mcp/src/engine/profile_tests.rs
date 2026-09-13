//! Unit tests for the methodology profiles.
//!
//! Included from `profile.rs` via `#[path]`, so `super` is the profile module.
//!
//! These cover the five built-ins, the by-name lookup, the absent-config default, and
//! the unknown-name error. The exhaustive generated coverage lives in the Property 10
//! proptest.

use std::fs;

use tempfile::TempDir;

use super::*;

/// Seed `.agent/profile.yml` with the given profile name under a fresh temp repo root.
fn seed_profile(name: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(AGENT_DIR);
    fs::create_dir_all(&agent).expect("create .agent");
    fs::write(agent.join("profile.yml"), format!("profile: {name}\n")).expect("write profile.yml");
    repo
}

#[test]
fn all_profiles_holds_the_five_built_ins() {
    let names: Vec<&str> = ALL_PROFILES.iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        vec![
            "epic-based",
            "issue-per-task",
            "kanban",
            "milestone-based",
            "generic"
        ]
    );
}

#[test]
fn by_name_resolves_each_built_in() {
    for profile in ALL_PROFILES {
        assert_eq!(by_name(profile.name), Some(profile));
    }
}

#[test]
fn by_name_is_none_for_an_unknown_name() {
    assert_eq!(by_name("scrumban"), None);
    assert_eq!(by_name(""), None);
    assert_eq!(by_name("Epic-Based"), None);
}

#[test]
fn required_profiles_carry_a_grouping_vocab() {
    // A required-grouping profile names a concrete vocabulary, not None.
    assert_eq!(EPIC_BASED.rule, GroupingRule::Required);
    assert_eq!(EPIC_BASED.vocab, GroupingVocab::Epic);
    assert_eq!(MILESTONE_BASED.rule, GroupingRule::Required);
    assert_eq!(MILESTONE_BASED.vocab, GroupingVocab::Milestone);

    // The no-grouping profiles use None and mark grouping optional.
    assert_eq!(KANBAN.vocab, GroupingVocab::None);
    assert_eq!(KANBAN.rule, GroupingRule::Optional);
    assert_eq!(GENERIC.vocab, GroupingVocab::None);
    assert_eq!(GENERIC.rule, GroupingRule::Optional);
}

#[test]
fn resolve_active_defaults_to_issue_per_task_when_config_is_absent() {
    // A repo with no .agent/profile.yml at all resolves to the default (Requirement 3.5).
    let repo = TempDir::new().expect("temp repo");
    let resolved = resolve_active(repo.path()).expect("absent config resolves");
    assert_eq!(resolved, ISSUE_PER_TASK);
}

#[test]
fn resolve_active_reads_each_declared_profile() {
    for profile in ALL_PROFILES {
        let repo = seed_profile(profile.name);
        let resolved = resolve_active(repo.path()).expect("declared profile resolves");
        assert_eq!(resolved, profile);
    }
}

#[test]
fn resolve_active_rejects_an_unknown_profile_naming_the_five() {
    let repo = seed_profile("waterfall");
    let error = resolve_active(repo.path()).expect_err("unknown profile rejected");
    match error {
        ProfileError::UnknownProfile { name } => {
            assert_eq!(name, "waterfall");
            let message = ProfileError::UnknownProfile { name }.to_string();
            for valid in [
                "epic-based",
                "issue-per-task",
                "kanban",
                "milestone-based",
                "generic",
            ] {
                assert!(message.contains(valid), "message must name `{valid}`");
            }
        }
        other => panic!("expected UnknownProfile, got {other:?}"),
    }
}

#[test]
fn resolve_active_errors_on_a_malformed_config() {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(AGENT_DIR);
    fs::create_dir_all(&agent).expect("create .agent");
    // A mapping without the `profile` key fails to deserialize into ProfileConfig.
    fs::write(agent.join("profile.yml"), "other: value\n").expect("write malformed");

    let error = resolve_active(repo.path()).expect_err("malformed config rejected");
    assert!(matches!(error, ProfileError::Parse { .. }));
}
