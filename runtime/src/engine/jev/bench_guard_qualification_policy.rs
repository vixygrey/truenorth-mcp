//! Whole-guard acceptance, immutable provenance, and guarded report writing.

use super::*;
/// Evaluate acceptance only on real HTTP evidence with complete findings and known usage.
pub fn evaluate_guard_acceptance(report: &mut GuardQualificationReport, policy: &AcceptancePolicy) {
    report.accepted = false;
    let fail = &mut report.failures;
    let Some(limits) = policy.limits.as_ref() else {
        fail.push("reviewed whole-guard limits missing".into());
        return;
    };
    if policy.approval != "approved" {
        fail.push("guard qualification policy pending approval".into());
    }
    if report.client_kind != "http" {
        fail.push("offline fake cannot establish provider acceptance".into());
    }
    if report.budget_interrupted || report.cases.len() != 100 {
        fail.push("qualification incomplete or interrupted".into());
    }
    if report.deterministic_cases.len() < 4
        || report.deterministic_cases.iter().any(|case| {
            case.actual_deterministic != case.expected_deterministic
                || case.actual_decision != "block"
                || case.actual_mcp != "block"
                || case.actual_cli != "block"
                || case.logical_calls != 0
                || !case.source_unchanged
        })
    {
        fail.push("deterministic probes failed or mutated source".into());
    }
    if report.cases.iter().any(|case| !case.source_unchanged) {
        fail.push("guard changed source or created a proposed file".into());
    }
    if report.provenance.code_sha.len() != 40
        || !report
            .provenance
            .code_sha
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        fail.push("immutable code provenance missing".into());
    }
    if report.provenance.requested_model != policy.requested_model
        || report.provenance.concrete_model != policy.concrete_model
        || report.provenance.observed_models.as_slice() != [policy.concrete_model.as_str()]
    {
        fail.push("unverified responding model".into());
    }
    if report.provenance.pricing.version != policy.pricing.version
        || report.provenance.pricing.source != policy.pricing.source
        || report.provenance.pricing.input_per_million != policy.pricing.input_per_million
        || report.provenance.pricing.output_per_million != policy.pricing.output_per_million
    {
        fail.push("pricing provenance mismatch".into());
    }
    if report.cases.iter().any(|case| {
        case.expected_deterministic != case.actual_deterministic
            || case.expected_decision != case.actual_decision
            || case.expected_mcp != case.actual_mcp
            || case.expected_cli != case.actual_cli
            || case.actual_decision == "block"
            || case.actual_deterministic == "allow" && case.unavailable.is_some()
    }) {
        fail.push("guard decision, availability or deterministic policy mismatch".into());
    }
    if report.cases.iter().any(|case| {
        case.actual_deterministic == "allow"
            && ((case.focus == "rigor" && case.findings.len() != 5)
                || (case.focus == "scope" && case.findings.len() != 5))
    }) {
        fail.push("missing required production findings".into());
    }
    for name in [
        "hallucinated_import",
        "violates_conventions",
        "complexity_high",
        "contains_secrets",
        "out_of_scope",
    ] {
        match report.signals.get(name) {
            Some(signal)
                if signal.samples >= limits.minimum_samples_per_aspect
                    && signal.positives >= limits.minimum_positive_per_signal
                    && signal.negatives >= limits.minimum_negative_per_signal
                    && signal.agreement >= limits.minimum_agreement_per_signal
                    && signal.false_positives <= limits.maximum_false_positives_per_signal
                    && signal.false_negatives <= limits.maximum_false_negatives_per_signal => {}
            _ => fail.push(format!("{name}: primary signal coverage or quality missed")),
        }
    }
    if report
        .cases
        .iter()
        .any(|case| case.actual_deterministic != "allow" && case.logical_calls != 0)
    {
        fail.push("deterministic rejection made provider calls".into());
    }
    if report
        .whole_guard_p95_latency_ms
        .is_none_or(|p95| p95 > limits.maximum_p95_latency_ms)
    {
        fail.push("whole guard p95 latency exceeds reviewed limit".into());
    }
    let successes = report
        .cases
        .iter()
        .filter(|case| case.actual_deterministic == "allow")
        .count();
    let errors = report
        .cases
        .iter()
        .filter(|case| {
            case.actual_deterministic == "allow" && (case.logical_calls != 2 || case.responses != 2)
        })
        .count();
    if successes == 0 || errors as f64 / successes.max(1) as f64 > limits.maximum_error_rate {
        fail.push("successful-path error rate exceeds reviewed limit".into());
    }
    if report.unknown_usage_calls != 0
        || report
            .transport_attempts
            .is_none_or(|n| n != report.logical_attempts)
    {
        fail.push("transport retries or failed attempts have unknown billed usage".into());
    }
    if report.cases.iter().any(|case| {
        case.estimated_cost_usd
            .is_none_or(|cost| cost > limits.maximum_cost_per_case)
    }) {
        fail.push("whole-guard combined cost/case exceeds reviewed limit or is unknown".into());
    }
    if report.total_estimated_cost_usd.is_none_or(|cost| {
        cost > limits.maximum_total_cost
            || report
                .run_estimated_budget_usd
                .is_none_or(|budget| cost > budget)
    }) {
        fail.push("total estimated cost exceeds reviewed budget or is unknown".into());
    }
    report.accepted = fail.is_empty();
}

pub fn write_guard_report(
    repo_root: &Path,
    report: &GuardQualificationReport,
) -> Result<String, JevError> {
    let yaml = crate::engine::yaml::to_string(report).map_err(|_| JevError::Config {
        path: REPORT_PATH.into(),
        detail: "cannot serialize guard qualification report".into(),
    })?;
    crate::engine::agent_ws::write_under_agent(repo_root, Path::new(REPORT_PATH), &yaml).map_err(
        |_| JevError::Config {
            path: REPORT_PATH.into(),
            detail: "cannot write guarded qualification report".into(),
        },
    )?;
    Ok(REPORT_PATH.into())
}

pub fn provenance(
    fixtures: &GuardFixtureSet,
    fixture_sha256: String,
    policy_sha256: String,
    policy: &AcceptancePolicy,
    config: &JevConfig,
) -> GuardProvenance {
    GuardProvenance {
        source_name: fixtures.source.name.clone(),
        source_version: fixtures.source.version.clone(),
        source_sha256: fixtures.source.sha256.clone(),
        fixture_sha256,
        policy_sha256,
        code_sha: std::env::var("GITHUB_SHA").unwrap_or_default(),
        config_sha256: crate::engine::digest::sha256(format!("{config:?}").as_bytes()),
        context_rules_sha256: crate::engine::digest::sha256(
            include_str!("secret_filter.rs").as_bytes(),
        ),
        rigor_prompt_sha256: crate::engine::digest::sha256(include_str!("rigor.rs").as_bytes()),
        drift_prompt_sha256: crate::engine::digest::sha256(include_str!("drift.rs").as_bytes()),
        requested_model: policy.requested_model.clone(),
        concrete_model: policy.concrete_model.clone(),
        observed_models: Vec::new(),
        pricing: policy.pricing.clone(),
        scoring: [
            ("rigor_failure_boundary", config.rigor_failure_boundary),
            ("secret_boundary", config.secret_boundary),
            ("complexity_boundary", config.complexity_boundary),
            ("drift_boundary", config.drift_boundary),
        ]
        .into_iter()
        .map(|(name, value)| (name.into(), value))
        .collect(),
        transport: [
            ("retry_limit", u64::from(config.retry_limit)),
            (
                "max_backoff_ms",
                u64::try_from(config.max_backoff.as_millis()).unwrap_or(u64::MAX),
            ),
            (
                "timeout_ms",
                u64::try_from(config.timeout.as_millis()).unwrap_or(u64::MAX),
            ),
        ]
        .into_iter()
        .map(|(name, value)| (name.into(), value))
        .collect(),
    }
}

#[cfg(test)]
#[path = "bench_guard_qualification_policy_tests.rs"]
mod tests;
