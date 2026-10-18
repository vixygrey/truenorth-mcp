//! Guard qualification behavior and failure regressions.

use std::path::Path;

use super::*;
use crate::engine::jev::client_fake::FakeClient;

fn inputs() -> (GuardFixtureSet, AcceptancePolicy, String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let directory = root.join("benchmarks/jev/guard-qualification");
    let (policy, policy_digest) =
        super::super::load_policy(&directory.join("policy-v1.yml")).unwrap();
    let (fixtures, fixture_digest) =
        load_guard_fixtures(&directory.join("fixtures-v1.yml"), &policy).unwrap();
    (fixtures, policy, fixture_digest, policy_digest)
}

#[test]
fn weaker_policy_is_rejected_before_any_provider_call() {
    let (_, mut policy, _, _) = inputs();
    policy
        .limits
        .as_mut()
        .unwrap()
        .maximum_false_negatives_per_signal = 1;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    assert!(
        load_guard_fixtures(
            &root.join("benchmarks/jev/guard-qualification/fixtures-v1.yml"),
            &policy
        )
        .is_err()
    );
}

#[tokio::test]
async fn offline_failure_preserves_privacy_and_cannot_accept() {
    let (fixtures, policy, fixture_digest, policy_digest) = inputs();
    let fake = FakeClient::new();
    let config = JevConfig::default();
    let origin = provenance(&fixtures, fixture_digest, policy_digest, &policy, &config);
    let mut report =
        run_guard_qualification(&fake, &fixtures, &config, None, origin, "fake", || None)
            .await
            .unwrap();
    assert_eq!(
        report.deterministic_cases.len(),
        fixtures.deterministic_cases.len()
    );
    assert!(
        report
            .deterministic_cases
            .iter()
            .all(|case| case.logical_calls == 0 && case.source_unchanged)
    );
    assert!(report.cases[0].source_unchanged);
    assert_eq!(fake.call_count(), 1);
    assert!(report.budget_interrupted);
    evaluate_guard_acceptance(&mut report, &policy);
    assert!(!report.accepted);
    let sanitized = serde_json::to_string(&report).unwrap();
    assert!(!sanitized.contains("fn grant_slots"));
    assert!(!sanitized.contains("svcx_"));
}

#[tokio::test]
async fn reaching_budget_after_rigor_prevents_the_drift_request() {
    use crate::engine::jev::{Answer, Usage};
    let (fixtures, policy, fixture_digest, policy_digest) = inputs();
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-1.13.0".into(),
        answers: BTreeMap::from([
            ("hallucinated_import".into(), Answer::Noul { noul: 0.0 }),
            ("violates_conventions".into(), Answer::Noul { noul: 0.0 }),
            ("contains_secrets".into(), Answer::Noul { noul: 0.0 }),
            (
                "complexity".into(),
                Answer::Score {
                    score: 0.0,
                    legend: BTreeMap::from([("0".into(), "trivial".into())]),
                    probabilities: BTreeMap::from([("0".into(), 1.0)]),
                    confidence: 1.0,
                },
            ),
        ]),
        usage: Usage {
            input_tokens: 1,
            output_tokens: 0,
        },
    });
    let config = JevConfig::default();
    let origin = provenance(&fixtures, fixture_digest, policy_digest, &policy, &config);
    let report = run_guard_qualification(
        &fake,
        &fixtures,
        &config,
        Some(1e-12),
        origin,
        "fake",
        || None,
    )
    .await
    .expect("bounded measurement");
    assert_eq!(
        fake.call_count(),
        1,
        "a second logical request would exceed the reviewed boundary"
    );
    assert_eq!(report.cases.len(), 1);
    assert_eq!(report.cases[0].responses, 1);
    assert!(report.cases[0].unavailable.is_some());
    assert!(report.budget_interrupted);
    assert_eq!(
        report.unknown_usage_calls, 0,
        "known usage must not become an unknown bill"
    );
    assert!(
        report
            .total_estimated_cost_usd
            .is_some_and(|cost| cost > 1e-12)
    );
}

#[tokio::test]
async fn every_completed_proposal_grades_off_focus_findings_and_combined_usage() {
    use crate::engine::jev::{Answer, Usage};
    let (fixtures, policy, fixture_digest, policy_digest) = inputs();
    let off_focus = fixtures
        .cases
        .iter()
        .position(|case| {
            case.focus == "rigor"
                && case.expected.drift_out_of_scope == Some(false)
                && case.expected.decision == "annotate"
        })
        .expect("a rigor proposal whose decision can mask a scope false positive");
    let low_rigor: JevResponse = serde_json::from_value(serde_json::json!({
        "model": policy.concrete_model,
        "answers": {
            "hallucinated_import": {"type": "noul", "noul": 0.0},
            "violates_conventions": {"type": "noul", "noul": 0.0},
            "contains_secrets": {"type": "noul", "noul": 0.0},
            "complexity": {"type": "score", "score": 0.0, "legend": {"0": "trivial"},
                "probabilities": {"0": 1.0}, "confidence": 1.0}
        },
        "usage": {"input_tokens": 1, "output_tokens": 0}
    }))
    .expect("typed response");
    let fake = FakeClient::new();
    for index in 0..fixtures.cases.len() {
        fake.push_response(low_rigor.clone());
        fake.push_response(JevResponse {
            model: policy.concrete_model.clone(),
            answers: BTreeMap::from([(
                "drift".into(),
                Answer::Noul {
                    noul: if index == off_focus { 0.9 } else { 0.0 },
                },
            )]),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 0,
            },
        });
    }
    let config = JevConfig::default();
    let origin = provenance(&fixtures, fixture_digest, policy_digest, &policy, &config);
    let mut report =
        run_guard_qualification(&fake, &fixtures, &config, None, origin, "fake", || None)
            .await
            .expect("complete offline measurements");
    assert_eq!(report.cases.len(), 100);
    assert_eq!(report.logical_attempts, 200);
    assert_eq!(report.unknown_usage_calls, 0);
    for signal in report.signals.values() {
        assert_eq!(
            signal.samples, 100,
            "off-focus observations must not disappear"
        );
    }
    assert_eq!(report.signals["out_of_scope"].false_positives, 1);
    assert!(report.signals["contains_secrets"].false_negatives >= 20);
    assert!(!report.budget_interrupted);
    let expected_cost = 200.0 * policy.pricing.input_per_million / 1_000_000.0;
    assert!((report.total_estimated_cost_usd.expect("known usage") - expected_cost).abs() < 1e-12);
    evaluate_guard_acceptance(&mut report, &policy);
    assert!(
        !report.accepted,
        "successful transport cannot hide missed findings"
    );
}
