//! Property tests for the confidence-band classifier.
//!
//! Included from `confidence.rs` via `#[path]`, so `super` is the confidence module.
//!
//! Feature: jev-integration-eval, Property 23: confidence-band totality. For every
//! confidence in 0 to 1 and every threshold pair, a valid pair yields exactly one band and
//! an invalid pair yields `InvalidThresholds`. The band obeys the boundaries: at or above
//! high is High, in the low-to-high span is Medium, below low is Low.
//!
//! Feature: jev-integration-eval, Property 24: confidence-band determinism. For every
//! input, two calls with the same arguments return the same result.

use proptest::prelude::*;

use super::*;

/// Report whether a threshold pair is valid: both in 0 to 1 and low no more than high.
fn is_valid_pair(low: f64, high: f64) -> bool {
    (0.0..=1.0).contains(&low) && (0.0..=1.0).contains(&high) && low <= high
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Totality: a valid pair yields one band matching the boundaries; an invalid pair
    /// errors. The threshold generators reach outside 0 to 1, so both paths are exercised.
    #[test]
    fn confidence_band_is_total(
        confidence in 0.0f64..=1.0,
        low in -0.5f64..=1.5,
        high in -0.5f64..=1.5,
    ) {
        let result = confidence_band(confidence, low, high);

        if is_valid_pair(low, high) {
            let band = result.expect("a valid pair yields a band");
            let expected = if confidence >= high {
                ConfidenceBand::High
            } else if confidence >= low {
                ConfidenceBand::Medium
            } else {
                ConfidenceBand::Low
            };
            prop_assert_eq!(band, expected);
        } else {
            match result {
                Err(super::super::JevError::InvalidThresholds { low: l, high: h }) => {
                    prop_assert_eq!(l, low);
                    prop_assert_eq!(h, high);
                }
                other => prop_assert!(false, "expected InvalidThresholds, got {:?}", other),
            }
        }
    }

    /// A NaN threshold always yields an error, so the totality split covers NaN too.
    #[test]
    fn nan_threshold_always_errors(confidence in 0.0f64..=1.0, other in 0.0f64..=1.0) {
        prop_assert!(confidence_band(confidence, f64::NAN, other).is_err());
        prop_assert!(confidence_band(confidence, other, f64::NAN).is_err());
    }

    /// Determinism: two calls with the same arguments return the same result.
    #[test]
    fn confidence_band_is_deterministic(
        confidence in -0.5f64..=1.5,
        low in -0.5f64..=1.5,
        high in -0.5f64..=1.5,
    ) {
        let first = confidence_band(confidence, low, high);
        let second = confidence_band(confidence, low, high);
        match (first, second) {
            (Ok(a), Ok(b)) => prop_assert_eq!(a, b),
            (Err(_), Err(_)) => {}
            (a, b) => prop_assert!(false, "results differ: {:?} vs {:?}", a, b),
        }
    }
}
