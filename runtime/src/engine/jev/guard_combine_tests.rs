//! Decision policy examples: detection boundaries are independent and never hard-block.

use super::super::super::config::JevConfig;
use super::super::super::drift::DriftOutcome;
use super::super::super::rigor::RigorReport;
use super::super::super::self_heal::SelfHeal;
use super::super::GuardDecision;
use super::*;

fn rigor() -> RigorReport {
    RigorReport {
        hallucinated_import: 0.0,
        violates_conventions: 0.0,
        complexity: 0.0,
        contains_secrets: 0.0,
        latency_ms: 0,
    }
}

fn drift(noul: f64) -> DriftOutcome {
    DriftOutcome {
        noul,
        out_of_scope: noul >= JevConfig::default().drift_boundary,
        forced_by_path_match: false,
    }
}

#[test]
fn clean_signals_allow() {
    assert_eq!(
        combine(&rigor(), &drift(0.1), None, &JevConfig::default()),
        GuardDecision::Allow { notes: vec![] }
    );
}

#[test]
fn model_drift_is_visible_but_not_blocking() {
    let decision = combine(&rigor(), &drift(0.9), None, &JevConfig::default());
    assert!(
        matches!(decision, GuardDecision::Annotate { notes } if notes.iter().any(|note| note.starts_with("drift:")))
    );
}

#[test]
fn unrelated_strong_signal_cannot_promote_weak_signal_across_its_boundary() {
    let config = JevConfig::default();
    let mut report = rigor();
    report.violates_conventions = config.rigor_failure_boundary - 0.01;
    assert_eq!(
        combine(&report, &drift(0.1), None, &config),
        GuardDecision::Allow { notes: vec![] }
    );
    report.contains_secrets = config.secret_boundary;
    assert!(
        matches!(combine(&report, &drift(0.1), None, &config), GuardDecision::Annotate { notes } if notes.len() == 1 && notes[0].starts_with("rigor:"))
    );
}

#[test]
fn both_findings_and_fix_remain_visible_without_model_block() {
    let mut report = rigor();
    report.violates_conventions = 0.99;
    let decision = combine(
        &report,
        &drift(0.9),
        Some(SelfHeal::SimplifyLogic),
        &JevConfig::default(),
    );
    assert!(
        matches!(decision, GuardDecision::Annotate { notes } if notes.iter().any(|n| n.starts_with("drift:")) && notes.iter().any(|n| n.starts_with("rigor:")) && notes.iter().any(|n| n == "suggested fix: SIMPLIFY_LOGIC"))
    );
}

#[test]
fn ordinal_complexity_boundary_produces_only_advice() {
    let config = JevConfig::default();
    let mut report = rigor();
    report.complexity = config.complexity_boundary;
    assert!(matches!(
        combine(&report, &drift(0.1), None, &config),
        GuardDecision::Annotate { .. }
    ));
}
