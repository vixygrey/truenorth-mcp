//! The Jev benchmark data layer: fixtures, per-aspect metrics, agreement, cost, and the report.
//!
//! This module holds the benchmark data types and the pure accounting: [`Fixture`] and
//! [`ExpectedAnswers`] on the input side, [`AspectMetrics`] and [`CostEstimate`] on the
//! output side, and the pure functions [`load_fixtures`], [`agreement`], [`cost_estimate`],
//! and [`write_report`]. The async runner lives in the sibling `bench_run.rs`, included as a
//! child module, so this file stays a focused data layer under the size guidance.
//!
//! The report carries only the aspect name, the latency and confidence vectors, the
//! agreement fraction, the token counts, and an optional cost. It carries no API key and no
//! repository content, so it leaks no secret by construction (R11.6).
//!
//! The runner has no non-test caller under the default build: its only caller is the
//! `jev-bench` bin, which builds behind the `jev-http` feature (task 12.2). Under the
//! default build every public item here has only test callers, so each carries a narrow
//! non-test `allow` with this reason, per the repo dead-code policy (main.rs). The test
//! build exercises every item through the sibling `bench_tests.rs`.
//!
//! Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.8, 11.9, 12.1, 12.2, 12.6, 12.7,
//! 12.8. Design: jev-integration-eval, the benchmark runner.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::JevError;
use super::config::JevPrice;

// The async runner (the client decorator, `run_benchmark`, and the per-aspect helpers) lives
// in a sibling file to hold this module under the size guidance. The `#[path]` include keeps
// it part of the `bench` module, so `run_benchmark` reads as `bench::run_benchmark`.
#[path = "bench_run.rs"]
mod run;

#[cfg_attr(not(test), allow(unused_imports))]
pub use run::run_benchmark;

/// The report path under `.agent/`, relative to the agent root for the write guard.
const REPORT_REL_PATH: &str = "telemetry/jev-benchmark.yml";

/// The tokens-per-million divisor for the cost estimate (R12.7).
const TOKENS_PER_MILLION: f64 = 1_000_000.0;

/// The measured metrics for one aspect over the fixture set (R11.3 to R11.5, R12.1).
///
/// The `latency_ms` and `confidence` vectors carry one entry per evaluated case, in fixture
/// order. The `confidence` vector is empty for an aspect that returns no confidence, for
/// example drift. The `agreement` is the fraction of cases whose outcome matched the
/// expected answer. The token counts sum the response usage across the cases. The
/// `estimated_cost` is present only with a configured price (R12.6).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize)]
pub struct AspectMetrics {
    /// The aspect name, for example `routing`, `drift`, or `self_heal`.
    pub aspect: String,
    /// The per-case latency in milliseconds, in fixture order.
    pub latency_ms: Vec<u64>,
    /// The per-case confidence from 0 to 1, in fixture order. Empty when none applies.
    pub confidence: Vec<f64>,
    /// The fraction of cases whose outcome matched the expected answer, from 0 to 1.
    pub agreement: f64,
    /// The summed input token count across the cases.
    pub input_tokens: u64,
    /// The summed output token count across the cases.
    pub output_tokens: u64,
    /// The estimated cost, present only with a configured price (R12.6).
    pub estimated_cost: Option<CostEstimate>,
}

/// An estimated cost for one aspect, always labeled unverified (R12.7).
///
/// The `price_unverified` flag is always true, because the token count is an estimate and
/// the price is a configured rate, not a billed amount.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CostEstimate {
    /// The estimated cost amount in the configured currency.
    pub amount: f64,
    /// Always true: the amount is an estimate from a configured rate, not a billed cost.
    pub price_unverified: bool,
}

/// One labeled benchmark fixture: a repository case and its expected answers (R11.8).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Deserialize)]
pub struct Fixture {
    /// The case identifier, for report traceability.
    pub case_id: String,
    /// The evaluation input state, passed to each aspect.
    pub state: serde_json::Value,
    /// The expected answer per aspect. An absent aspect answer is not scored.
    pub expected: ExpectedAnswers,
}

/// The expected answers for one fixture case, per aspect (R11.4, R11.5).
///
/// An absent field means the case does not label that aspect, so the runner does not score
/// it. The `routing` value is the expected tool name or `NONE`. The `drift_out_of_scope`
/// value is the expected out-of-scope decision. The `self_heal` value is the expected
/// recovery instruction, for example `REVERT` or `ASK_HUMAN`.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExpectedAnswers {
    /// The expected routing target, a tool name or `NONE`.
    #[serde(default)]
    pub routing: Option<String>,
    /// The expected drift out-of-scope decision.
    #[serde(default)]
    pub drift_out_of_scope: Option<bool>,
    /// The expected self-heal instruction, for example `REVERT` or `ASK_HUMAN`.
    #[serde(default)]
    pub self_heal: Option<String>,
}

/// Load the labeled fixture set from a YAML file (R11.8, R11.9).
///
/// The file is a YAML sequence of [`Fixture`] entries. A missing file or an unparsable file
/// returns [`JevError::InvalidFixtureSet`] naming the path and the detail, so no aspect runs
/// and the caller retains any prior report (R11.9). A valid file returns the parsed vector.
///
/// # Errors
///
/// Returns [`JevError::InvalidFixtureSet`] when the file is missing or cannot be parsed.
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_fixtures(path: &Path) -> Result<Vec<Fixture>, JevError> {
    let text = std::fs::read_to_string(path).map_err(|error| JevError::InvalidFixtureSet {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_yaml::from_str::<Vec<Fixture>>(&text).map_err(|error| JevError::InvalidFixtureSet {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

/// The fraction of matching cases, or 0.0 when there is nothing to score (R11.5).
///
/// The result is `matches / total`, clamped by construction to 0 to 1 because a caller never
/// passes more matches than cases. A total of 0 returns 0.0 rather than divide by zero.
#[cfg_attr(not(test), allow(dead_code))]
pub fn agreement(matches: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    matches as f64 / total as f64
}

/// The estimated cost for a token count under an optional price (R12.6, R12.7).
///
/// A `None` price returns `None`, so the report carries token counts and no cost (R12.6). A
/// configured price returns `Some`, rating the input and output tokens separately by their
/// per-million rates and summing them (R12.1). A zero output rate contributes zero output
/// cost (R12.2). The estimate is always labeled unverified (R12.7).
#[cfg_attr(not(test), allow(dead_code))]
pub fn cost_estimate(
    input_tokens: u64,
    output_tokens: u64,
    price: Option<JevPrice>,
) -> Option<CostEstimate> {
    let price = price?;
    let input_cost = input_tokens as f64 / TOKENS_PER_MILLION * price.input_per_million;
    let output_cost = output_tokens as f64 / TOKENS_PER_MILLION * price.output_per_million;
    Some(CostEstimate {
        amount: input_cost + output_cost,
        price_unverified: true,
    })
}

/// Write the metrics report under `.agent/telemetry/` and return the relative path (R11.6).
///
/// The report serializes the metrics to YAML and writes to `.agent/telemetry/jev-benchmark.yml`
/// through [`crate::engine::agent_ws::write_under_agent`], the single write guard. The metrics
/// carry no API key and no repository content, so the report leaks no secret by construction
/// (R11.6). A serialization or write-guard failure maps to [`JevError::Config`] naming the
/// report path, so no partial report is claimed.
///
/// # Errors
///
/// Returns [`JevError::Config`] when the metrics cannot be serialized or the guarded write
/// fails.
#[cfg_attr(not(test), allow(dead_code))]
pub fn write_report(repo_root: &Path, metrics: &[AspectMetrics]) -> Result<String, JevError> {
    let yaml = serde_yaml::to_string(metrics).map_err(|error| JevError::Config {
        path: REPORT_REL_PATH.to_string(),
        detail: format!("could not serialize the benchmark report: {error}"),
    })?;

    crate::engine::agent_ws::write_under_agent(repo_root, Path::new(REPORT_REL_PATH), &yaml)
        .map_err(|error| JevError::Config {
            path: REPORT_REL_PATH.to_string(),
            detail: format!("could not write the benchmark report: {error}"),
        })?;

    Ok(REPORT_REL_PATH.to_string())
}

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `bench`.
#[cfg(test)]
#[path = "bench_tests.rs"]
mod tests;
