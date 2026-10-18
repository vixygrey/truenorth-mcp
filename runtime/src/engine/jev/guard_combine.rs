//! Pure model-advisory decision policy for the active guard.
//!
//! Detection uses each aspect's own configured boundary. A Noul judgment is not an
//! independent probability or confidence in a different signal; the Score rubric is ordinal.
//! Only the deterministic guard layer may block a proposed change.
use super::super::config::JevConfig;
use super::super::drift::DriftOutcome;
use super::super::rigor::RigorReport;
use super::super::self_heal::{SelfHeal, is_rigor_failure, option_str};
use super::GuardDecision;

/// Combine independent model findings into visible, nonblocking advice.
///
/// Both findings survive when both aspects fire; neither can suppress the other.
/// The caller may attach an existing self-heal suggestion but never executes one.
pub fn combine(
    rigor: &RigorReport,
    drift: &DriftOutcome,
    suggested_fix: Option<SelfHeal>,
    config: &JevConfig,
) -> GuardDecision {
    let mut notes = Vec::new();
    if drift.out_of_scope {
        notes.push(format!("drift: out of scope (judgment {:.2})", drift.noul));
    }
    notes.extend(rigor_advice(rigor, suggested_fix, config));

    if notes.is_empty() {
        GuardDecision::Allow { notes }
    } else {
        GuardDecision::Annotate { notes }
    }
}

/// Extract the rigor finding independently when drift scoring is unavailable.
pub fn rigor_advice(
    rigor: &RigorReport,
    suggested_fix: Option<SelfHeal>,
    config: &JevConfig,
) -> Vec<String> {
    if !is_rigor_failure(
        rigor,
        config.rigor_failure_boundary,
        config.secret_boundary,
        config.complexity_boundary,
    ) {
        return Vec::new();
    }
    let mut notes = vec![rigor_summary(rigor)];
    if let Some(fix) = suggested_fix {
        notes.push(format!("suggested fix: {}", option_str(fix)));
    }
    notes
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
