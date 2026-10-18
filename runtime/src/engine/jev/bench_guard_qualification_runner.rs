//! Complete-guard execution and finding measurements.

use crate::engine::jev::guard::evaluate::GuardObservation;

use super::*;
fn decision_name(decision: &GuardDecision) -> &'static str {
    match decision {
        GuardDecision::Allow { .. } => "allow",
        GuardDecision::Annotate { .. } => "annotate",
        GuardDecision::Block(_) => "block",
    }
}

/// Exercise the real MCP and CLI decision mappers, so a serializer regression is caught
/// by qualification rather than by a parroted expectation field (#344 review finding 2).
fn interface_labels(decision: &GuardDecision, observation: &GuardObservation) -> (String, String) {
    let actual_cli = match (
        crate::engine::jev::guard_cli::exit_code_for(decision),
        decision,
    ) {
        (0, GuardDecision::Allow { .. }) => "allow",
        (0, GuardDecision::Annotate { .. }) => "annotate",
        _ => "block",
    }
    .to_string();
    let mcp_result = crate::tools::guard::decision_to_result(
        decision.clone(),
        crate::engine::features::TokenCaps::default(),
        crate::tools::guard::GuardAccounting::default(),
        observation.clone(),
    );
    let actual_mcp = match &mcp_result {
        Ok(result) => result
            .content
            .first()
            .and_then(|block| block.as_text())
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text.text).ok())
            .and_then(|value| value["decision"].as_str().map(str::to_string))
            .unwrap_or_else(|| "unparseable".into()),
        Err(_) => "block".into(),
    };
    (actual_mcp, actual_cli)
}
fn deterministic_name(decision: &GuardDecision) -> &str {
    match decision {
        GuardDecision::Block(packet)
            if matches!(packet.violated_check.as_str(), "protected-path" | "secret") =>
        {
            packet.violated_check.as_str()
        }
        _ => "allow",
    }
}

fn signal(expected: bool, score: f64, threshold: f64) -> super::SignalDiagnostic {
    super::SignalDiagnostic {
        expected,
        score,
        threshold,
        actual: score >= threshold,
    }
}

/// Exercise the real production guard against disposable repository states, never replacing
/// proposed content with the disk copy. No fixture contents are written to the report.
pub async fn run_guard_qualification<C: JevClient>(
    client: &C,
    fixtures: &GuardFixtureSet,
    config: &JevConfig,
    protected_budget: Option<f64>,
    provenance: GuardProvenance,
    client_kind: &str,
    transport_attempts: impl Fn() -> Option<u64>,
) -> Result<GuardQualificationReport, JevError> {
    if protected_budget.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(JevError::Config {
            path: "JEV_BENCH_MAX_ESTIMATED_USD".into(),
            detail: "reviewed ceiling must be positive".into(),
        });
    }
    if config.rigor_failure_boundary != 0.70
        || config.secret_boundary != 0.25
        || config.complexity_boundary != 62.5
        || config.drift_boundary != 0.50
    {
        return Err(JevError::Config {
            path: "guard-qualification".into(),
            detail: "qualification requires frozen production finding boundaries".into(),
        });
    }
    // Materialize all cases before the first provider request; missing proposed paths remain absent.
    let roots: Vec<_> = fixtures
        .cases
        .iter()
        .map(prepare_case)
        .collect::<Result<_, _>>()?;
    let deterministic_roots: Vec<_> = fixtures
        .deterministic_cases
        .iter()
        .map(prepare_case)
        .collect::<Result<_, _>>()?;
    let price = JevPrice {
        input_per_million: provenance.pricing.input_per_million,
        output_per_million: provenance.pricing.output_per_million,
    };
    let recorder = Recorder::new(client, &transport_attempts, protected_budget, price);
    let mut deterministic_cases = Vec::with_capacity(fixtures.deterministic_cases.len());
    for (index, (case, root)) in fixtures
        .deterministic_cases
        .iter()
        .zip(&deterministic_roots)
        .enumerate()
    {
        let start = Instant::now();
        let (decision, observation) = evaluate_guard_observed(
            &case.proposal,
            &case.protected_paths,
            config,
            true,
            true,
            Some(&recorder),
            root.path(),
        )
        .await?;
        if recorder.totals().calls != 0
            || transport_attempts().is_some_and(|count| count != 0)
            || !source_unchanged(case, root.path())
        {
            return Err(invalid(
                Path::new("guard-qualification"),
                "deterministic probe called provider",
            ));
        }
        let actual_deterministic = deterministic_name(&decision).to_string();
        let (actual_mcp, actual_cli) = interface_labels(&decision, &observation);
        deterministic_cases.push(GuardCaseResult {
            case_index: index + 1,
            case_id_sha256: crate::engine::digest::sha256(case.case_id.as_bytes()),
            focus: case.focus.clone(),
            expected_deterministic: case.expected.deterministic.clone(),
            actual_deterministic,
            expected_decision: case.expected.decision.clone(),
            actual_decision: decision_name(&decision).into(),
            expected_mcp: case.expected.mcp.clone(),
            actual_mcp,
            expected_cli: case.expected.cli.clone(),
            actual_cli,
            findings: BTreeMap::new(),
            unavailable: None,
            unavailable_context: Vec::new(),
            elapsed_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            logical_calls: 0,
            responses: 0,
            transport_attempts: transport_attempts(),
            unknown_usage_calls: 0,
            estimated_cost_usd: Some(0.0),
            source_unchanged: true,
        });
    }
    let mut cases = Vec::with_capacity(fixtures.cases.len());
    let mut signals: BTreeMap<String, SignalMetrics> = BTreeMap::new();
    let mut unknown = 0_u64;
    let mut max_cost: Option<f64> = None;
    let mut interrupted = false;
    for (index, (case, root)) in fixtures.cases.iter().zip(&roots).enumerate() {
        let before = recorder.totals();
        let attempts_before = transport_attempts();
        if before.calls != before.responses
            || attempts_before.is_some_and(|count| count != before.calls)
            || protected_budget.is_some_and(|budget| {
                cost_estimate(before.input, before.output, Some(price))
                    .is_none_or(|cost| cost.amount >= budget)
            })
        {
            interrupted = true;
            break;
        }
        let start = Instant::now();
        let (decision, observation) = evaluate_guard_observed(
            &case.proposal,
            &case.protected_paths,
            config,
            true,
            true,
            Some(&recorder),
            root.path(),
        )
        .await?;
        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let after = recorder.totals();
        let attempts_after = transport_attempts();
        let attempts = attempts_after
            .zip(attempts_before)
            .map(|(after, before)| after.saturating_sub(before));
        let calls = after.calls.saturating_sub(before.calls);
        let responses = after.responses.saturating_sub(before.responses);
        let unknown_for_case = attempts.unwrap_or(calls).saturating_sub(responses);
        unknown += unknown_for_case;
        let cost = if unknown_for_case == 0 {
            cost_estimate(
                after.input.saturating_sub(before.input),
                after.output.saturating_sub(before.output),
                Some(price),
            )
            .map(|v| v.amount)
        } else {
            None
        };
        if let Some(cost) = cost {
            max_cost = Some(max_cost.map_or(cost, |max| max.max(cost)));
        }
        let (actual_mcp, actual_cli) = interface_labels(&decision, &observation);
        let mut findings = BTreeMap::new();
        if let Some(rigor) = observation.rigor {
            let expected = case
                .expected
                .rigor
                .as_ref()
                .ok_or_else(|| invalid(root.path(), "missing rigor labels"))?;
            for (name, result) in [
                (
                    "hallucinated_import",
                    signal(
                        expected.hallucinated_import,
                        rigor.hallucinated_import,
                        config.rigor_failure_boundary,
                    ),
                ),
                (
                    "violates_conventions",
                    signal(
                        expected.violates_conventions,
                        rigor.violates_conventions,
                        config.rigor_failure_boundary,
                    ),
                ),
                (
                    "complexity_high",
                    signal(
                        expected.complexity_high,
                        rigor.complexity / 100.0,
                        config.complexity_boundary / 100.0,
                    ),
                ),
                (
                    "contains_secrets",
                    signal(
                        expected.contains_secrets,
                        rigor.contains_secrets,
                        config.secret_boundary,
                    ),
                ),
            ] {
                signals
                    .entry(name.into())
                    .or_default()
                    .record(result.expected, result.actual);
                findings.insert(name, result);
            }
        }
        if let Some(drift) = observation.drift {
            let result = signal(
                case.expected
                    .drift_out_of_scope
                    .ok_or_else(|| invalid(root.path(), "missing scope label"))?,
                drift.noul,
                config.drift_boundary,
            );
            signals
                .entry("out_of_scope".into())
                .or_default()
                .record(result.expected, result.actual);
            findings.insert("out_of_scope", result);
        }
        let actual_deterministic = deterministic_name(&decision).to_string();
        cases.push(GuardCaseResult {
            case_index: index + 1,
            case_id_sha256: crate::engine::digest::sha256(case.case_id.as_bytes()),
            focus: case.focus.clone(),
            expected_deterministic: case.expected.deterministic.clone(),
            actual_deterministic,
            expected_decision: case.expected.decision.clone(),
            actual_decision: decision_name(&decision).into(),
            expected_mcp: case.expected.mcp.clone(),
            actual_mcp,
            expected_cli: case.expected.cli.clone(),
            actual_cli,
            findings,
            unavailable: observation
                .unavailable
                .map(|_| "evaluation_unavailable".into()),
            unavailable_context: observation.unavailable_context,
            elapsed_ms,
            logical_calls: calls,
            responses,
            transport_attempts: attempts,
            unknown_usage_calls: unknown_for_case,
            estimated_cost_usd: cost,
            source_unchanged: source_unchanged(case, root.path()),
        });
        if unknown != 0
            || attempts_after.is_some_and(|count| count != after.calls)
            || cases
                .last()
                .is_some_and(|case| !case.source_unchanged || case.unavailable.is_some())
        {
            interrupted = true;
            break;
        }
    }
    let totals = recorder.totals();
    let all_cost = if unknown == 0 {
        cost_estimate(totals.input, totals.output, Some(price)).map(|value| value.amount)
    } else {
        None
    };
    let mut times: Vec<_> = cases
        .iter()
        .filter(|case| case.logical_calls != 0)
        .map(|case| case.elapsed_ms)
        .collect();
    times.sort_unstable();
    let p95 = (!times.is_empty()).then(|| times[(times.len() * 95).div_ceil(100) - 1]);
    let mut provenance = provenance;
    provenance.observed_models = recorder.models();
    Ok(GuardQualificationReport {
        schema_version: 1,
        provenance,
        client_kind: client_kind.into(),
        deterministic_cases,
        cases,
        signals,
        whole_guard_p95_latency_ms: p95,
        logical_attempts: totals.calls,
        transport_attempts: transport_attempts(),
        unknown_usage_calls: unknown,
        total_estimated_cost_usd: all_cost,
        maximum_case_cost_usd: if unknown == 0 { max_cost } else { None },
        run_estimated_budget_usd: protected_budget,
        budget_interrupted: interrupted,
        accepted: false,
        failures: Vec::new(),
    })
}
