//! Full-guard proposal qualification; the historical evaluator benchmark is unchanged.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{AcceptancePolicy, Pricing, SignalMetrics, cost_estimate};
use crate::engine::jev::config::{JevConfig, JevPrice};
use crate::engine::jev::guard::evaluate::evaluate_guard_observed;
use crate::engine::jev::guard::{GuardDecision, ProposedChange, deterministic_layer};
use crate::engine::jev::{JevClient, JevError, JevRequest, JevResponse};

use super::{SignalDiagnostic, verify_source};

#[path = "bench_guard_qualification_fixtures.rs"]
mod fixtures;
#[path = "bench_guard_qualification_policy.rs"]
mod policy;
#[path = "bench_guard_qualification_recording.rs"]
mod recording;
#[path = "bench_guard_qualification_runner.rs"]
mod runner;

pub use fixtures::load_guard_fixtures;
use fixtures::{invalid, prepare_case, source_unchanged};
pub use policy::{evaluate_guard_acceptance, provenance, write_guard_report};
use recording::Recorder;
pub use runner::run_guard_qualification;

const REPORT_PATH: &str = "telemetry/jev-guard-qualification.yml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardFixtureSet {
    pub schema_version: u32,
    pub source: super::Source,
    pub cases: Vec<GuardCase>,
    #[serde(default)]
    pub deterministic_cases: Vec<GuardCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardCase {
    pub case_id: String,
    pub focus: String,
    pub files: Vec<super::FixtureFile>,
    pub proposal: ProposedChange,
    pub protected_paths: Vec<String>,
    pub expected: GuardExpected,
    #[serde(default)]
    pub cockpit_task: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardExpected {
    pub deterministic: String,
    pub rigor: Option<super::RigorLabels>,
    pub drift_out_of_scope: Option<bool>,
    pub decision: String,
    pub mcp: String,
    pub cli: String,
}

#[derive(Debug, Serialize)]
pub struct GuardCaseResult {
    pub case_index: usize,
    pub case_id_sha256: String,
    pub focus: String,
    pub expected_deterministic: String,
    pub actual_deterministic: String,
    pub expected_decision: String,
    pub actual_decision: String,
    pub expected_mcp: String,
    pub actual_mcp: String,
    pub expected_cli: String,
    pub actual_cli: String,
    pub findings: BTreeMap<&'static str, super::SignalDiagnostic>,
    pub unavailable: Option<String>,
    pub unavailable_context: Vec<String>,
    pub elapsed_ms: u64,
    pub logical_calls: u64,
    pub responses: u64,
    pub transport_attempts: Option<u64>,
    pub unknown_usage_calls: u64,
    pub estimated_cost_usd: Option<f64>,
    pub source_unchanged: bool,
}

#[derive(Debug, Serialize)]
pub struct GuardQualificationReport {
    pub schema_version: u32,
    pub provenance: GuardProvenance,
    pub client_kind: String,
    pub deterministic_cases: Vec<GuardCaseResult>,
    pub cases: Vec<GuardCaseResult>,
    pub signals: BTreeMap<String, SignalMetrics>,
    pub whole_guard_p95_latency_ms: Option<u64>,
    pub logical_attempts: u64,
    pub transport_attempts: Option<u64>,
    pub unknown_usage_calls: u64,
    pub total_estimated_cost_usd: Option<f64>,
    pub maximum_case_cost_usd: Option<f64>,
    pub run_estimated_budget_usd: Option<f64>,
    pub budget_interrupted: bool,
    pub accepted: bool,
    pub failures: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GuardProvenance {
    pub source_name: String,
    pub source_version: String,
    pub source_sha256: String,
    pub fixture_sha256: String,
    pub policy_sha256: String,
    pub code_sha: String,
    pub config_sha256: String,
    pub context_rules_sha256: String,
    pub rigor_prompt_sha256: String,
    pub drift_prompt_sha256: String,
    pub requested_model: String,
    pub concrete_model: String,
    pub observed_models: Vec<String>,
    pub pricing: Pricing,
    pub scoring: BTreeMap<String, f64>,
    pub transport: BTreeMap<String, u64>,
}

#[cfg(test)]
#[path = "bench_guard_qualification_tests.rs"]
mod tests;
