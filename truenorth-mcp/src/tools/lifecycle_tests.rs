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

#[test]
fn cockpit_validation_error_maps_to_invalid_params() {
    let error = cockpit_error(CockpitError::Validation(ValidationError::Schema {
        file: "state.yaml",
        detail: "bad".to_string(),
    }));
    // invalid_params carries code -32602.
    assert_eq!(error.code.0, -32602);
}
