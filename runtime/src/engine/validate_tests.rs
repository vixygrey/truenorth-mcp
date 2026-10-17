//! Tests for backward-compat validation and legacy phase mapping (task 3.4).
//!
//! Included from `validate.rs` via `#[path]`, so `super` is the validate module.
//!
//! Requirements: 9.1, 9.2, 9.5, 9.6.

use super::*;

#[test]
fn valid_state_parses() {
    let json = r#"{"active_epic":null,"git":{"branch":"main"},"bigpowers_version":"2.88.2"}"#;
    let state = validate_state(json).expect("valid state parses");
    assert_eq!(state.git_branch(), Some("main"));
}

#[test]
fn malformed_state_is_rejected_naming_the_file() {
    // `git` must be a mapping, not a scalar. The parse fails and the error names the file.
    let json = r#"{"git":"not-an-object"}"#;
    let error = validate_state(json).expect_err("malformed state is rejected");
    match error {
        ValidationError::Schema { file, .. } => assert_eq!(file, "state.json"),
        other => panic!("expected a schema error, got {other:?}"),
    }
}

#[test]
fn malformed_release_plan_is_rejected_naming_the_file() {
    // `build_order` must be a sequence, not a scalar.
    let json = r#"{"build_order":42}"#;
    let error = validate_release_plan(json).expect_err("malformed release-plan is rejected");
    match error {
        ValidationError::Schema { file, .. } => assert_eq!(file, "release-plan.json"),
        other => panic!("expected a schema error, got {other:?}"),
    }
}

#[test]
fn write_validator_accepts_a_valid_state() {
    let state = validate_state(r#"{"git":{"branch":"main"}}"#).expect("parse");
    let json = validate_state_for_write(&state).expect("write validation passes");
    assert!(json.contains(r#""branch": "main""#));
}

#[test]
fn write_validator_accepts_a_valid_release_plan() {
    let plan = validate_release_plan(r#"{"build_order":["e01"]}"#).expect("parse");
    let json = validate_release_plan_for_write(&plan).expect("write validation passes");
    assert!(json.contains("e01"));
}

#[test]
fn legacy_phase_names_map_to_six_phase_model() {
    assert_eq!(map_legacy_phase("Build").unwrap(), Phase::Execute);
    assert_eq!(map_legacy_phase("Verify").unwrap(), Phase::Review);
    assert_eq!(map_legacy_phase("Release").unwrap(), Phase::Integrate);
    assert_eq!(map_legacy_phase("Sustain").unwrap(), Phase::Integrate);
}

#[test]
fn legacy_phase_mapping_is_case_insensitive() {
    assert_eq!(map_legacy_phase("build").unwrap(), Phase::Execute);
    assert_eq!(map_legacy_phase("VERIFY").unwrap(), Phase::Review);
    assert_eq!(map_legacy_phase("  release  ").unwrap(), Phase::Integrate);
}

#[test]
fn six_phase_names_map_to_themselves() {
    // The mapping is idempotent for the canonical names.
    assert_eq!(map_legacy_phase("discover").unwrap(), Phase::Discover);
    assert_eq!(map_legacy_phase("design").unwrap(), Phase::Design);
    assert_eq!(map_legacy_phase("plan").unwrap(), Phase::Plan);
    assert_eq!(map_legacy_phase("execute").unwrap(), Phase::Execute);
    assert_eq!(map_legacy_phase("review").unwrap(), Phase::Review);
    assert_eq!(map_legacy_phase("integrate").unwrap(), Phase::Integrate);
}

#[test]
fn unrecognized_phase_name_is_rejected() {
    let error = map_legacy_phase("deploy").expect_err("unrecognized phase is rejected");
    match error {
        ValidationError::UnknownPhase { name } => assert_eq!(name, "deploy"),
        other => panic!("expected an unknown-phase error, got {other:?}"),
    }
}
