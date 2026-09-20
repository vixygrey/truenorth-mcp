//! Example tests for the combine step and the confidence gate (task 2, issue #294).
//!
//! Included from `guard_combine.rs` via `#[path]`, so `super` is the `combine` module. These
//! pin the decision boundaries: the confidence gate at, above, and below the threshold, the
//! destructive threshold for a destructive candidate, the drift block, the rigor block versus
//! annotate, and the clean allow (jev-active-guardrail R3.4, R5, R6.2).

use super::super::super::config::JevConfig;
use super::super::super::drift::DriftOutcome;
use super::super::super::rigor::RigorReport;
use super::super::super::self_heal::SelfHeal;
use super::super::{GuardDecision, NeutralizationPacket};
use super::*;

/// A default config with known thresholds: confidence_high 0.85, destructive 0.90, drift
/// boundary 0.70, rigor-failure boundary 0.70, complexity boundary 70.0.
fn config() -> JevConfig {
    JevConfig::default()
}

/// A rigor report with all signals clean (no failure).
fn clean_rigor() -> RigorReport {
    RigorReport {
        hallucinated_import: 0.0,
        violates_conventions: 0.0,
        complexity: 0.0,
        contains_secrets: 0.0,
        latency_ms: 5,
    }
}

/// A rigor report that is a failure: convention violation at 0.9, above the 0.70 boundary.
fn failing_rigor() -> RigorReport {
    RigorReport {
        hallucinated_import: 0.0,
        violates_conventions: 0.9,
        complexity: 10.0,
        contains_secrets: 0.0,
        latency_ms: 5,
    }
}

/// A drift outcome that is in scope.
fn in_scope_drift() -> DriftOutcome {
    DriftOutcome {
        noul: 0.1,
        out_of_scope: false,
        forced_by_path_match: false,
    }
}

/// A drift outcome that is out of scope.
fn out_of_scope_drift() -> DriftOutcome {
    DriftOutcome {
        noul: 0.9,
        out_of_scope: true,
        forced_by_path_match: false,
    }
}

/// A non-destructive candidate at the given confidence.
fn candidate(confidence: f64) -> CandidateSignal {
    CandidateSignal {
        confidence,
        destructive: false,
    }
}

/// A destructive candidate at the given confidence.
fn destructive_candidate(confidence: f64) -> CandidateSignal {
    CandidateSignal {
        confidence,
        destructive: true,
    }
}

#[test]
fn the_gate_blocks_at_exactly_the_high_threshold() {
    let cfg = config();
    // confidence_high is 0.85. A candidate exactly at it clears the gate (R5.1).
    assert!(confidence_gate(candidate(cfg.confidence_high), &cfg));
}

#[test]
fn the_gate_blocks_above_the_high_threshold() {
    let cfg = config();
    assert!(confidence_gate(candidate(0.95), &cfg));
}

#[test]
fn the_gate_does_not_block_below_the_high_threshold() {
    let cfg = config();
    // Just below 0.85 does not clear the gate (R5.2).
    assert!(!confidence_gate(candidate(0.84), &cfg));
}

#[test]
fn a_destructive_candidate_uses_the_higher_threshold() {
    let cfg = config();
    // destructive_threshold is 0.90. A destructive candidate at 0.86 clears the plain high
    // threshold but not the destructive one (R5.3).
    assert!(!confidence_gate(destructive_candidate(0.86), &cfg));
    assert!(confidence_gate(
        destructive_candidate(cfg.destructive_threshold),
        &cfg
    ));
}

#[test]
fn an_out_of_scope_confident_drift_blocks() {
    let cfg = config();
    let decision = combine(
        &clean_rigor(),
        candidate(0.0),
        &out_of_scope_drift(),
        candidate(0.90),
        None,
        &cfg,
    );
    match decision {
        GuardDecision::Block(NeutralizationPacket { violated_check, .. }) => {
            assert_eq!(violated_check, "drift");
        }
        other => panic!("expected a drift Block, got {other:?}"),
    }
}

#[test]
fn an_out_of_scope_but_unconfident_drift_does_not_block() {
    let cfg = config();
    // The drift is out of scope, but the confidence 0.5 is below the gate, so no drift block.
    // The rigor is clean, so the result is Allow (R5.2).
    let decision = combine(
        &clean_rigor(),
        candidate(0.0),
        &out_of_scope_drift(),
        candidate(0.5),
        None,
        &cfg,
    );
    assert_eq!(decision, GuardDecision::Allow { notes: Vec::new() });
}

#[test]
fn a_confident_rigor_failure_blocks_with_the_suggested_fix() {
    let cfg = config();
    let decision = combine(
        &failing_rigor(),
        candidate(0.90),
        &in_scope_drift(),
        candidate(0.0),
        Some(SelfHeal::RefactorImports),
        &cfg,
    );
    match decision {
        GuardDecision::Block(packet) => {
            assert_eq!(packet.violated_check, "rigor");
            assert_eq!(packet.suggested_fix.as_deref(), Some("REFACTOR_IMPORTS"));
        }
        other => panic!("expected a rigor Block, got {other:?}"),
    }
}

#[test]
fn an_unconfident_rigor_failure_annotates_with_the_suggested_fix() {
    let cfg = config();
    // The rigor is a failure, but the confidence 0.5 is below the gate, so it annotates (R5.2).
    let decision = combine(
        &failing_rigor(),
        candidate(0.5),
        &in_scope_drift(),
        candidate(0.0),
        Some(SelfHeal::SimplifyLogic),
        &cfg,
    );
    match decision {
        GuardDecision::Annotate { notes } => {
            assert!(
                notes.iter().any(|n| n.starts_with("rigor:")),
                "carries the summary"
            );
            assert!(
                notes.iter().any(|n| n == "suggested fix: SIMPLIFY_LOGIC"),
                "carries the suggested fix"
            );
        }
        other => panic!("expected an Annotate, got {other:?}"),
    }
}

#[test]
fn a_clean_change_allows_with_no_notes() {
    let cfg = config();
    let decision = combine(
        &clean_rigor(),
        candidate(0.0),
        &in_scope_drift(),
        candidate(0.0),
        None,
        &cfg,
    );
    assert_eq!(decision, GuardDecision::Allow { notes: Vec::new() });
}

#[test]
fn the_drift_block_wins_over_a_rigor_failure() {
    let cfg = config();
    // Both a confident drift and a confident rigor failure fire. The drift block is first.
    let decision = combine(
        &failing_rigor(),
        candidate(0.95),
        &out_of_scope_drift(),
        candidate(0.95),
        Some(SelfHeal::Revert),
        &cfg,
    );
    match decision {
        GuardDecision::Block(packet) => assert_eq!(packet.violated_check, "drift"),
        other => panic!("expected the drift Block first, got {other:?}"),
    }
}

#[test]
fn an_annotate_note_names_no_secret_value() {
    let cfg = config();
    let decision = combine(
        &failing_rigor(),
        candidate(0.5),
        &in_scope_drift(),
        candidate(0.0),
        None,
        &cfg,
    );
    if let GuardDecision::Annotate { notes } = decision {
        let joined = notes.join(" ");
        // The summary reports the signal values, not the change content.
        assert!(joined.contains("violates_conventions 0.90"));
    } else {
        panic!("expected an Annotate");
    }
}
