//! Behavioral coverage for bounded, privacy-safe per-case benchmark diagnostics.

use std::collections::BTreeMap;
use std::path::Path;

use super::super::client_fake::{FakeClient, FakeError};
use super::super::config::JevConfig;
use super::super::{Answer, JevResponse, Usage};
use super::*;

fn inputs() -> (FixtureSet, String, AcceptancePolicy, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let (fixtures, fixture_digest) =
        load_fixtures(&root.join("benchmarks/jev/smoke/fixtures-v1.yml")).expect("smoke fixtures");
    let (policy, policy_digest) =
        load_policy(&root.join("benchmarks/jev/policy-v1.yml")).expect("benchmark policy");
    (fixtures, fixture_digest, policy, policy_digest)
}

fn rigor_response(import: f64, complexity_index: f64) -> JevResponse {
    JevResponse {
        model: "jev-1.13.0".into(),
        answers: BTreeMap::from([
            ("hallucinated_import".into(), Answer::Noul { noul: import }),
            ("violates_conventions".into(), Answer::Noul { noul: 0.0 }),
            ("contains_secrets".into(), Answer::Noul { noul: 0.0 }),
            (
                "complexity".into(),
                Answer::Score {
                    score: complexity_index,
                    legend: BTreeMap::new(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.9,
                },
            ),
        ]),
        usage: Usage {
            input_tokens: 100,
            output_tokens: 20,
        },
    }
}

fn drift_response(score: f64) -> JevResponse {
    JevResponse {
        model: "jev-1.13.0".into(),
        answers: BTreeMap::from([("drift".into(), Answer::Noul { noul: score })]),
        usage: Usage {
            input_tokens: 50,
            output_tokens: 5,
        },
    }
}

fn combined_case(fixtures: &mut FixtureSet) {
    let drift = fixtures.cases[3].drift.clone();
    fixtures.cases.truncate(1);
    fixtures.cases[0].drift = drift;
    fixtures.cases[0].expected.drift_out_of_scope = Some(false);
}

#[tokio::test]
async fn exact_boundaries_preserve_classification_and_normalize_only_display_scores() {
    let (mut fixtures, _, policy, _) = inputs();
    combined_case(&mut fixtures);
    let config = JevConfig::default();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(config.rigor_failure_boundary, 2.8));
    fake.push_response(drift_response(config.drift_boundary));
    let measured = run_benchmark(&fake, &fixtures, &config, &policy.pricing)
        .await
        .expect("offline benchmark");
    assert_eq!(fake.call_count(), 2);
    assert_eq!(measured.case_diagnostics.len(), 2);
    let rigor = &measured.case_diagnostics[0];
    let drift = &measured.case_diagnostics[1];
    for diagnostic in [rigor, drift] {
        assert_eq!(diagnostic.case_index, 1);
        assert_eq!(
            diagnostic.case_id_sha256,
            crate::engine::digest::sha256(fixtures.cases[0].case_id.as_bytes())
        );
        assert_eq!(diagnostic.case_id_sha256.len(), 64);
        assert_eq!(diagnostic.error, None);
    }
    assert_eq!(rigor.aspect, "rigor");
    assert_eq!(drift.aspect, "drift");
    let import = &rigor.signals["hallucinated_import"];
    assert!(!import.expected);
    assert_eq!(import.score, config.rigor_failure_boundary);
    assert_eq!(import.threshold, config.rigor_failure_boundary);
    assert!(import.actual, "a score exactly on the boundary is positive");
    let complexity = &rigor.signals["complexity_high"];
    assert!(!complexity.expected);
    assert_eq!(complexity.score, 0.7);
    assert_eq!(complexity.threshold, config.complexity_boundary / 100.0);
    assert!(complexity.actual, "the 70/100 boundary is inclusive");
    let scope = &drift.signals["out_of_scope"];
    assert!(!scope.expected);
    assert_eq!(scope.score, config.drift_boundary);
    assert_eq!(scope.threshold, config.drift_boundary);
    assert!(scope.actual);
    assert_eq!(
        measured.metrics[0].signals["hallucinated_import"].false_positives,
        1
    );
    assert_eq!(
        measured.metrics[0].signals["complexity_high"].false_positives,
        1
    );
    assert_eq!(
        measured.metrics[1].signals["out_of_scope"].false_positives,
        1
    );
}

#[tokio::test]
async fn scores_below_boundaries_keep_negative_production_decisions() {
    let (mut fixtures, _, policy, _) = inputs();
    combined_case(&mut fixtures);
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.69, 2.0));
    fake.push_response(drift_response(0.24));
    let measured = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("offline benchmark");
    let rigor = &measured.case_diagnostics[0].signals;
    let drift = &measured.case_diagnostics[1].signals["out_of_scope"];
    assert!(!rigor["hallucinated_import"].actual);
    assert!(!rigor["complexity_high"].actual);
    assert_eq!(rigor["complexity_high"].score, 0.5);
    assert!(!drift.actual);
    assert_eq!(drift.score, 0.24);
    assert_eq!(
        measured.metrics[0].signals["hallucinated_import"].true_negatives,
        1
    );
    assert_eq!(
        measured.metrics[1].signals["out_of_scope"].true_negatives,
        1
    );
}

#[tokio::test]
async fn partial_response_and_sensitive_failure_are_unscored_and_never_serialized() {
    let (mut fixtures, fixture_digest, policy, policy_digest) = inputs();
    combined_case(&mut fixtures);
    let sensitive_id = "PRIVATE_CASE_IDENTIFIER_k97Z";
    let sensitive_context = "PRIVATE_CONTEXT_BODY_t17H";
    let sensitive_error = "PRIVATE_PROVIDER_ERROR_b42Q";
    fixtures.cases[0].case_id = sensitive_id.into();
    fixtures.cases[0].files[0].content = format!("fn example() {{ /* {sensitive_context} */ }}");
    let fake = FakeClient::new();
    let mut partial = rigor_response(1.0, 4.0);
    partial.answers.remove("complexity");
    fake.push_response(partial);
    fake.push_error(FakeError::Network {
        detail: sensitive_error.into(),
    });
    let measured = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("offline benchmark");
    assert_eq!(fake.call_count(), 2);
    assert_eq!(measured.case_diagnostics.len(), 2);
    for diagnostic in &measured.case_diagnostics {
        assert_eq!(diagnostic.case_index, 1);
        assert_eq!(
            diagnostic.case_id_sha256,
            crate::engine::digest::sha256(sensitive_id.as_bytes())
        );
        assert!(
            diagnostic.signals.is_empty(),
            "partial signals must not masquerade as scored"
        );
        assert_eq!(diagnostic.error, Some("evaluation_failed"));
    }
    assert_eq!(measured.metrics[0].attempts, 1);
    assert_eq!(measured.metrics[0].errors, 1);
    assert_eq!(measured.metrics[0].responses, 1);
    assert!(measured.metrics[0].signals.is_empty());
    assert_eq!(measured.metrics[1].attempts, 1);
    assert_eq!(measured.metrics[1].errors, 1);
    assert_eq!(measured.metrics[1].responses, 0);
    let report = BenchmarkReport {
        schema_version: 2,
        provenance: Provenance {
            source_name: fixtures.source.name.clone(),
            source_version: fixtures.source.version.clone(),
            source_sha256: fixtures.source.sha256.clone(),
            fixture_sha256: fixture_digest.clone(),
            policy_sha256: policy_digest,
            requested_model: policy.requested_model.clone(),
            concrete_model: policy.concrete_model.clone(),
            observed_models: measured.models,
            pricing: policy.pricing,
            scoring: BTreeMap::new(),
            transport: BTreeMap::new(),
        },
        client_kind: "fake".into(),
        metrics: measured.metrics,
        case_diagnostics: measured.case_diagnostics,
        transport_attempts: None,
        maximum_fixture_cost: measured.maximum_fixture_cost,
        run_estimated_budget_usd: None,
        budget_interrupted: measured.budget_interrupted,
        accepted: false,
        failures: Vec::new(),
    };
    let serialized = crate::engine::yaml::to_string(&report).expect("serializable report");
    assert!(serialized.contains("schema_version: 2"));
    assert!(serialized.contains("case_diagnostics:"));
    assert!(serialized.contains(&fixture_digest));
    assert!(serialized.contains(&crate::engine::digest::sha256(sensitive_id.as_bytes())));
    for private in [
        sensitive_id,
        sensitive_context,
        sensitive_error,
        "src/example.rs",
    ] {
        assert!(
            !serialized.contains(private),
            "report exposed private case data"
        );
    }
}

#[tokio::test]
async fn malformed_numeric_response_is_explicitly_unscorable() {
    let (mut fixtures, _, policy, _) = inputs();
    fixtures.cases.truncate(1);
    let fake = FakeClient::new();
    fake.push_response(rigor_response(f64::NAN, 4.0));
    let measured = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("offline benchmark");
    assert_eq!(measured.case_diagnostics.len(), 1);
    let diagnostic = &measured.case_diagnostics[0];
    assert_eq!(diagnostic.error, Some("evaluation_failed"));
    assert!(diagnostic.signals.is_empty());
    assert_eq!(measured.metrics[0].responses, 1);
    assert_eq!(measured.metrics[0].errors, 1);
    assert!(measured.metrics[0].signals.is_empty());
}

#[tokio::test]
async fn budget_omits_every_evaluation_not_attempted() {
    let (mut fixtures, _, policy, _) = inputs();
    let drift = fixtures.cases[3].drift.clone();
    fixtures.cases.truncate(2);
    fixtures.cases[0].drift = drift;
    fixtures.cases[0].expected.drift_out_of_scope = Some(false);
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0));
    let measured = run_benchmark_with_budget(
        &fake,
        &fixtures,
        &JevConfig::default(),
        &policy.pricing,
        Some(0.000004),
    )
    .await
    .expect("bounded offline benchmark");
    assert!(measured.budget_interrupted);
    assert_eq!(fake.call_count(), 1);
    assert_eq!(measured.metrics[0].attempts, 1);
    assert_eq!(measured.case_diagnostics.len(), 1);
    assert_eq!(measured.case_diagnostics[0].case_index, 1);
    assert_eq!(measured.case_diagnostics[0].aspect, "rigor");
    assert_eq!(measured.case_diagnostics[0].signals.len(), 4);
}
