//! Example tests for the guard CLI mapping (task 6, issue #298).
//!
//! Included from `guard_cli.rs` via `#[path]`, so `super` is the `guard_cli` module. These pin
//! the exit code and the stderr message for each decision kind, and confirm the message names
//! no secret value (jev-active-guardrail R7.2, R7.3, R7.6).

use super::super::guard::{GuardDecision, NeutralizationPacket};
use super::*;

#[test]
fn an_allow_exits_zero_and_prints_nothing() {
    let decision = GuardDecision::Allow {
        notes: vec!["probabilistic layer did not run".to_string()],
    };
    assert_eq!(exit_code_for(&decision), EXIT_ALLOW);
    assert!(block_message(&decision).is_none());
}

#[test]
fn an_annotate_exits_zero_and_prints_nothing() {
    let decision = GuardDecision::Annotate {
        notes: vec!["rigor: complexity 40.0".to_string()],
    };
    assert_eq!(exit_code_for(&decision), EXIT_ALLOW);
    assert!(block_message(&decision).is_none());
}

#[test]
fn a_block_exits_nonzero_and_names_the_check_and_remediation() {
    let decision = GuardDecision::Block(NeutralizationPacket {
        violated_check: "protected-path".to_string(),
        offending_value: Some("specs/plan.md".to_string()),
        remediation: "Do not write a protected path.".to_string(),
        suggested_fix: None,
    });
    assert_eq!(exit_code_for(&decision), EXIT_BLOCK);
    let message = block_message(&decision).expect("a block carries a message");
    assert!(message.contains("protected-path"));
    assert!(message.contains("Do not write a protected path."));
    assert!(message.contains("specs/plan.md"));
}

#[test]
fn a_secret_block_message_names_the_marker_not_the_value() {
    // The packet already excludes the secret value; the offending value is a marker name.
    let decision = GuardDecision::Block(NeutralizationPacket {
        violated_check: "secret".to_string(),
        offending_value: Some("secret-marker".to_string()),
        remediation: "Remove the secret before the write.".to_string(),
        suggested_fix: None,
    });
    let message = block_message(&decision).expect("a block carries a message");
    assert!(message.contains("secret-marker"));
    assert!(
        !message.contains("value"),
        "the marker carries no secret value: {message}"
    );
}

#[test]
fn a_rigor_block_message_carries_the_suggested_fix() {
    let decision = GuardDecision::Block(NeutralizationPacket {
        violated_check: "rigor".to_string(),
        offending_value: None,
        remediation: "Re-align the change with the project conventions.".to_string(),
        suggested_fix: Some("REFACTOR_IMPORTS".to_string()),
    });
    let message = block_message(&decision).expect("a block carries a message");
    assert!(message.contains("suggested fix: REFACTOR_IMPORTS"));
}
