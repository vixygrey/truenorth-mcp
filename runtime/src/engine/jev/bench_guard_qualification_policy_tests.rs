//! Synthetic acceptance inputs are not provider evidence.

use super::*;

fn measured_case(case: &GuardCase, index: usize) -> GuardCaseResult {
    let mut findings = BTreeMap::new();
    if let Some(rigor) = &case.expected.rigor {
        for (name, expected, threshold) in [
            ("hallucinated_import", rigor.hallucinated_import, 0.7),
            ("violates_conventions", rigor.violates_conventions, 0.7),
            ("contains_secrets", rigor.contains_secrets, 0.25),
            ("complexity_high", rigor.complexity_high, 0.625),
            (
                "out_of_scope",
                case.expected.drift_out_of_scope.unwrap(),
                0.5,
            ),
        ] {
            findings.insert(
                name,
                SignalDiagnostic {
                    expected,
                    actual: expected,
                    score: if expected { 1.0 } else { 0.0 },
                    threshold,
                },
            );
        }
    }
    let logical_calls = if case.expected.deterministic == "allow" {
        2
    } else {
        0
    };
    GuardCaseResult {
        case_index: index + 1,
        case_id_sha256: crate::engine::digest::sha256(case.case_id.as_bytes()),
        focus: case.focus.clone(),
        expected_deterministic: case.expected.deterministic.clone(),
        actual_deterministic: case.expected.deterministic.clone(),
        expected_decision: case.expected.decision.clone(),
        actual_decision: case.expected.decision.clone(),
        expected_mcp: case.expected.mcp.clone(),
        actual_mcp: case.expected.mcp.clone(),
        expected_cli: case.expected.cli.clone(),
        actual_cli: case.expected.cli.clone(),
        findings,
        unavailable: None,
        unavailable_context: Vec::new(),
        elapsed_ms: 20,
        logical_calls,
        responses: logical_calls,
        transport_attempts: Some(logical_calls),
        unknown_usage_calls: 0,
        estimated_cost_usd: Some(if logical_calls == 0 { 0.0 } else { 0.00001 }),
        source_unchanged: true,
    }
}

fn qualifying_measurements() -> (GuardQualificationReport, AcceptancePolicy) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("benchmarks/jev/guard-qualification");
    let (mut policy, policy_digest) =
        super::super::super::load_policy(&directory.join("policy-v1.yml")).unwrap();
    let (fixtures, fixture_digest) =
        load_guard_fixtures(&directory.join("fixtures-v1.yml"), &policy).unwrap();
    policy.approval = "approved".into();
    let mut origin = provenance(
        &fixtures,
        fixture_digest,
        policy_digest,
        &policy,
        &JevConfig::default(),
    );
    origin.code_sha = "a".repeat(40);
    origin.observed_models = vec![policy.concrete_model.clone()];
    let cases: Vec<_> = fixtures
        .cases
        .iter()
        .enumerate()
        .map(|(i, case)| measured_case(case, i))
        .collect();
    let mut signals: BTreeMap<String, SignalMetrics> = BTreeMap::new();
    for case in &cases {
        for (name, finding) in &case.findings {
            signals
                .entry((*name).into())
                .or_default()
                .record(finding.expected, finding.actual);
        }
    }
    (
        GuardQualificationReport {
            schema_version: 1,
            provenance: origin,
            client_kind: "http".into(),
            deterministic_cases: fixtures
                .deterministic_cases
                .iter()
                .enumerate()
                .map(|(i, case)| measured_case(case, i))
                .collect(),
            cases,
            signals,
            whole_guard_p95_latency_ms: Some(20),
            logical_attempts: 200,
            transport_attempts: Some(200),
            unknown_usage_calls: 0,
            total_estimated_cost_usd: Some(0.001),
            maximum_case_cost_usd: Some(0.00001),
            run_estimated_budget_usd: Some(0.25),
            budget_interrupted: false,
            accepted: false,
            failures: Vec::new(),
        },
        policy,
    )
}

#[test]
fn complete_measurements_accept_but_each_safety_violation_rejects() {
    let (mut complete, policy) = qualifying_measurements();
    evaluate_guard_acceptance(&mut complete, &policy);
    assert!(complete.accepted, "{:?}", complete.failures);
    type Mutation = fn(&mut GuardQualificationReport, &mut AcceptancePolicy);
    let violations: &[(&str, Mutation)] = &[
        ("limits absent", |_, policy| policy.limits = None),
        ("pending approval", |_, policy| {
            policy.approval = "pending".into()
        }),
        ("fake client", |report, _| {
            report.client_kind = "fake".into()
        }),
        ("missing deterministic probes", |report, _| {
            report.deterministic_cases.clear()
        }),
        ("deterministic MCP bypass", |report, _| {
            report.deterministic_cases[0].actual_mcp = "allow".into()
        }),
        ("source mutation", |report, _| {
            report.cases[0].source_unchanged = false
        }),
        ("invalid source identity", |report, _| {
            report.provenance.code_sha = "z".repeat(40)
        }),
        ("changed model", |report, _| {
            report.provenance.observed_models = vec!["unexpected-model".into()]
        }),
        ("changed price", |report, _| {
            report.provenance.pricing.input_per_million = 0.0
        }),
        ("CLI mapping failure", |report, _| {
            report.cases[0].actual_cli = "block".into()
        }),
        ("MCP mapping failure", |report, _| {
            report.cases[0].actual_mcp = "block".into()
        }),
        ("missing finding", |report, _| {
            report.cases[0].findings.remove("contains_secrets");
        }),
        ("missed positive", |report, _| {
            report
                .signals
                .get_mut("contains_secrets")
                .unwrap()
                .false_negatives = 1;
        }),
        ("unexpected provider call", |report, _| {
            report.cases[0].actual_deterministic = "secret".into()
        }),
        ("slow full guard", |report, _| {
            report.whole_guard_p95_latency_ms = Some(10001)
        }),
        ("missing timing", |report, _| {
            report.whole_guard_p95_latency_ms = None
        }),
        ("incomplete response", |report, _| {
            report.cases[0].responses = 1
        }),
        ("unknown usage", |report, _| report.unknown_usage_calls = 1),
        ("retry not free", |report, _| {
            report.transport_attempts = Some(201)
        }),
        ("missing attempt counts", |report, _| {
            report.transport_attempts = None
        }),
        ("unknown case cost", |report, _| {
            report.cases[0].estimated_cost_usd = None
        }),
        ("excessive case cost", |report, _| {
            report.cases[0].estimated_cost_usd = Some(0.0021)
        }),
        ("unknown total cost", |report, _| {
            report.total_estimated_cost_usd = None
        }),
        ("excessive total cost", |report, _| {
            report.total_estimated_cost_usd = Some(0.251)
        }),
        ("reviewed budget reached", |report, _| {
            report.run_estimated_budget_usd = Some(0.0005)
        }),
    ];
    for (name, mutate) in violations {
        let (mut report, mut policy) = qualifying_measurements();
        mutate(&mut report, &mut policy);
        evaluate_guard_acceptance(&mut report, &policy);
        assert!(!report.accepted, "accepted {name}");
        assert!(
            !report.failures.is_empty(),
            "missing rejection evidence for {name}"
        );
    }
}

#[test]
fn guarded_report_writer_rejects_escape_through_a_telemetry_symlink() {
    let (report, _) = qualifying_measurements();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(".agent")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join(".agent/telemetry")).unwrap();
    assert!(write_guard_report(root.path(), &report).is_err());
    assert!(!outside.path().join("jev-guard-qualification.yml").exists());
}

#[test]
fn removing_required_policy_cannot_preserve_an_old_accepted_flag() {
    let (mut report, mut policy) = qualifying_measurements();
    evaluate_guard_acceptance(&mut report, &policy);
    assert!(report.accepted);
    policy.limits = None;
    evaluate_guard_acceptance(&mut report, &policy);
    assert!(
        !report.accepted,
        "missing approval limits must fail closed even on reevaluation"
    );
}
