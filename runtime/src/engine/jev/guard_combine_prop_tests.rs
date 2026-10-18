//! Pure advisory policy properties over independently detected aspect boundaries.

use proptest::prelude::*;

use super::super::super::config::JevConfig;
use super::super::super::drift::DriftOutcome;
use super::super::super::rigor::RigorReport;
use super::super::GuardDecision;
use super::*;

proptest! {
    #[test]
    fn model_findings_never_block_and_both_findings_survive(
        convention in 0.0f64..=1.0,
        secret in 0.0f64..=1.0,
        complexity in 0.0f64..=100.0,
        scope in 0.0f64..=1.0,
    ) {
        let config = JevConfig::default();
        let rigor = RigorReport {
            hallucinated_import: 0.0,
            violates_conventions: convention,
            contains_secrets: secret,
            complexity,
            latency_ms: 0,
        };
        let drift = DriftOutcome {
            noul: scope,
            out_of_scope: scope >= config.drift_boundary,
            forced_by_path_match: false,
        };
        let decision = combine(&rigor, &drift, None, &config);
        let rigor_fires = convention >= config.rigor_failure_boundary
            || secret >= config.secret_boundary
            || complexity >= config.complexity_boundary;
        match decision {
            GuardDecision::Annotate { notes } => {
                prop_assert_eq!(notes.iter().any(|n| n.starts_with("drift:")), drift.out_of_scope);
                prop_assert_eq!(notes.iter().any(|n| n.starts_with("rigor:")), rigor_fires);
            }
            GuardDecision::Allow { notes } => {
                prop_assert!(notes.is_empty());
                prop_assert!(!drift.out_of_scope && !rigor_fires);
            }
            GuardDecision::Block(_) => prop_assert!(false, "a model finding blocked"),
        }
    }
}
