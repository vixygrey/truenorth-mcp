//! Versioned Jev benchmark inputs, accounting and production acceptance.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::JevError;
use super::config::JevPrice;

#[path = "bench_run.rs"]
mod run;
pub use run::{BenchmarkMeasurements, run_benchmark, run_benchmark_with_budget};

const REPORT_REL_PATH: &str = "telemetry/jev-benchmark.yml";
const TOKENS_PER_MILLION: f64 = 1_000_000.0;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSet {
    pub schema_version: u32,
    pub source: Source,
    pub cases: Vec<Fixture>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub name: String,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub case_id: String,
    #[serde(default)]
    pub files: Vec<FixtureFile>,
    #[serde(default)]
    pub drift: Option<DriftInput>,
    #[serde(default)]
    pub state: Option<serde_json::Value>,
    pub expected: ExpectedAnswers,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftInput {
    pub changed_paths: Vec<String>,
    pub protected_paths: Vec<String>,
    pub task: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ExpectedAnswers {
    #[serde(default)]
    pub rigor: Option<RigorLabels>,
    #[serde(default)]
    pub drift_out_of_scope: Option<bool>,
    // Deferred harness aspects are not production acceptance coverage.
    #[serde(default)]
    pub routing: Option<String>,
    #[serde(default)]
    pub self_heal: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigorLabels {
    pub hallucinated_import: bool,
    pub violates_conventions: bool,
    pub complexity_high: bool,
    pub contains_secrets: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptancePolicy {
    pub schema_version: u32,
    pub approval: String,
    pub requested_model: String,
    pub concrete_model: String,
    pub pricing: Pricing,
    pub required_aspects: Vec<String>,
    pub limits: Option<Limits>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pricing {
    pub version: String,
    pub source: String,
    pub currency: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub minimum_samples_per_aspect: usize,
    pub minimum_positive_per_signal: usize,
    pub minimum_negative_per_signal: usize,
    pub minimum_agreement_per_signal: f64,
    pub maximum_false_positives_per_signal: usize,
    pub maximum_false_negatives_per_signal: usize,
    pub maximum_p95_latency_ms: u64,
    pub maximum_error_rate: f64,
    pub maximum_cost_per_case: f64,
    pub maximum_total_cost: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalMetrics {
    pub samples: usize,
    pub positives: usize,
    pub negatives: usize,
    pub matches: usize,
    pub true_positives: usize,
    pub true_negatives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub agreement: f64,
}

impl SignalMetrics {
    pub(super) fn record(&mut self, expected: bool, actual: bool) {
        self.samples += 1;
        if expected {
            self.positives += 1;
        } else {
            self.negatives += 1;
        }
        match (expected, actual) {
            (true, true) => self.true_positives += 1,
            (false, false) => self.true_negatives += 1,
            (false, true) => self.false_positives += 1,
            (true, false) => self.false_negatives += 1,
        }
        self.matches = self.true_positives + self.true_negatives;
        self.agreement = agreement(self.matches, self.samples);
    }
}

impl Default for SignalMetrics {
    fn default() -> Self {
        Self {
            samples: 0,
            positives: 0,
            negatives: 0,
            matches: 0,
            true_positives: 0,
            true_negatives: 0,
            false_positives: 0,
            false_negatives: 0,
            agreement: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AspectMetrics {
    pub aspect: String,
    /// One logical evaluation per case; HTTP retries are reflected in latency and
    /// the separate transport_attempts counter.
    pub attempts: usize,
    pub successes: usize,
    pub errors: usize,
    /// Valid response bodies (including ones with unscorable answers).
    pub responses: usize,
    /// Sum of matching signal judgments, so rigor can exceed the case count.
    pub matches: usize,
    /// Matching signals divided by scored signals (failed calls contribute no signal).
    pub agreement: f64,
    pub latency_ms: Vec<u64>,
    pub p95_latency_ms: u64,
    pub signals: BTreeMap<String, SignalMetrics>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub unknown_usage_calls: usize,
    pub estimated_cost: Option<CostEstimate>,
    pub maximum_case_cost: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CostEstimate {
    pub amount: f64,
    pub price_unverified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    pub source_name: String,
    pub source_version: String,
    pub source_sha256: String,
    pub fixture_sha256: String,
    pub policy_sha256: String,
    pub requested_model: String,
    pub concrete_model: String,
    pub observed_models: Vec<String>,
    pub pricing: Pricing,
    pub scoring: BTreeMap<String, f64>,
    pub transport: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalDiagnostic {
    pub expected: bool,
    pub score: f64,
    pub threshold: f64,
    pub actual: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseDiagnostic {
    pub case_index: usize,
    pub case_id_sha256: String,
    pub aspect: &'static str,
    pub signals: BTreeMap<&'static str, SignalDiagnostic>,
    pub error: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub schema_version: u32,
    pub provenance: Provenance,
    pub client_kind: String,
    pub metrics: Vec<AspectMetrics>,
    pub case_diagnostics: Vec<CaseDiagnostic>,
    /// HTTP sends include retries; absent for the offline fake.
    pub transport_attempts: Option<u64>,
    /// Maximum combined cost of every aspect labeled on one fixture (not a bill).
    pub maximum_fixture_cost: Option<f64>,
    /// Reviewed estimated run ceiling; never a billed-spend guarantee.
    pub run_estimated_budget_usd: Option<f64>,
    pub budget_interrupted: bool,
    pub accepted: bool,
    pub failures: Vec<String>,
}

fn invalid(path: &Path, detail: impl Into<String>) -> JevError {
    JevError::InvalidFixtureSet {
        path: path.display().to_string(),
        detail: detail.into(),
    }
}

fn read_input<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<(T, String), JevError> {
    let bytes = std::fs::read(path).map_err(|e| invalid(path, e.to_string()))?;
    let digest = crate::engine::digest::sha256(&bytes);
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid(path, "input must be UTF-8"))?;
    crate::engine::yaml::from_str(text)
        .map(|input| (input, digest))
        .map_err(|_| invalid(path, "invalid versioned input schema"))
}

pub fn load_fixtures(path: &Path) -> Result<(FixtureSet, String), JevError> {
    let (set, digest): (FixtureSet, String) = read_input(path)?;
    if set.schema_version != 1
        || set.cases.is_empty()
        || !valid_tag(&set.source.name)
        || set.source.version != "1"
        || !valid_sha(&set.source.sha256)
    {
        return Err(invalid(
            path,
            "expected version 1, a nonempty source with SHA-256, and nonempty cases",
        ));
    }
    let mut ids = HashSet::new();
    let mut rigor_coverage = 0usize;
    let mut drift_coverage = 0usize;
    for case in &set.cases {
        if case.case_id.trim().is_empty() || !ids.insert(&case.case_id) {
            return Err(invalid(path, "case IDs must be nonempty and unique"));
        }
        if case.expected.rigor.is_none()
            && case.expected.drift_out_of_scope.is_none()
            && case.expected.routing.is_none()
            && case.expected.self_heal.is_none()
        {
            return Err(invalid(path, "every case needs a labeled aspect"));
        }
        if case.expected.rigor.is_some() == case.files.is_empty()
            || case.expected.drift_out_of_scope.is_some() != case.drift.is_some()
        {
            return Err(invalid(
                path,
                "rigor needs files and drift needs scope state",
            ));
        }
        if case.expected.routing.is_some() || case.expected.self_heal.is_some() {
            if !case
                .state
                .as_ref()
                .is_some_and(|state| state.is_string() || state.is_array() || state.is_object())
            {
                return Err(invalid(path, "deferred aspects require a non-null state"));
            }
        } else if case.state.is_some() {
            return Err(invalid(path, "state is only for deferred aspects"));
        }
        if case.expected.rigor.is_some() {
            rigor_coverage += 1;
        }
        if case.expected.drift_out_of_scope.is_some() {
            drift_coverage += 1;
        }
        let mut paths = HashSet::new();
        for file in &case.files {
            let path = Path::new(&file.path);
            if file.content.trim().is_empty()
                || !paths.insert(&file.path)
                || crate::engine::agent_ws::fs_boundary::RepositoryFs::validate_path(path).is_err()
                || crate::config::is_secret_path(path)
            {
                return Err(invalid(path, "unsafe, repeated or empty fixture file"));
            }
        }
        if let Some(scope) = &case.drift {
            if scope.changed_paths.is_empty()
                || scope.task.trim().is_empty()
                || scope.protected_paths.is_empty()
            {
                return Err(invalid(
                    path,
                    "drift requires a task, paths and protected paths",
                ));
            }
            for name in scope.changed_paths.iter().chain(&scope.protected_paths) {
                if crate::engine::agent_ws::fs_boundary::RepositoryFs::validate_path(Path::new(
                    name.trim_end_matches('/'),
                ))
                .is_err()
                    || crate::config::is_secret_path(Path::new(name))
                {
                    return Err(invalid(path, "unsafe drift path"));
                }
            }
            if scope
                .changed_paths
                .iter()
                .any(|changed| super::drift::path_is_protected(changed, &scope.protected_paths))
            {
                return Err(invalid(
                    path,
                    "drift acceptance case is forced by the deterministic path layer",
                ));
            }
        }
        if case.expected.routing.as_deref().is_some_and(|label| {
            label != "NONE" && !super::routing::ROUTING_TARGETS.contains(&label)
        }) || case.expected.self_heal.as_deref().is_some_and(|label| {
            !matches!(
                label,
                "REVERT" | "REFACTOR_IMPORTS" | "SIMPLIFY_LOGIC" | "ASK_HUMAN"
            )
        }) {
            return Err(invalid(path, "unsupported routing or self-heal label"));
        }
    }
    if rigor_coverage == 0 || drift_coverage == 0 {
        return Err(invalid(path, "rigor and drift coverage are both required"));
    }
    Ok((set, digest))
}

/// Verify the fixture's versioned source manifest, rather than trusting a self-reported digest.
pub fn verify_source(fixtures_path: &Path, source: &Source) -> Result<(), JevError> {
    let manifest = fixtures_path.with_file_name("source-v1.yml");
    if std::fs::symlink_metadata(&manifest)
        .map_err(|_| invalid(&manifest, "missing source manifest"))?
        .file_type()
        .is_symlink()
    {
        return Err(invalid(&manifest, "source manifest must not be a symlink"));
    }
    let bytes =
        std::fs::read(&manifest).map_err(|_| invalid(&manifest, "unreadable source manifest"))?;
    if crate::engine::digest::sha256(&bytes) != source.sha256 {
        return Err(invalid(&manifest, "source provenance digest mismatch"));
    }
    Ok(())
}

fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

pub fn load_policy(path: &Path) -> Result<(AcceptancePolicy, String), JevError> {
    let (policy, digest): (AcceptancePolicy, String) = read_input(path)?;
    if policy.schema_version != 1
        || !matches!(policy.approval.as_str(), "pending" | "approved")
        || policy.requested_model != super::JEV_MODEL
        || !valid_tag(&policy.concrete_model)
        || policy.concrete_model == policy.requested_model
        || !valid_tag(&policy.pricing.version)
        || policy.pricing.source != "https://docs.typesafe.ai/models"
        || policy.pricing.currency != "USD"
        || !policy.pricing.input_per_million.is_finite()
        || policy.pricing.input_per_million < 0.0
        || !policy.pricing.output_per_million.is_finite()
        || policy.pricing.output_per_million < 0.0
        || policy.required_aspects.len() != 2
        || policy.required_aspects[0] != "rigor"
        || policy.required_aspects[1] != "drift"
    {
        return Err(invalid(
            path,
            "unsupported policy schema, model, price or required aspects",
        ));
    }
    if policy.approval == "approved" && policy.limits.is_none() {
        return Err(invalid(
            path,
            "approved policies require reviewed numerical limits",
        ));
    }
    if let Some(l) = &policy.limits {
        if l.minimum_samples_per_aspect == 0
            || l.minimum_positive_per_signal == 0
            || l.minimum_negative_per_signal == 0
            || !unit(l.minimum_agreement_per_signal)
            || !unit(l.maximum_error_rate)
            || l.maximum_p95_latency_ms == 0
            || !positive(l.maximum_cost_per_case)
            || !positive(l.maximum_total_cost)
        {
            return Err(invalid(path, "invalid acceptance limits"));
        }
    }
    Ok((policy, digest))
}

/// Fail incomplete or one-sided suites before a billable call, including under a pending policy.
pub fn validate_coverage(
    set: &FixtureSet,
    policy: &AcceptancePolicy,
    path: &Path,
) -> Result<(), JevError> {
    let min_samples = policy
        .limits
        .as_ref()
        .map_or(1, |l| l.minimum_samples_per_aspect);
    let min_positive = policy
        .limits
        .as_ref()
        .map_or(1, |l| l.minimum_positive_per_signal);
    let min_negative = policy
        .limits
        .as_ref()
        .map_or(1, |l| l.minimum_negative_per_signal);
    let mut signals = BTreeMap::from([
        ("hallucinated_import", (0usize, 0usize)),
        ("violates_conventions", (0, 0)),
        ("complexity_high", (0, 0)),
        ("contains_secrets", (0, 0)),
        ("out_of_scope", (0, 0)),
    ]);
    let mut rigor_cases = 0;
    let mut drift_cases = 0;
    for case in &set.cases {
        if let Some(rigor) = &case.expected.rigor {
            rigor_cases += 1;
            for (name, expected) in [
                ("hallucinated_import", rigor.hallucinated_import),
                ("violates_conventions", rigor.violates_conventions),
                ("complexity_high", rigor.complexity_high),
                ("contains_secrets", rigor.contains_secrets),
            ] {
                let counts = signals
                    .get_mut(name)
                    .ok_or_else(|| invalid(path, "unknown rigor signal"))?;
                if expected {
                    counts.0 += 1;
                } else {
                    counts.1 += 1;
                }
            }
        }
        if let Some(expected) = case.expected.drift_out_of_scope {
            drift_cases += 1;
            let counts = signals
                .get_mut("out_of_scope")
                .ok_or_else(|| invalid(path, "unknown drift signal"))?;
            if expected {
                counts.0 += 1;
            } else {
                counts.1 += 1;
            }
        }
    }
    if rigor_cases < min_samples
        || drift_cases < min_samples
        || signals
            .values()
            .any(|(positive, negative)| *positive < min_positive || *negative < min_negative)
    {
        return Err(invalid(
            path,
            "missing required rigor/drift sample or positive/negative signal coverage",
        ));
    }
    Ok(())
}
fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

pub fn agreement(matches: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        matches as f64 / total as f64
    }
}

pub fn cost_estimate(
    input_tokens: u64,
    output_tokens: u64,
    price: Option<JevPrice>,
) -> Option<CostEstimate> {
    let price = price?;
    Some(CostEstimate {
        amount: input_tokens as f64 / TOKENS_PER_MILLION * price.input_per_million
            + output_tokens as f64 / TOKENS_PER_MILLION * price.output_per_million,
        price_unverified: true,
    })
}

pub fn evaluate_acceptance(report: &mut BenchmarkReport, policy: &AcceptancePolicy) {
    let fail = &mut report.failures;
    if policy.approval != "approved" {
        fail.push("acceptance policy is pending operator approval".into());
    }
    let Some(limits) = &policy.limits else {
        fail.push("reviewed numerical acceptance limits are absent".into());
        report.accepted = false;
        return;
    };
    if report.client_kind != "http" {
        fail.push("offline fake client is not production evidence".into());
    }
    if report.budget_interrupted {
        fail.push(
            "estimated run budget or unknown failed usage interrupted remaining cases".into(),
        );
    }
    if report.client_kind == "http"
        && !report
            .run_estimated_budget_usd
            .is_some_and(|budget| budget.is_finite() && budget > 0.0)
    {
        fail.push("reviewed estimated run budget is missing".into());
    }
    if report.provenance.requested_model != policy.requested_model
        || report.provenance.concrete_model != policy.concrete_model
        || report.provenance.observed_models.len() != 1
        || report
            .provenance
            .observed_models
            .first()
            .map(String::as_str)
            != Some(policy.concrete_model.as_str())
    {
        fail.push(
            "requested or responding model does not match the approved concrete model".into(),
        );
    }
    if report.provenance.pricing.version != policy.pricing.version
        || report.provenance.pricing.currency != policy.pricing.currency
        || report.provenance.pricing.source != policy.pricing.source
        || report.provenance.pricing.input_per_million != policy.pricing.input_per_million
        || report.provenance.pricing.output_per_million != policy.pricing.output_per_million
    {
        fail.push("pricing provenance mismatch".into());
    }
    if let Some(sends) = report.transport_attempts {
        let responses: usize = report.metrics.iter().map(|m| m.responses).sum();
        if sends > responses as u64 {
            fail.push("retry or failed HTTP attempts have unobservable token usage/cost".into());
        }
    } else {
        fail.push("HTTP transport attempt accounting is missing".into());
    }
    let mut total_cost = 0.0;
    for aspect in &policy.required_aspects {
        let Some(m) = report.metrics.iter().find(|m| &m.aspect == aspect) else {
            fail.push(format!("missing {aspect} coverage"));
            continue;
        };
        if m.attempts < limits.minimum_samples_per_aspect || m.successes == 0 {
            fail.push(format!("{aspect}: insufficient successful coverage"));
        }
        if m.errors as f64 / m.attempts.max(1) as f64 > limits.maximum_error_rate {
            fail.push(format!("{aspect}: error rate exceeds limit"));
        }
        if m.p95_latency_ms > limits.maximum_p95_latency_ms {
            fail.push(format!("{aspect}: p95 latency exceeds limit"));
        }
        if m.unknown_usage_calls != 0 {
            fail.push(format!("{aspect}: failed calls have unknown usage/cost"));
        }
        // Cost across all aspects, including deferred harness probes, is checked below.
        let required: &[&str] = if aspect == "rigor" {
            &[
                "hallucinated_import",
                "violates_conventions",
                "complexity_high",
                "contains_secrets",
            ]
        } else {
            &["out_of_scope"]
        };
        for signal in required {
            match m.signals.get(*signal) {
                Some(s)
                    if s.samples >= limits.minimum_samples_per_aspect
                        && s.positives >= limits.minimum_positive_per_signal
                        && s.negatives >= limits.minimum_negative_per_signal
                        && s.agreement >= limits.minimum_agreement_per_signal
                        && s.false_positives <= limits.maximum_false_positives_per_signal
                        && s.false_negatives <= limits.maximum_false_negatives_per_signal => {}
                _ => fail.push(format!(
                    "{aspect}.{signal}: coverage, agreement or confusion limit missed"
                )),
            }
        }
    }
    for metric in &report.metrics {
        if metric.unknown_usage_calls != 0 {
            fail.push(format!("{}: unknown failed-call usage/cost", metric.aspect));
        }
        match metric.estimated_cost {
            Some(cost) if cost.amount.is_finite() => total_cost += cost.amount,
            _ => fail.push(format!("{}: cost is unknown", metric.aspect)),
        }
    }
    match report.maximum_fixture_cost {
        Some(cost) if cost.is_finite() && cost <= limits.maximum_cost_per_case => {}
        _ => fail.push("per-fixture combined cost exceeds limit or is unknown".into()),
    }
    if total_cost > limits.maximum_total_cost {
        fail.push("total estimated cost exceeds limit".into());
    }
    if report
        .run_estimated_budget_usd
        .is_some_and(|budget| total_cost > budget)
    {
        fail.push("total estimated cost exceeds the reviewed run budget".into());
    }
    report.accepted = fail.is_empty();
}

pub fn write_report(repo_root: &Path, report: &BenchmarkReport) -> Result<String, JevError> {
    let yaml = crate::engine::yaml::to_string(report).map_err(|error| JevError::Config {
        path: REPORT_REL_PATH.to_string(),
        detail: format!("could not serialize benchmark report: {error}"),
    })?;
    crate::engine::agent_ws::write_under_agent(repo_root, Path::new(REPORT_REL_PATH), &yaml)
        .map_err(|error| JevError::Config {
            path: REPORT_REL_PATH.to_string(),
            detail: format!("could not write benchmark report: {error}"),
        })?;
    Ok(REPORT_REL_PATH.to_string())
}

#[cfg(test)]
#[path = "bench_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bench_validation_tests.rs"]
mod validation_tests;

#[cfg(test)]
#[path = "bench_case_diagnostics_tests.rs"]
mod case_diagnostics_tests;
