//! Example tests for the confidence-band classifier.
//!
//! Included from `confidence.rs` via `#[path]`, so `super` is the confidence module. These
//! pin the band boundaries at exact threshold values and the invalid-threshold rejections
//! (Requirement 8.6, 8.7).

use super::*;

#[test]
fn confidence_at_or_above_high_is_high() {
    assert_eq!(
        confidence_band(0.90, 0.60, 0.85).expect("valid pair"),
        ConfidenceBand::High
    );
    // Exactly at the high threshold is High.
    assert_eq!(
        confidence_band(0.85, 0.60, 0.85).expect("valid pair"),
        ConfidenceBand::High
    );
}

#[test]
fn confidence_in_the_middle_band_is_medium() {
    assert_eq!(
        confidence_band(0.70, 0.60, 0.85).expect("valid pair"),
        ConfidenceBand::Medium
    );
    // Exactly at the low threshold is Medium.
    assert_eq!(
        confidence_band(0.60, 0.60, 0.85).expect("valid pair"),
        ConfidenceBand::Medium
    );
}

#[test]
fn confidence_below_low_is_low() {
    assert_eq!(
        confidence_band(0.10, 0.60, 0.85).expect("valid pair"),
        ConfidenceBand::Low
    );
}

#[test]
fn equal_thresholds_are_valid() {
    // low == high is a valid pair. The middle band is empty, so a value at the shared
    // threshold is High and a value below it is Low.
    assert_eq!(
        confidence_band(0.70, 0.70, 0.70).expect("equal pair"),
        ConfidenceBand::High
    );
    assert_eq!(
        confidence_band(0.69, 0.70, 0.70).expect("equal pair"),
        ConfidenceBand::Low
    );
}

#[test]
fn low_greater_than_high_is_rejected() {
    match confidence_band(0.5, 0.9, 0.5) {
        Err(super::super::JevError::InvalidThresholds { low, high }) => {
            assert_eq!(low, 0.9);
            assert_eq!(high, 0.5);
        }
        other => panic!("expected InvalidThresholds, got {other:?}"),
    }
}

#[test]
fn out_of_range_threshold_is_rejected() {
    assert!(confidence_band(0.5, -0.1, 0.8).is_err());
    assert!(confidence_band(0.5, 0.2, 1.1).is_err());
}

#[test]
fn nan_threshold_is_rejected() {
    assert!(confidence_band(0.5, f64::NAN, 0.8).is_err());
    assert!(confidence_band(0.5, 0.2, f64::NAN).is_err());
}
