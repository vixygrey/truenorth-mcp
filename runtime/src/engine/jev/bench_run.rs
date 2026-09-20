//! The Jev benchmark async runner: the usage-recording client decorator and the per-aspect
//! evaluation loops.
//!
//! Included from `bench.rs` via `#[path]`, so `super` is the `bench` module and
//! `super::super` is the `jev` module. The runner covers the three aspects a fixture can
//! label: routing, drift, and self-heal. Each aspect module consumes the response usage
//! internally, so the runner routes every aspect call through a [`UsageRecorder`] decorator.
//! The decorator satisfies [`JevClient`], delegates to the chosen client, and tallies the
//! input and output tokens as each response passes through. This keeps the input and output
//! counts separate (R12.1) and adds a zero output contribution for a zero output rate
//! (R12.2), and it changes no aspect signature.
//!
//! Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 12.1, 12.2. Design: jev-integration-eval, the
//! benchmark runner.

use std::sync::Mutex;
use std::time::Instant;

use super::super::config::{JevConfig, JevPrice};
use super::super::{JevClient, JevError, JevRequest, JevResponse};
use super::{AspectMetrics, Fixture, agreement, cost_estimate};

/// A usage-recording [`JevClient`] decorator over an inner client.
///
/// Each aspect module consumes the response usage internally, so the runner wraps the chosen
/// client in this decorator. Every call delegates to the inner client and tallies the input
/// and output tokens from a successful response, so the runner reads the summed usage after
/// an aspect runs. The decorator makes no call of its own, so the fake stays silent when the
/// runner passes it through (R11.7).
struct UsageRecorder<'a, C: JevClient> {
    /// The wrapped client the calls delegate to.
    inner: &'a C,
    /// The running token tally, guarded for the shared reference the aspect holds.
    totals: Mutex<UsageTotals>,
}

/// A running token tally, kept separate for input and output (R12.1).
#[derive(Debug, Clone, Copy, Default)]
struct UsageTotals {
    /// The summed input tokens.
    input_tokens: u64,
    /// The summed output tokens.
    output_tokens: u64,
}

impl<'a, C: JevClient> UsageRecorder<'a, C> {
    /// Wrap `inner` with a zeroed tally.
    fn new(inner: &'a C) -> Self {
        Self {
            inner,
            totals: Mutex::new(UsageTotals::default()),
        }
    }

    /// Read the current tally, recovering a poisoned lock rather than panicking.
    fn totals(&self) -> UsageTotals {
        *self
            .totals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<C: JevClient> JevClient for UsageRecorder<'_, C> {
    async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError> {
        let response = self.inner.evaluate(request).await?;
        let mut totals = self
            .totals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        totals.input_tokens = totals
            .input_tokens
            .saturating_add(response.usage.input_tokens);
        totals.output_tokens = totals
            .output_tokens
            .saturating_add(response.usage.output_tokens);
        drop(totals);
        Ok(response)
    }
}

/// Run the benchmark across the fixtures and return one [`AspectMetrics`] per labeled aspect.
///
/// The runner covers the three aspects a fixture can label: routing, drift, and self-heal.
/// For each aspect it evaluates only the cases whose expected answers carry that aspect, so
/// an aspect with no labeled case produces no metrics. For each evaluated case it measures
/// the call latency, records the aspect confidence where one applies, tallies the response
/// usage through the [`UsageRecorder`], and counts an agreement match against the expected
/// answer. It then computes the cost from the summed tokens and the config price.
///
/// A case whose aspect call returns an error is not counted as a match and contributes no
/// confidence, but its latency is still recorded, so a failed call is visible in the report.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn run_benchmark<C: JevClient>(
    client: &C,
    fixtures: &[Fixture],
    config: &JevConfig,
) -> Vec<AspectMetrics> {
    let mut metrics = Vec::new();

    if let Some(m) = run_routing(client, fixtures, config).await {
        metrics.push(m);
    }
    if let Some(m) = run_drift(client, fixtures, config).await {
        metrics.push(m);
    }
    if let Some(m) = run_self_heal(client, fixtures, config).await {
        metrics.push(m);
    }

    metrics
}

/// Evaluate the routing aspect across the labeled cases, or `None` when none apply.
async fn run_routing<C: JevClient>(
    client: &C,
    fixtures: &[Fixture],
    config: &JevConfig,
) -> Option<AspectMetrics> {
    let recorder = UsageRecorder::new(client);
    let mut latency_ms = Vec::new();
    let mut confidence = Vec::new();
    let mut matches = 0usize;
    let mut total = 0usize;

    for fixture in fixtures {
        let Some(expected) = fixture.expected.routing.as_deref() else {
            continue;
        };
        total += 1;
        let started = Instant::now();
        let outcome = super::super::routing::evaluate_routing(
            &recorder,
            fixture.state.clone(),
            config.confidence_low,
            config.confidence_high,
        )
        .await;
        latency_ms.push(elapsed_ms(started));
        if let Ok(outcome) = outcome {
            confidence.push(outcome.confidence);
            if outcome.target == expected {
                matches += 1;
            }
        }
    }

    if total == 0 {
        return None;
    }
    Some(build_metrics(
        "routing",
        latency_ms,
        confidence,
        matches,
        total,
        recorder.totals(),
        config.price,
    ))
}

/// Evaluate the drift aspect across the labeled cases, or `None` when none apply.
///
/// Drift returns a noul value, not a confidence, so the confidence vector stays empty. The
/// runner passes no written or protected paths from a fixture state, so the model-free layer
/// does not force a decision and the aspect asks the model.
async fn run_drift<C: JevClient>(
    client: &C,
    fixtures: &[Fixture],
    config: &JevConfig,
) -> Option<AspectMetrics> {
    let recorder = UsageRecorder::new(client);
    let mut latency_ms = Vec::new();
    let mut matches = 0usize;
    let mut total = 0usize;

    for fixture in fixtures {
        let Some(expected) = fixture.expected.drift_out_of_scope else {
            continue;
        };
        total += 1;
        let started = Instant::now();
        let outcome = super::super::drift::evaluate_drift(
            &recorder,
            fixture.state.clone(),
            &[],
            &[],
            config.drift_boundary,
        )
        .await;
        latency_ms.push(elapsed_ms(started));
        if let Ok(outcome) = outcome
            && outcome.out_of_scope == expected
        {
            matches += 1;
        }
    }

    if total == 0 {
        return None;
    }
    Some(build_metrics(
        "drift",
        latency_ms,
        Vec::new(),
        matches,
        total,
        recorder.totals(),
        config.price,
    ))
}

/// Evaluate the self-heal aspect across the labeled cases, or `None` when none apply.
async fn run_self_heal<C: JevClient>(
    client: &C,
    fixtures: &[Fixture],
    config: &JevConfig,
) -> Option<AspectMetrics> {
    let recorder = UsageRecorder::new(client);
    let mut latency_ms = Vec::new();
    let mut confidence = Vec::new();
    let mut matches = 0usize;
    let mut total = 0usize;

    for fixture in fixtures {
        let Some(expected) = fixture.expected.self_heal.as_deref() else {
            continue;
        };
        total += 1;
        let started = Instant::now();
        let outcome = super::super::self_heal::evaluate_self_heal(
            &recorder,
            fixture.state.clone(),
            config.confidence_low,
            config.destructive_threshold,
        )
        .await;
        latency_ms.push(elapsed_ms(started));
        if let Ok(decision) = outcome {
            confidence.push(decision.confidence);
            if super::super::self_heal::option_str(decision.instruction) == expected {
                matches += 1;
            }
        }
    }

    if total == 0 {
        return None;
    }
    Some(build_metrics(
        "self_heal",
        latency_ms,
        confidence,
        matches,
        total,
        recorder.totals(),
        config.price,
    ))
}

/// Build one [`AspectMetrics`] from the collected per-case data and the summed usage.
#[allow(clippy::too_many_arguments)] // Each argument is one distinct metric field, no clearer as a struct here.
fn build_metrics(
    aspect: &str,
    latency_ms: Vec<u64>,
    confidence: Vec<f64>,
    matches: usize,
    total: usize,
    totals: UsageTotals,
    price: Option<JevPrice>,
) -> AspectMetrics {
    AspectMetrics {
        aspect: aspect.to_string(),
        latency_ms,
        confidence,
        agreement: agreement(matches, total),
        input_tokens: totals.input_tokens,
        output_tokens: totals.output_tokens,
        estimated_cost: cost_estimate(totals.input_tokens, totals.output_tokens, price),
    }
}

/// The elapsed milliseconds since `started`, saturating at [`u64::MAX`].
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
