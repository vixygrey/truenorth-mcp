//! Run production rigor and drift evaluators over anonymized, versioned cases.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use super::super::config::JevConfig;
use super::super::config::JevPrice;
use super::super::secret_filter::{build_drift_scope_state, build_state_with_secret_filter};
use super::super::{JevClient, JevError, JevRequest, JevResponse};
use super::{
    AspectMetrics, CaseDiagnostic, Fixture, FixtureSet, Pricing, SignalDiagnostic, SignalMetrics,
    agreement, cost_estimate,
};

#[derive(Default, Clone, Copy)]
struct UsageTotals {
    input: u64,
    output: u64,
    responses: usize,
}

struct UsageRecorder<'a, C: JevClient> {
    inner: &'a C,
    totals: Mutex<UsageTotals>,
    models: Mutex<BTreeSet<String>>,
}

impl<'a, C: JevClient> UsageRecorder<'a, C> {
    fn new(inner: &'a C) -> Self {
        Self {
            inner,
            totals: Mutex::new(UsageTotals::default()),
            models: Mutex::new(BTreeSet::new()),
        }
    }
    fn totals(&self) -> UsageTotals {
        *self.totals.lock().unwrap_or_else(|p| p.into_inner())
    }
    fn models(&self) -> Vec<String> {
        self.models
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}

impl<C: JevClient> JevClient for UsageRecorder<'_, C> {
    async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError> {
        let response = self.inner.evaluate(request).await?;
        let mut totals = self.totals.lock().unwrap_or_else(|p| p.into_inner());
        totals.input = totals.input.saturating_add(response.usage.input_tokens);
        totals.output = totals.output.saturating_add(response.usage.output_tokens);
        totals.responses += 1;
        drop(totals);
        let model = if !response.model.is_empty()
            && response.model.len() <= 64
            && response
                .model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            response.model.clone()
        } else {
            "<invalid responder identity>".into()
        };
        self.models
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(model);
        Ok(response)
    }
}

struct AspectCollector {
    metric: AspectMetrics,
    case_costs: Vec<f64>,
}

impl AspectCollector {
    fn new(aspect: &str) -> Self {
        Self {
            metric: AspectMetrics {
                aspect: aspect.into(),
                attempts: 0,
                successes: 0,
                errors: 0,
                responses: 0,
                matches: 0,
                agreement: 0.0,
                latency_ms: Vec::new(),
                p95_latency_ms: 0,
                signals: BTreeMap::new(),
                input_tokens: 0,
                output_tokens: 0,
                unknown_usage_calls: 0,
                estimated_cost: None,
                maximum_case_cost: None,
            },
            case_costs: Vec::new(),
        }
    }
    fn record_call(
        &mut self,
        elapsed: u64,
        before: UsageTotals,
        after: UsageTotals,
        ok: bool,
        price: JevPrice,
    ) {
        self.metric.attempts += 1;
        self.metric.latency_ms.push(elapsed);
        if ok {
            self.metric.successes += 1;
        } else {
            self.metric.errors += 1;
        }
        let input = after.input.saturating_sub(before.input);
        let output = after.output.saturating_sub(before.output);
        self.metric.input_tokens = self.metric.input_tokens.saturating_add(input);
        self.metric.output_tokens = self.metric.output_tokens.saturating_add(output);
        // A failed call with no response has unknown billable usage, never a zero-cost call.
        self.metric.responses += after.responses.saturating_sub(before.responses);
        if !ok && after.responses == before.responses {
            self.metric.unknown_usage_calls += 1;
        }
        if let Some(cost) = cost_estimate(input, output, Some(price)) {
            self.case_costs.push(cost.amount);
        }
    }
    fn signal(&mut self, name: &str, expected: bool, actual: bool) {
        let signal: &mut SignalMetrics = self.metric.signals.entry(name.into()).or_default();
        signal.record(expected, actual);
    }
    fn finish(mut self, price: JevPrice) -> AspectMetrics {
        let mut ordered = self.metric.latency_ms.clone();
        ordered.sort_unstable();
        if !ordered.is_empty() {
            self.metric.p95_latency_ms = ordered[(ordered.len() * 95).div_ceil(100) - 1];
        }
        self.metric.matches = self.metric.signals.values().map(|s| s.matches).sum();
        let scored = self.metric.signals.values().map(|s| s.samples).sum();
        self.metric.agreement = agreement(self.metric.matches, scored);
        if self.metric.unknown_usage_calls == 0 {
            self.metric.estimated_cost = cost_estimate(
                self.metric.input_tokens,
                self.metric.output_tokens,
                Some(price),
            );
            self.metric.maximum_case_cost = self.case_costs.into_iter().reduce(f64::max);
        }
        self.metric
    }
}

fn benchmark_error() -> JevError {
    JevError::InvalidFixtureSet {
        path: "benchmarks/jev/fixtures-v1.yml".into(),
        detail: "could not build a safe anonymized production state".into(),
    }
}

pub(super) fn rigor_state(case: &Fixture) -> Result<serde_json::Value, JevError> {
    // Use the same protected file reader and secret filter as evaluate_guard. Never write
    // fixture contents into telemetry or include them in benchmark errors/reports.
    let dir = tempfile::tempdir().map_err(|_| benchmark_error())?;
    let mut paths = Vec::with_capacity(case.files.len());
    for file in &case.files {
        let path = PathBuf::from(&file.path);
        if crate::engine::agent_ws::fs_boundary::RepositoryFs::validate_path(&path).is_err()
            || crate::config::is_secret_path(&path)
        {
            return Err(benchmark_error());
        }
        let target = dir.path().join(&path);
        std::fs::create_dir_all(target.parent().ok_or_else(benchmark_error)?)
            .map_err(|_| benchmark_error())?;
        std::fs::write(target, &file.content).map_err(|_| benchmark_error())?;
        paths.push(path);
    }
    build_state_with_secret_filter(dir.path(), &paths).map_err(|_| benchmark_error())
}

pub struct BenchmarkMeasurements {
    pub metrics: Vec<AspectMetrics>,
    pub models: Vec<String>,
    pub maximum_fixture_cost: Option<f64>,
    pub budget_interrupted: bool,
    pub case_diagnostics: Vec<CaseDiagnostic>,
}

/// The returned model identities come from actual response bodies, never from requested aliases.
/// Each evaluator is the same one used by the guarded production path.
pub async fn run_benchmark<C: JevClient>(
    client: &C,
    fixtures: &FixtureSet,
    config: &JevConfig,
    pricing: &Pricing,
) -> Result<BenchmarkMeasurements, JevError> {
    run_benchmark_with_budget(client, fixtures, config, pricing, None).await
}

/// A reviewed estimated-cost ceiling stops further calls; provider spend caps separately
/// bound billing because transport retries and failed calls can have unobservable usage.
pub async fn run_benchmark_with_budget<C: JevClient>(
    client: &C,
    fixtures: &FixtureSet,
    config: &JevConfig,
    pricing: &Pricing,
    estimated_budget_usd: Option<f64>,
) -> Result<BenchmarkMeasurements, JevError> {
    if estimated_budget_usd.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(JevError::Config {
            path: "JEV_BENCH_MAX_ESTIMATED_USD".into(),
            detail: "the reviewed estimated ceiling must be finite and positive".into(),
        });
    }
    // Prepare every state before the first potentially billable call. A malformed later
    // case cannot leave an unreported partially executed provider run.
    let states: Vec<_> = fixtures
        .cases
        .iter()
        .map(|case| {
            let rigor = case
                .expected
                .rigor
                .as_ref()
                .map(|_| rigor_state(case))
                .transpose()?;
            let drift = case
                .drift
                .as_ref()
                .map(|scope| {
                    let paths: Vec<PathBuf> =
                        scope.changed_paths.iter().map(PathBuf::from).collect();
                    build_drift_scope_state(&paths, &scope.protected_paths, Some(&scope.task))
                        .map_err(|_| benchmark_error())
                })
                .transpose()?;
            Ok::<_, JevError>((rigor, drift))
        })
        .collect::<Result<_, _>>()?;
    let price = JevPrice {
        input_per_million: pricing.input_per_million,
        output_per_million: pricing.output_per_million,
    };
    let recorder = UsageRecorder::new(client);
    let mut rigor = AspectCollector::new("rigor");
    let mut drift = AspectCollector::new("drift");
    let mut routing = AspectCollector::new("routing");
    let mut self_heal = AspectCollector::new("self_heal");
    let mut maximum_fixture_cost: Option<f64> = None;
    let mut budget_interrupted = false;
    let mut partial_case_start = None;
    let mut case_diagnostics = Vec::new();

    'cases: for (index, (case, (rigor_state, drift_state))) in
        fixtures.cases.iter().zip(states).enumerate()
    {
        let case_start = recorder.totals();
        partial_case_start = Some(case_start);
        if let (Some(expected), Some(state)) = (&case.expected.rigor, rigor_state) {
            if budget_reached(
                estimated_budget_usd,
                &recorder,
                [&rigor, &drift, &routing, &self_heal],
                price,
            ) {
                budget_interrupted = true;
                break 'cases;
            }
            let before = recorder.totals();
            let started = Instant::now();
            let outcome = super::super::rigor::evaluate_rigor(&recorder, state).await;
            let elapsed = elapsed(started);
            let mut diagnostic = CaseDiagnostic {
                case_index: index + 1,
                case_id_sha256: crate::engine::digest::sha256(case.case_id.as_bytes()),
                aspect: "rigor",
                signals: BTreeMap::new(),
                error: None,
            };
            if let Ok(result) = outcome {
                let scores = [
                    (
                        "hallucinated_import",
                        expected.hallucinated_import,
                        result.hallucinated_import,
                        config.rigor_failure_boundary,
                        result.hallucinated_import >= config.rigor_failure_boundary,
                    ),
                    (
                        "violates_conventions",
                        expected.violates_conventions,
                        result.violates_conventions,
                        config.rigor_failure_boundary,
                        result.violates_conventions >= config.rigor_failure_boundary,
                    ),
                    (
                        "complexity_high",
                        expected.complexity_high,
                        result.complexity / 100.0,
                        config.complexity_boundary / 100.0,
                        result.complexity >= config.complexity_boundary,
                    ),
                    (
                        "contains_secrets",
                        expected.contains_secrets,
                        result.contains_secrets,
                        config.secret_boundary,
                        result.contains_secrets >= config.secret_boundary,
                    ),
                ];
                if scores.iter().all(|(_, _, score, threshold, _)| {
                    super::unit(*score) && super::unit(*threshold)
                }) {
                    for (name, expected, score, threshold, actual) in scores {
                        rigor.signal(name, expected, actual);
                        diagnostic.signals.insert(
                            name,
                            SignalDiagnostic {
                                expected,
                                score,
                                threshold,
                                actual,
                            },
                        );
                    }
                } else {
                    diagnostic.error = Some("evaluation_failed");
                }
            } else {
                diagnostic.error = Some("evaluation_failed");
            }
            rigor.record_call(
                elapsed,
                before,
                recorder.totals(),
                diagnostic.error.is_none(),
                price,
            );
            case_diagnostics.push(diagnostic);
        }
        if let (Some(scope), Some(expected), Some(state)) =
            (&case.drift, case.expected.drift_out_of_scope, drift_state)
        {
            if budget_reached(
                estimated_budget_usd,
                &recorder,
                [&rigor, &drift, &routing, &self_heal],
                price,
            ) {
                budget_interrupted = true;
                break 'cases;
            }
            let before = recorder.totals();
            let started = Instant::now();
            let outcome = super::super::drift::evaluate_drift(
                &recorder,
                state,
                &scope.changed_paths,
                &scope.protected_paths,
                config.drift_boundary,
            )
            .await;
            let elapsed = elapsed(started);
            let mut diagnostic = CaseDiagnostic {
                case_index: index + 1,
                case_id_sha256: crate::engine::digest::sha256(case.case_id.as_bytes()),
                aspect: "drift",
                signals: BTreeMap::new(),
                error: None,
            };
            if let Ok(result) = outcome {
                if super::unit(result.noul) && super::unit(config.drift_boundary) {
                    drift.signal("out_of_scope", expected, result.out_of_scope);
                    diagnostic.signals.insert(
                        "out_of_scope",
                        SignalDiagnostic {
                            expected,
                            score: result.noul,
                            threshold: config.drift_boundary,
                            actual: result.out_of_scope,
                        },
                    );
                } else {
                    diagnostic.error = Some("evaluation_failed");
                }
            } else {
                diagnostic.error = Some("evaluation_failed");
            }
            drift.record_call(
                elapsed,
                before,
                recorder.totals(),
                diagnostic.error.is_none(),
                price,
            );
            case_diagnostics.push(diagnostic);
        }
        if let Some(expected) = &case.expected.routing {
            if budget_reached(
                estimated_budget_usd,
                &recorder,
                [&rigor, &drift, &routing, &self_heal],
                price,
            ) {
                budget_interrupted = true;
                break 'cases;
            }
            let before = recorder.totals();
            let started = Instant::now();
            let outcome = super::super::routing::evaluate_routing(
                &recorder,
                case.state.clone().ok_or_else(benchmark_error)?,
                config.confidence_low,
                config.confidence_high,
            )
            .await;
            routing.record_call(
                elapsed(started),
                before,
                recorder.totals(),
                outcome.is_ok(),
                price,
            );
            if let Ok(result) = outcome {
                routing.signal("target_match", true, result.target == expected.as_str());
            }
        }
        if let Some(expected) = &case.expected.self_heal {
            if budget_reached(
                estimated_budget_usd,
                &recorder,
                [&rigor, &drift, &routing, &self_heal],
                price,
            ) {
                budget_interrupted = true;
                break 'cases;
            }
            let before = recorder.totals();
            let started = Instant::now();
            let outcome = super::super::self_heal::evaluate_self_heal(
                &recorder,
                case.state.clone().ok_or_else(benchmark_error)?,
                config.confidence_low,
                config.destructive_threshold,
            )
            .await;
            self_heal.record_call(
                elapsed(started),
                before,
                recorder.totals(),
                outcome.is_ok(),
                price,
            );
            if let Ok(result) = outcome {
                self_heal.signal(
                    "instruction_match",
                    true,
                    super::super::self_heal::option_str(result.instruction) == expected.as_str(),
                );
            }
        }
        let case_end = recorder.totals();
        if let Some(cost) = cost_estimate(
            case_end.input.saturating_sub(case_start.input),
            case_end.output.saturating_sub(case_start.output),
            Some(price),
        ) {
            maximum_fixture_cost =
                Some(maximum_fixture_cost.map_or(cost.amount, |current| current.max(cost.amount)));
        }
        partial_case_start = None;
    }
    if let Some(case_start) = partial_case_start {
        let case_end = recorder.totals();
        if let Some(cost) = cost_estimate(
            case_end.input.saturating_sub(case_start.input),
            case_end.output.saturating_sub(case_start.output),
            Some(price),
        ) {
            maximum_fixture_cost =
                Some(maximum_fixture_cost.map_or(cost.amount, |current| current.max(cost.amount)));
        }
    }
    let metrics: Vec<AspectMetrics> = [rigor, drift, routing, self_heal]
        .into_iter()
        .filter(|c| c.metric.attempts != 0)
        .map(|collector| collector.finish(price))
        .collect();
    if metrics.iter().any(|metric| metric.unknown_usage_calls != 0) {
        maximum_fixture_cost = None;
    }
    Ok(BenchmarkMeasurements {
        metrics,
        case_diagnostics,
        models: recorder.models(),
        maximum_fixture_cost,
        budget_interrupted,
    })
}

fn budget_reached<C: JevClient>(
    budget: Option<f64>,
    recorder: &UsageRecorder<'_, C>,
    collectors: [&AspectCollector; 4],
    price: JevPrice,
) -> bool {
    let Some(budget) = budget else { return false };
    if collectors
        .iter()
        .any(|collector| collector.metric.unknown_usage_calls != 0)
    {
        return true;
    }
    let totals = recorder.totals();
    cost_estimate(totals.input, totals.output, Some(price))
        .is_none_or(|cost| !cost.amount.is_finite() || cost.amount >= budget)
}

fn elapsed(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
