//! Behavioral regressions for fixture validation, production evaluators and acceptance accounting.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use tempfile::tempdir;

use super::super::client_fake::{FakeClient, FakeError};
use super::super::config::JevConfig;
use super::super::{Answer, JevResponse, Usage};
use super::*;

fn inputs() -> (FixtureSet, String, AcceptancePolicy, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let path = root.join("benchmarks/jev/smoke/fixtures-v1.yml");
    let policy = root.join("benchmarks/jev/policy-v1.yml");
    let (fixtures, digest) = load_fixtures(&path).expect("versioned fixtures");
    verify_source(&path, &fixtures.source).expect("source digest");
    let (mut policy, policy_digest) = load_policy(&policy).expect("versioned policy");
    // Small behavioral fixtures exercise scoring independently of pilot sample requirements.
    policy.limits = None;
    (fixtures, digest, policy, policy_digest)
}

fn rigor_response(import: f64, conventions: f64, complexity: f64, secrets: f64) -> JevResponse {
    JevResponse {
        model: "jev-1.13.0".into(),
        answers: BTreeMap::from([
            ("hallucinated_import".into(), Answer::Noul { noul: import }),
            (
                "violates_conventions".into(),
                Answer::Noul { noul: conventions },
            ),
            ("contains_secrets".into(), Answer::Noul { noul: secrets }),
            (
                "complexity".into(),
                Answer::Score {
                    score: complexity,
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

fn drift_response(noul: f64) -> JevResponse {
    JevResponse {
        model: "jev-1.13.0".into(),
        answers: BTreeMap::from([("drift".into(), Answer::Noul { noul })]),
        usage: Usage {
            input_tokens: 50,
            output_tokens: 5,
        },
    }
}

fn approved(mut policy: AcceptancePolicy) -> AcceptancePolicy {
    policy.approval = "approved".into();
    policy.limits = Some(Limits {
        minimum_samples_per_aspect: 2,
        minimum_positive_per_signal: 1,
        minimum_negative_per_signal: 1,
        minimum_agreement_per_signal: 1.0,
        maximum_false_positives_per_signal: 0,
        maximum_false_negatives_per_signal: 0,
        maximum_p95_latency_ms: 1_000,
        maximum_error_rate: 0.0,
        maximum_cost_per_case: 1.0,
        maximum_total_cost: 1.0,
    });
    policy
}

fn report(
    fixtures: &FixtureSet,
    fixture_digest: &str,
    policy: &AcceptancePolicy,
    policy_digest: &str,
    metrics: Vec<AspectMetrics>,
    models: Vec<String>,
    transport_attempts: Option<u64>,
) -> BenchmarkReport {
    let maximum_fixture_cost = metrics
        .iter()
        .filter_map(|m| m.maximum_case_cost)
        .reduce(f64::max);
    let maximum_fixture_cost = if metrics.iter().any(|m| m.unknown_usage_calls != 0) {
        None
    } else {
        maximum_fixture_cost
    };
    BenchmarkReport {
        schema_version: 2,
        provenance: Provenance {
            source_name: fixtures.source.name.clone(),
            source_version: fixtures.source.version.clone(),
            source_sha256: fixtures.source.sha256.clone(),
            fixture_sha256: fixture_digest.into(),
            policy_sha256: policy_digest.into(),
            requested_model: policy.requested_model.clone(),
            concrete_model: policy.concrete_model.clone(),
            observed_models: models,
            pricing: policy.pricing.clone(),
            scoring: BTreeMap::from([
                ("rigor_failure_boundary".into(), 0.70),
                ("drift_boundary".into(), 0.25),
                ("complexity_boundary".into(), 70.0),
            ]),
            transport: BTreeMap::from([
                ("retry_limit".into(), 3),
                ("max_backoff_ms".into(), 8_000),
                ("timeout_ms".into(), 30_000),
            ]),
        },
        client_kind: "http".into(),
        metrics,
        case_diagnostics: Vec::new(),
        transport_attempts,
        maximum_fixture_cost,
        run_estimated_budget_usd: Some(1.0),
        budget_interrupted: false,
        accepted: false,
        failures: Vec::new(),
    }
}

#[test]
fn rejects_missing_duplicate_unknown_and_uncovered_cases() {
    let dir = tempdir().expect("dir");
    let path = dir.path().join("fixtures-v1.yml");
    for payload in [
        "schema_version: 1\nsource: {name: synthetic, version: '1', sha256: abc}\ncases: []\n",
        "schema_version: 99\nsource: {name: synthetic, version: '1', sha256: abc}\ncases: []\n",
        "schema_version: 1\nunknown: 12\n",
    ] {
        std::fs::write(&path, payload).expect("input");
        assert!(load_fixtures(&path).is_err());
    }
    // Construct malformed test inputs from actual fixture bytes, not handwritten lookalikes.
    let original = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("benchmarks/jev/smoke/fixtures-v1.yml"),
    )
    .unwrap();
    std::fs::write(&path, original.replace("rigor-risk-02", "rigor-clean-01")).unwrap();
    assert!(load_fixtures(&path).is_err());
    let only_rigor = original
        .split("- case_id: drift-in-scope-01")
        .next()
        .unwrap();
    std::fs::write(&path, only_rigor).unwrap();
    assert!(load_fixtures(&path).is_err());
    std::fs::write(
        &path,
        original.replace("complexity_high: false", "complexity_high: unsupported"),
    )
    .unwrap();
    assert!(load_fixtures(&path).is_err());
    std::fs::write(
        &path,
        original.replace("path: src/example.rs", "path: ../escape.rs"),
    )
    .unwrap();
    assert!(load_fixtures(&path).is_err());
}

#[test]
fn source_digest_and_policy_schema_are_checked() {
    let (fixtures, _, _, _) = inputs();
    let dir = tempdir().unwrap();
    let path = dir.path().join("fixtures-v1.yml");
    std::fs::write(dir.path().join("source-v1.yml"), "forged source").unwrap();
    assert!(verify_source(&path, &fixtures.source).is_err());
    let policy_path = dir.path().join("policy-v1.yml");
    std::fs::write(&policy_path, "schema_version: 1\nunknown: unsupported\n").unwrap();
    assert!(load_policy(&policy_path).is_err());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let original = std::fs::read_to_string(root.join("benchmarks/jev/policy-v1.yml")).unwrap();
    std::fs::write(
        &policy_path,
        format!(
            "{}limits: null\n",
            original.split("limits:").next().unwrap()
        )
        .replace("approval: pending", "approval: approved"),
    )
    .unwrap();
    assert!(
        load_policy(&policy_path).is_err(),
        "approved policies require numerical limits"
    );
    std::fs::write(
        &policy_path,
        original.replace("input_per_million: 0.042", "input_per_million: -1"),
    )
    .unwrap();
    assert!(
        load_policy(&policy_path).is_err(),
        "negative pricing cannot pass"
    );
}

#[test]
fn coverage_policy_rejects_one_sided_or_too_small_suite_before_any_call() {
    let (mut fixtures, _, policy, _) = inputs();
    let path = Path::new("benchmarks/jev/smoke/fixtures-v1.yml");
    validate_coverage(&fixtures, &policy, path).expect("two-sided checked-in cases");
    fixtures
        .cases
        .retain(|case| case.case_id != "drift-out-of-scope-02");
    assert!(validate_coverage(&fixtures, &policy, path).is_err());
    let (fixtures, _, policy, _) = inputs();
    let mut stricter = approved(policy);
    stricter.limits.as_mut().unwrap().minimum_samples_per_aspect = 4;
    assert!(validate_coverage(&fixtures, &stricter, path).is_err());
}

#[tokio::test]
async fn invalid_later_state_is_rejected_before_any_client_call() {
    let (mut fixtures, _, policy, _) = inputs();
    fixtures
        .cases
        .last_mut()
        .unwrap()
        .drift
        .as_mut()
        .unwrap()
        .task = "secret".into();
    let fake = FakeClient::new();
    assert!(
        run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
            .await
            .is_err()
    );
    assert_eq!(fake.call_count(), 0);
}

#[tokio::test]
async fn reviewed_estimated_ceiling_stops_before_further_billable_cases() {
    let (fixtures, digest, policy, policy_digest) = inputs();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    let measured = run_benchmark_with_budget(
        &fake,
        &fixtures,
        &JevConfig::default(),
        &policy.pricing,
        Some(0.000004),
    )
    .await
    .expect("bounded runner");
    assert_eq!(
        fake.call_count(),
        1,
        "no second case is requested after known cap breach"
    );
    assert!(measured.budget_interrupted);
    let mut rejected = report(
        &fixtures,
        &digest,
        &policy,
        &policy_digest,
        measured.metrics,
        measured.models,
        Some(1),
    );
    rejected.budget_interrupted = true;
    rejected.run_estimated_budget_usd = Some(0.000004);
    evaluate_acceptance(&mut rejected, &approved(policy));
    assert!(!rejected.accepted);
    assert!(
        rejected
            .failures
            .iter()
            .any(|message| message.contains("budget"))
    );
}

#[tokio::test]
async fn unknown_failed_call_usage_interrupts_bounded_run() {
    let (fixtures, _, policy, _) = inputs();
    let fake = FakeClient::new();
    fake.push_error(FakeError::Timeout { timeout_ms: 5 });
    let measured = run_benchmark_with_budget(
        &fake,
        &fixtures,
        &JevConfig::default(),
        &policy.pricing,
        Some(1.0),
    )
    .await
    .expect("bounded runner");
    assert_eq!(fake.call_count(), 1);
    assert!(measured.budget_interrupted);
    assert_eq!(measured.metrics[0].unknown_usage_calls, 1);
}

#[tokio::test]
async fn production_state_builders_and_per_signal_confusion_are_scored() {
    let (fixtures, fixture_digest, policy, policy_digest) = inputs();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(rigor_response(1.0, 1.0, 0.0, 0.0));
    fake.push_response(rigor_response(0.0, 0.0, 4.0, 1.0));
    fake.push_response(drift_response(0.0));
    fake.push_response(drift_response(1.0));
    let measurements = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("runner");
    assert!(measurements.maximum_fixture_cost.is_some());
    let (metrics, models) = (measurements.metrics, measurements.models);
    assert_eq!(fake.call_count(), 5);
    let calls = fake.calls();
    assert!(
        calls[0].state.is_array(),
        "production rigor state has filtered file array"
    );
    assert_eq!(
        calls[0]
            .questions
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert!(
        calls[3].state.get("task").is_some(),
        "production drift scope contains the task"
    );
    assert!(calls[3].state.get("protected_paths").is_some());
    let mut result = report(
        &fixtures,
        &fixture_digest,
        &policy,
        &policy_digest,
        metrics,
        models,
        Some(5),
    );
    assert_eq!(result.metrics[0].matches, 12);
    assert_eq!(result.metrics[0].input_tokens, 300);
    assert_eq!(
        result.metrics[0].signals["complexity_high"].true_positives,
        1
    );
    assert_eq!(result.metrics[1].signals["out_of_scope"].true_negatives, 1);
    assert!(
        result
            .metrics
            .iter()
            .all(|m| m.errors == 0 && m.estimated_cost.is_some())
    );
    evaluate_acceptance(&mut result, &policy);
    assert!(
        !result.accepted,
        "unapproved policy always rejects perfect offline responses"
    );
    result.failures.clear();
    let mut candidate = approved(policy);
    candidate.approval = "pending".into();
    evaluate_acceptance(&mut result, &candidate);
    assert!(
        !result.accepted,
        "numeric candidates cannot pass until approved"
    );
    result.failures.clear();
    candidate.approval = "approved".into();
    evaluate_acceptance(&mut result, &candidate);
    assert!(
        result.accepted,
        "reviewed synthetic policy boundary permits exact matching counts"
    );
    let mut stale = result.clone();
    stale.provenance.pricing.version = "stale-price".into();
    evaluate_acceptance(&mut stale, &candidate);
    assert!(!stale.accepted);
    assert!(
        stale
            .failures
            .iter()
            .any(|s| s.contains("pricing provenance"))
    );
    let mut slow = result.clone();
    slow.metrics[0].p95_latency_ms = 1_001;
    evaluate_acceptance(&mut slow, &candidate);
    assert!(!slow.accepted);
    assert!(slow.failures.iter().any(|s| s.contains("p95 latency")));
    let mut retried = result.clone();
    retried.transport_attempts = Some(6);
    evaluate_acceptance(&mut retried, &candidate);
    assert!(
        !retried.accepted,
        "a successful response cannot conceal unpriced retries"
    );
}

#[tokio::test]
async fn combined_fixture_cost_is_enforced_across_aspects() {
    let (mut fixtures, digest, policy, policy_digest) = inputs();
    let scope = fixtures.cases.pop().expect("out-of-scope drift case");
    fixtures.cases[0].drift = scope.drift;
    fixtures.cases[0].expected.drift_out_of_scope = scope.expected.drift_out_of_scope;
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(1.0));
    fake.push_response(rigor_response(1.0, 1.0, 0.0, 1.0));
    fake.push_response(rigor_response(0.0, 0.0, 4.0, 1.0));
    fake.push_response(drift_response(0.0));
    let measured = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("runner");
    let max_cost = measured.maximum_fixture_cost.expect("known usage");
    assert!((max_cost - 150.0 * 0.042 / 1_000_000.0).abs() < 1e-12);
    let mut result = report(
        &fixtures,
        &digest,
        &policy,
        &policy_digest,
        measured.metrics,
        measured.models,
        Some(5),
    );
    result.maximum_fixture_cost = Some(max_cost);
    let mut candidate = approved(policy);
    candidate.limits.as_mut().unwrap().maximum_cost_per_case = 0.000005;
    evaluate_acceptance(&mut result, &candidate);
    assert!(!result.accepted);
    assert!(
        result
            .failures
            .iter()
            .any(|message| message.contains("per-fixture combined cost"))
    );
}

#[tokio::test]
async fn false_positive_and_false_negative_fail_independent_rigor_limits() {
    let (fixtures, fixture_digest, policy, policy_digest) = inputs();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(1.0, 0.0, 0.0, 1.0));
    fake.push_response(rigor_response(0.0, 1.0, 0.0, 0.0));
    fake.push_response(rigor_response(0.0, 0.0, 4.0, 0.0));
    fake.push_response(drift_response(0.0));
    fake.push_response(drift_response(1.0));
    let measurements = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("runner");
    let (metrics, models) = (measurements.metrics, measurements.models);
    assert_eq!(metrics[0].signals["hallucinated_import"].false_positives, 1);
    assert_eq!(metrics[0].signals["hallucinated_import"].false_negatives, 1);
    assert_eq!(metrics[0].signals["contains_secrets"].false_positives, 1);
    assert_eq!(metrics[0].signals["contains_secrets"].false_negatives, 1);
    let mut result = report(
        &fixtures,
        &fixture_digest,
        &policy,
        &policy_digest,
        metrics,
        models,
        Some(5),
    );
    evaluate_acceptance(&mut result, &approved(policy));
    assert!(!result.accepted);
    assert!(
        result
            .failures
            .iter()
            .any(|f| f.contains("rigor.hallucinated_import"))
    );
}

#[tokio::test]
async fn errors_unknown_usage_and_alias_drift_never_pass_cost_accounting() {
    let (fixtures, fixture_digest, policy, policy_digest) = inputs();
    let fake = FakeClient::new();
    fake.push_error(FakeError::Timeout { timeout_ms: 5 });
    fake.push_response(rigor_response(1.0, 1.0, 0.0, 1.0));
    fake.push_response(rigor_response(0.0, 0.0, 4.0, 0.0));
    fake.push_response(drift_response(0.0));
    fake.push_response(drift_response(1.0));
    let measurements = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("runner");
    assert!(measurements.maximum_fixture_cost.is_none());
    let (metrics, models) = (measurements.metrics, measurements.models);
    assert_eq!(metrics[0].attempts, 3);
    assert_eq!(metrics[0].errors, 1);
    assert_eq!(metrics[0].unknown_usage_calls, 1);
    assert!(metrics[0].estimated_cost.is_none());
    let mut result = report(
        &fixtures,
        &fixture_digest,
        &policy,
        &policy_digest,
        metrics,
        models,
        Some(5),
    );
    evaluate_acceptance(&mut result, &approved(policy));
    assert!(!result.accepted);
    assert!(result.failures.iter().any(|s| s.contains("usage/cost")));
    result.failures.clear();
    result.provenance.observed_models = vec!["jev-latest".into()];
    evaluate_acceptance(&mut result, &approved(inputs().2));
    assert!(
        result
            .failures
            .iter()
            .any(|s| s.contains("responding model"))
    );
}

#[tokio::test]
async fn untrusted_responder_identity_is_not_copied_into_aggregate_evidence() {
    let (fixtures, _, policy, _) = inputs();
    let fake = FakeClient::new();
    let mut response = rigor_response(0.0, 0.0, 0.0, 0.0);
    response.model = "unexpected provider payload: synthetic sensitive text".into();
    fake.push_response(response);
    let measured = run_benchmark(&fake, &fixtures, &JevConfig::default(), &policy.pricing)
        .await
        .expect("runner");
    assert_eq!(measured.models.len(), 1);
    assert_eq!(measured.models[0], "<invalid responder identity>");
    assert!(!format!("{:?}", measured.models).contains("synthetic sensitive text"));
}

#[test]
fn aggregate_report_rejects_symlink_escape_and_leaks_no_fixture_content() {
    let (fixtures, fixture_digest, policy, policy_digest) = inputs();
    let dir = tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".agent")).unwrap();
    let report = report(
        &fixtures,
        &fixture_digest,
        &policy,
        &policy_digest,
        Vec::new(),
        Vec::new(),
        None,
    );
    write_report(dir.path(), &report).expect("guarded write");
    let bytes =
        std::fs::read_to_string(dir.path().join(".agent/telemetry/jev-benchmark.yml")).unwrap();
    assert!(!bytes.contains("pretend_runtime_library"));
    assert!(!bytes.contains("src/example.rs"));
    let outside = tempdir().unwrap();
    std::fs::remove_dir_all(dir.path().join(".agent/telemetry")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".agent/telemetry")).unwrap();
    assert!(write_report(dir.path(), &report).is_err());
    assert!(!outside.path().join("jev-benchmark.yml").exists());
}

#[test]
fn calibration_labels_match_the_filtered_production_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let path = root.join("benchmarks/jev/fixtures-v1.yml");
    let (fixtures, _) = load_fixtures(&path).expect("calibration fixtures");
    verify_source(&path, &fixtures.source).expect("manifest digest");
    let (policy, _) = load_policy(&root.join("benchmarks/jev/policy-v1.yml")).unwrap();
    validate_coverage(&fixtures, &policy, &path).expect("candidate coverage");
    assert_eq!(fixtures.cases.len(), 100);
    for case in fixtures
        .cases
        .iter()
        .filter(|case| case.expected.rigor.is_some())
    {
        let state = super::run::rigor_state(case).expect("production filtered state");
        let entries = state.as_array().expect("file array");
        assert_eq!(entries.len(), case.files.len(), "{}", case.case_id);
        for (entry, file) in entries.iter().zip(&case.files) {
            assert_eq!(entry["path"], file.path);
            assert_eq!(
                entry["content"].as_str().unwrap(),
                file.content.trim_end_matches('\n'),
                "{}",
                case.case_id
            );
        }
        let labels = case.expected.rigor.as_ref().unwrap();
        let code = entries.last().unwrap()["content"].as_str().unwrap();
        assert_eq!(code.contains("tok_demo_"), labels.contains_secrets);
        assert_eq!(code.contains(".unwrap()"), labels.violates_conventions);
        assert_eq!(
            code.contains("let mut queue") || code.contains("let mut pending"),
            labels.complexity_high
        );
        assert_eq!(
            code.contains("use std::collections::VecDeque;"),
            !labels.hallucinated_import
        );
    }
}
