//! Tests for the TDD step transition rule (task 11.2).
//!
//! Included from `tdd.rs` via `#[path]`, so `super` is the tdd module.
//!
//! Requirements: 2.7, 2.8.

use super::*;

#[test]
fn parse_and_display_round_trip() {
    for (text, step) in [
        ("red", TddStep::Red),
        ("green", TddStep::Green),
        ("refactor", TddStep::Refactor),
    ] {
        assert_eq!(TddStep::parse(text), Some(step));
        assert_eq!(step.as_str(), text);
    }
    assert_eq!(TddStep::parse("blue"), None);
}

#[test]
fn a_fresh_cycle_starts_at_red() {
    // Requirement 2.7: with no recorded step, red is the only valid start.
    assert_eq!(next_step(None, TddStep::Red).unwrap(), TddStep::Red);
    assert!(next_step(None, TddStep::Green).is_err());
    assert!(next_step(None, TddStep::Refactor).is_err());
}

#[test]
fn red_advances_to_green_only() {
    assert_eq!(
        next_step(Some(TddStep::Red), TddStep::Green).unwrap(),
        TddStep::Green
    );
    assert!(next_step(Some(TddStep::Red), TddStep::Red).is_err());
    assert!(next_step(Some(TddStep::Red), TddStep::Refactor).is_err());
}

#[test]
fn green_advances_to_refactor_only() {
    assert_eq!(
        next_step(Some(TddStep::Green), TddStep::Refactor).unwrap(),
        TddStep::Refactor
    );
    assert!(next_step(Some(TddStep::Green), TddStep::Red).is_err());
    assert!(next_step(Some(TddStep::Green), TddStep::Green).is_err());
}

#[test]
fn refactor_returns_to_red_for_the_next_cycle() {
    assert_eq!(
        next_step(Some(TddStep::Refactor), TddStep::Red).unwrap(),
        TddStep::Red
    );
    assert!(next_step(Some(TddStep::Refactor), TddStep::Green).is_err());
    assert!(next_step(Some(TddStep::Refactor), TddStep::Refactor).is_err());
}

#[test]
fn invalid_transition_names_the_steps() {
    // Requirement 2.8: the error identifies the current and requested steps.
    let error = next_step(Some(TddStep::Red), TddStep::Refactor).expect_err("invalid");
    assert_eq!(error.current, "red");
    assert_eq!(error.requested, "refactor");
}
