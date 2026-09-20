//! Property tests for the combine step and the confidence gate (task 2, issue #294).
//!
//! Included from `guard_combine.rs` via `#[path]`, so `super` is the `combine` module.
//!
//! Feature: jev-active-guardrail.
//!
//! - Property 38: confidence-gated block. For all candidate signals and all valid thresholds,
//!   the confidence gate clears exactly when the confidence is at or above the threshold
//!   (R5.1, R5.2), and the combine step returns a block exactly when a fired candidate clears
//!   the gate.
//! - Property 39: decision determinism. For all signals and all thresholds, two evaluations of
//!   the same input return the same decision (R5.5).
//!
//! The combine step is pure, so both properties run with no async runtime.

use proptest::prelude::*;

use super::super::super::config::JevConfig;
use super::super::super::drift::DriftOutcome;
use super::super::super::rigor::RigorReport;
use super::*;

/// Build a config with the given valid thresholds. The other fields take defaults, so the
/// generated thresholds drive the gate. `confidence_low <= confidence_high <=
/// destructive_threshold` holds by construction, which the config validation requires.
fn config_with(confidence_high: f64, destructive_threshold: f64) -> JevConfig {
    JevConfig {
        confidence_high,
        confidence_low: 0.0,
        destructive_threshold,
        ..JevConfig::default()
    }
}

/// A rigor report that is a failure: a convention violation above any 0 to 1 boundary.
fn failing_rigor() -> RigorReport {
    RigorReport {
        hallucinated_import: 0.0,
        violates_conventions: 1.0,
        complexity: 0.0,
        contains_secrets: 0.0,
        latency_ms: 1,
    }
}

/// A clean rigor report: no signal fires.
fn clean_rigor() -> RigorReport {
    RigorReport {
        hallucinated_import: 0.0,
        violates_conventions: 0.0,
        complexity: 0.0,
        contains_secrets: 0.0,
        latency_ms: 1,
    }
}

/// An in-scope drift outcome.
fn in_scope_drift() -> DriftOutcome {
    DriftOutcome {
        noul: 0.0,
        out_of_scope: false,
        forced_by_path_match: false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 38: the confidence gate clears exactly when the confidence is at or above the
    /// threshold that applies to the candidate.
    #[test]
    fn p38_gate_clears_exactly_at_or_above_the_threshold(
        confidence in 0.0f64..=1.0,
        confidence_high in 0.0f64..=1.0,
        extra in 0.0f64..=0.10,
        destructive in any::<bool>(),
    ) {
        // Keep destructive_threshold >= confidence_high and in 0 to 1, as the config requires.
        let destructive_threshold = (confidence_high + extra).min(1.0);
        let cfg = config_with(confidence_high, destructive_threshold);
        let candidate = CandidateSignal { confidence, destructive };

        let cleared = confidence_gate(candidate, &cfg);
        let threshold = if destructive { destructive_threshold } else { confidence_high };
        prop_assert_eq!(cleared, confidence >= threshold, "the gate equals confidence >= threshold");
    }

    /// Property 38: a fired rigor candidate blocks exactly when its confidence clears the gate.
    #[test]
    fn p38_rigor_candidate_blocks_exactly_when_confident(
        confidence in 0.0f64..=1.0,
        confidence_high in 0.0f64..=1.0,
    ) {
        // A non-destructive rigor candidate uses confidence_high. destructive_threshold sits at
        // or above it. The rigor report is a failure, so the candidate fires.
        let cfg = config_with(confidence_high, confidence_high);
        let rigor_signal = CandidateSignal { confidence, destructive: false };

        let decision = combine(
            &failing_rigor(),
            rigor_signal,
            &in_scope_drift(),
            CandidateSignal { confidence: 0.0, destructive: false },
            None,
            &cfg,
        );

        let is_block = matches!(decision, GuardDecision::Block(_));
        let is_annotate = matches!(decision, GuardDecision::Annotate { .. });
        prop_assert_eq!(is_block, confidence >= confidence_high, "block exactly when confident");
        // Below the gate, a rigor failure annotates rather than allows or blocks (R5.2).
        if !is_block {
            prop_assert!(is_annotate, "an unconfident rigor failure annotates");
        }
    }

    /// Property 39: two evaluations of the same input return the same decision.
    #[test]
    fn p39_combine_is_deterministic(
        rigor_conf in 0.0f64..=1.0,
        drift_conf in 0.0f64..=1.0,
        confidence_high in 0.0f64..=1.0,
        out_of_scope in any::<bool>(),
        rigor_fails in any::<bool>(),
    ) {
        let cfg = config_with(confidence_high, confidence_high);
        let rigor = if rigor_fails { failing_rigor() } else { clean_rigor() };
        let drift = DriftOutcome {
            noul: if out_of_scope { 1.0 } else { 0.0 },
            out_of_scope,
            forced_by_path_match: false,
        };
        let rigor_signal = CandidateSignal { confidence: rigor_conf, destructive: false };
        let drift_signal = CandidateSignal { confidence: drift_conf, destructive: false };

        let first = combine(&rigor, rigor_signal, &drift, drift_signal, None, &cfg);
        let second = combine(&rigor, rigor_signal, &drift, drift_signal, None, &cfg);
        prop_assert_eq!(first, second, "two evaluations of the same input agree");
    }
}
