//! The pure combine step and the confidence gate for the active guardrail (task 2, #294).
//!
//! Included from `guard.rs` via `#[path]`, so `super` is the `guard` module. This module
//! holds the probabilistic decision logic: the confidence gate and the combine step. It runs
//! after the deterministic layer in `guard.rs`, so it is split by concern, not arbitrarily.
//!
//! The combine step is pure over its inputs: no async, no clock, no disk read (P39). It gates
//! a candidate block on the model confidence against a threshold from [`JevConfig`], using the
//! higher `destructive_threshold` for a destructive candidate (R5). It returns one
//! [`GuardDecision`]: a drift or rigor block when a candidate clears the gate, an annotate when
//! a rigor failure is below the gate, or an allow when no candidate fires.
//!
//! The step attaches a suggested self-heal instruction to a rigor block; it never applies it
//! and triggers no secondary agent (R9.2, R9.3, ADR-G5). The caller decides the self-heal
//! through the harness `self_heal` aspect (`evaluate_guard`, issue #295).
//!
//! The combine step and the gate have no non-test caller until `evaluate_guard` lands (issue
//! #295). Each staged public item carries a narrow non-test `allow` with this reason, per the
//! repo dead-code policy (main.rs). The test build exercises every item through the sibling
//! `guard_combine_tests.rs` and `guard_combine_prop_tests.rs`.
//!
//! Requirements: 3.4, 5.1, 5.2, 5.3, 5.4, 6.2. Design: jev-active-guardrail, the combine step,
//! P38 (confidence-gated block), P39 (decision determinism).

use super::super::config::JevConfig;
use super::super::drift::DriftOutcome;
use super::super::rigor::RigorReport;
use super::super::self_heal::{SelfHeal, is_rigor_failure, option_str};
use super::{GuardDecision, NeutralizationPacket};

/// The violated-check name a drift block reports (R6.1).
const CHECK_DRIFT: &str = "drift";

/// The violated-check name a rigor block reports (R6.1).
const CHECK_RIGOR: &str = "rigor";

/// The remediation hint a drift block returns (R3.4, R6.1).
const REMEDIATION_DRIFT: &str =
    "Re-align the plan with the project scope. The change drifts outside the intended work.";

/// The remediation hint a rigor block returns (R6.1, R6.2).
const REMEDIATION_RIGOR: &str =
    "Re-align the change with the project conventions. Review the flagged rigor signal.";

/// One candidate block signal, gated on its confidence (R3.4, R5.1, R5.2).
///
/// The probabilistic layer builds a candidate when an aspect value crosses its boundary
/// (R3.4). The candidate carries the model's confidence in that signal and a flag that marks
/// a destructive action. The confidence gate reads only these two fields, so the gate is a
/// pure function of the candidate and the config.
///
/// The [`RigorReport`] and the [`DriftOutcome`] from the harness carry a value but no separate
/// confidence, because a Noul answer carries none (jev-integration-eval, `Answer::Noul`). The
/// caller (`evaluate_guard`, issue #295) supplies the confidence from the model answer that
/// produced the value, so the confidence stays a real model signal, not a value invented in
/// the combine step. The combine step gates on that confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateSignal {
    /// The model confidence in the signal, from 0 to 1.
    pub confidence: f64,
    /// True when the candidate is a destructive action, which uses the higher threshold.
    pub destructive: bool,
}

/// Report whether a candidate block signal clears its confidence threshold (R5.1 to R5.4).
///
/// The gate compares the candidate confidence against a threshold from [`JevConfig`]. A
/// non-destructive candidate uses `confidence_high`. A destructive candidate uses the higher
/// `destructive_threshold` (R5.3). The comparison is `confidence >= threshold`, so a candidate
/// exactly at the threshold blocks (R5.1). The gate reads the threshold from the config and
/// hardcodes no value (R5.4).
///
/// The gate is pure over the candidate and the config, so two evaluations of the same input
/// return the same result (P39), and the property test drives it with no async runtime.
pub fn confidence_gate(candidate: CandidateSignal, config: &JevConfig) -> bool {
    let threshold = if candidate.destructive {
        config.destructive_threshold
    } else {
        config.confidence_high
    };
    candidate.confidence >= threshold
}

/// Combine the rigor and the drift signals into one decision (R3.4, R5, R6.2, P38, P39).
///
/// The step is pure over its inputs: no async, no clock, no disk read (P39). It runs two
/// candidate checks in a fixed order and returns the first block it reaches.
///
/// First, the drift candidate: when the drift outcome is out of scope, the step gates the
/// drift confidence. A confident drift candidate returns a `drift` block (R3.4, R5.1). A drift
/// outcome forced by the model-free path match is always confident, because the deterministic
/// layer already decided it; the caller passes a confidence at or above the threshold for a
/// forced outcome.
///
/// Then, the rigor candidate: when the rigor report is a failure ([`is_rigor_failure`] against
/// the configured boundaries), the step gates the rigor confidence. A confident rigor candidate
/// returns a `rigor` block carrying the suggested self-heal instruction (R6.2). A rigor failure
/// below the threshold returns an `Annotate` with the same suggested fix, so the change proceeds
/// with the advisory attached (R5.2).
///
/// When no candidate blocks, the step returns `Allow` with empty notes (R1.4).
///
/// The `suggested_fix` is the decided self-heal instruction the caller passes in. The combine
/// step attaches it; it never applies it and triggers no secondary agent (R9.2, R9.3, ADR-G5).
/// The caller decides the self-heal through the harness `self_heal` aspect (issue #295).
pub fn combine(
    rigor: &RigorReport,
    rigor_signal: CandidateSignal,
    drift: &DriftOutcome,
    drift_signal: CandidateSignal,
    suggested_fix: Option<SelfHeal>,
    config: &JevConfig,
) -> GuardDecision {
    // The drift candidate. An out-of-scope drift that clears the gate blocks first (R3.4).
    if drift.out_of_scope && confidence_gate(drift_signal, config) {
        return GuardDecision::Block(NeutralizationPacket {
            violated_check: CHECK_DRIFT.to_string(),
            offending_value: None,
            remediation: REMEDIATION_DRIFT.to_string(),
            suggested_fix: None,
        });
    }

    // The rigor candidate. A rigor failure blocks when confident, or annotates otherwise.
    if is_rigor_failure(
        rigor,
        config.rigor_failure_boundary,
        config.complexity_boundary,
    ) {
        let fix = suggested_fix.map(|s| option_str(s).to_string());
        if confidence_gate(rigor_signal, config) {
            return GuardDecision::Block(NeutralizationPacket {
                violated_check: CHECK_RIGOR.to_string(),
                offending_value: None,
                remediation: REMEDIATION_RIGOR.to_string(),
                suggested_fix: fix,
            });
        }
        // A rigor failure below the threshold does not block. It annotates with the summary and
        // the suggested fix, so the change proceeds with the advisory attached (R5.2).
        let mut notes = vec![rigor_summary(rigor)];
        if let Some(fix) = fix {
            notes.push(format!("suggested fix: {fix}"));
        }
        return GuardDecision::Annotate { notes };
    }

    // No candidate blocks. The change proceeds with no advisory (R1.4).
    GuardDecision::Allow { notes: Vec::new() }
}

/// Summarize a rigor report for an annotate note, naming no secret value (R6.3).
///
/// The summary reports the three Noul judgments and the complexity, so the annotate note
/// carries the signal that fired without echoing the change content. It names no secret value,
/// so an annotate note leaks nothing (P40).
fn rigor_summary(rigor: &RigorReport) -> String {
    format!(
        "rigor: hallucinated_import {:.2}, violates_conventions {:.2}, contains_secrets {:.2}, \
         complexity {:.1}",
        rigor.hallucinated_import,
        rigor.violates_conventions,
        rigor.contains_secrets,
        rigor.complexity,
    )
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `combine`.
#[cfg(test)]
#[path = "guard_combine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guard_combine_prop_tests.rs"]
mod prop_tests;
