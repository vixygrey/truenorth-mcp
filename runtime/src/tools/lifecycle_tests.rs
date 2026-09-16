//! Tests for the lifecycle tool validation (task 9.2).
//!
//! Included from `lifecycle.rs` via `#[path]`, so `super` is the lifecycle module. The
//! tool methods need a live `Peer`, so these tests drive the pure validation helpers
//! where Requirement 2.11 lives. The write behavior is covered by the cockpit engine
//! tests.
//!
//! Requirements: 2.2, 2.4, 2.11.

use super::*;

#[test]
fn parse_phase_accepts_six_phase_names() {
    assert_eq!(
        parse_phase("discover", "from_phase").unwrap(),
        Phase::Discover
    );
    assert_eq!(
        parse_phase("integrate", "to_phase").unwrap(),
        Phase::Integrate
    );
}

#[test]
fn parse_phase_accepts_legacy_names() {
    // Legacy names map, so an existing flow keeps working.
    assert_eq!(parse_phase("build", "from_phase").unwrap(), Phase::Execute);
    assert_eq!(parse_phase("verify", "to_phase").unwrap(), Phase::Review);
}

#[test]
fn parse_phase_rejects_unknown_naming_the_field() {
    // Requirement 2.11: the error names the offending field.
    let error = parse_phase("shipping", "to_phase").expect_err("unknown phase");
    assert!(error.message.contains("to_phase"));
    assert!(error.message.contains("shipping"));
}

#[test]
fn check_len_enforces_bounds() {
    assert!(check_len("ok", 1, 10, "field").is_ok());
    // Empty violates the min.
    assert!(check_len("", 1, 10, "artifacts_summary").is_err());
    // Over the max.
    let long = "x".repeat(11);
    let error = check_len(&long, 1, 10, "artifacts_summary").expect_err("too long");
    assert!(error.message.contains("artifacts_summary"));
    assert!(error.message.contains("1 to 10"));
}

#[test]
fn check_len_counts_characters_not_bytes() {
    // A 2-char multi-byte string is within a 2-char cap.
    assert!(check_len("éé", 1, 2, "field").is_ok());
}

#[test]
fn check_epic_id_accepts_valid_ids() {
    for id in ["e1", "e80", "e80s01", "e12-fork-innovations"] {
        assert!(check_epic_id(id).is_ok(), "`{id}` should be valid");
    }
}

#[test]
fn check_epic_id_rejects_invalid_ids() {
    // Requirement 2.4: the pattern rejects these, and the error names the field.
    for id in ["epic1", "80", "E80", "e", "e80_bad"] {
        let error = check_epic_id(id).expect_err("invalid id");
        assert!(
            error.message.contains("epic_id"),
            "`{id}` error names the field"
        );
    }
}

use tempfile::TempDir;

/// Build a RecordTaskArgs with the given grouping fields and fixed task/verify.
fn args(group_id: Option<&str>, group_kind: Option<&str>, epic_id: Option<&str>) -> RecordTaskArgs {
    RecordTaskArgs {
        group_id: group_id.map(str::to_string),
        group_kind: group_kind.map(str::to_string),
        epic_id: epic_id.map(str::to_string),
        task_name: "Wire the gate".to_string(),
        verify_command: "cargo test".to_string(),
    }
}

/// A repo with no `.agent/profile.yml`, so the default issue-per-task profile applies.
fn default_profile_repo() -> TempDir {
    TempDir::new().expect("temp repo")
}

/// A repo whose `.agent/profile.yml` names the given profile.
fn repo_with_profile(name: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(".agent");
    std::fs::create_dir_all(&agent).expect("create .agent");
    std::fs::write(agent.join("profile.yml"), format!("profile: {name}\n")).expect("write profile");
    repo
}

#[test]
fn resolve_grouping_maps_legacy_epic_id_under_any_profile() {
    // Requirement 4.7: a legacy epic_id maps to group_kind=epic and is accepted even under
    // the default issue-per-task profile whose vocabulary is ticket.
    let repo = default_profile_repo();
    let grouping = resolve_grouping(repo.path(), &args(None, None, Some("e80"))).expect("mapped");
    assert_eq!(grouping.id.as_deref(), Some("e80"));
    assert_eq!(grouping.kind.as_deref(), Some("epic"));
}

#[test]
fn resolve_grouping_accepts_optional_omission() {
    // Requirement 4.2: the default profile is optional, so an omitted grouping key passes.
    let repo = default_profile_repo();
    let grouping = resolve_grouping(repo.path(), &args(None, None, None)).expect("optional");
    assert!(grouping.id.is_none());
    assert!(grouping.kind.is_none());
}

#[test]
fn resolve_grouping_requires_group_id_for_a_required_profile() {
    // Requirement 4.3: the epic-based profile requires a grouping key.
    let repo = repo_with_profile("epic-based");
    let error = resolve_grouping(repo.path(), &args(None, None, None)).expect_err("required");
    assert!(error.message.contains("requires a grouping key"));
}

#[test]
fn resolve_grouping_rejects_an_over_long_group_id() {
    // Requirement 4.4: a group_id over 200 chars is rejected, naming the field.
    let repo = default_profile_repo();
    let long = "x".repeat(201);
    let error =
        resolve_grouping(repo.path(), &args(Some(&long), None, None)).expect_err("too long");
    assert!(error.message.contains("group_id"));
}

#[test]
fn resolve_grouping_rejects_a_group_kind_outside_the_set() {
    // Requirement 4.5: a group_kind outside the allowed set is rejected.
    let repo = repo_with_profile("epic-based");
    let error = resolve_grouping(repo.path(), &args(Some("e80"), Some("saga"), None))
        .expect_err("bad kind");
    assert!(error.message.contains("group_kind"));
}

#[test]
fn resolve_grouping_rejects_a_group_kind_off_the_profile_vocabulary() {
    // Requirement 4.6: a supplied group_kind must match the active profile vocabulary.
    // The default issue-per-task profile uses ticket, so epic is a mismatch.
    let repo = default_profile_repo();
    let error = resolve_grouping(repo.path(), &args(Some("t1"), Some("epic"), None))
        .expect_err("vocab mismatch");
    assert!(error.message.contains("does not match"));
}

#[test]
fn resolve_grouping_accepts_a_matching_group_kind() {
    // A supplied group_kind that matches the profile vocabulary passes.
    let repo = repo_with_profile("epic-based");
    let grouping =
        resolve_grouping(repo.path(), &args(Some("e80"), Some("epic"), None)).expect("match");
    assert_eq!(grouping.id.as_deref(), Some("e80"));
    assert_eq!(grouping.kind.as_deref(), Some("epic"));
}

#[test]
fn cockpit_validation_error_maps_to_invalid_params() {
    let error = cockpit_error(CockpitError::Validation(ValidationError::Schema {
        file: "state.yaml",
        detail: "bad".to_string(),
    }));
    // invalid_params carries code -32602.
    assert_eq!(error.code.0, -32602);
}
