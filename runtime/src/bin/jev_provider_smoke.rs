//! Explicit, protected provider compatibility probe; no provider values or payloads are logged.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};
use truenorth_mcp::engine::jev::client_http::{HttpClient, JEV_API_KEY_VAR, JevActivation};
use truenorth_mcp::engine::jev::config::JevConfig;
use truenorth_mcp::engine::jev::provider_smoke::{self, Counters, Outcome, SmokePolicy};

const POLICY: &str = "benchmarks/jev/policy-v1.yml";
const TIMEOUT_ENDPOINT: &str = "TRUENORTH_JEV_TIMEOUT_ENDPOINT";
const RETRY_ENDPOINT: &str = "TRUENORTH_JEV_RETRY_ENDPOINT";
const BUDGET_VAR: &str = "JEV_SMOKE_MAX_ESTIMATED_USD";
const SPEND_CAP_VAR: &str = "JEV_SMOKE_PROVIDER_SPEND_CAP_CONFIRMED";
const CONTROL_APPROVED_VAR: &str = "JEV_SMOKE_CONTROL_APPROVED";

#[derive(Serialize)]
struct Scenario {
    outcome: Outcome,
    /// HTTP status class, not provider text or response body.
    status_class: &'static str,
    elapsed_ms: u128,
    request_attempts: u64,
}

#[derive(Serialize)]
struct Evidence<'a> {
    schema_version: u32,
    source_sha: &'a str,
    binary_sha256: &'a str,
    policy_sha256: &'a str,
    contract: &'static str,
    synthetic_fixture_version: &'static str,
    control_modes_approved: bool,
    requested_model: &'a str,
    concrete_model: &'a str,
    price_version: &'a str,
    price_source: &'a str,
    currency: &'a str,
    max_estimated_usd: f64,
    estimated_success_cost_usd: Option<f64>,
    /// Non-200 attempts have no usage response; a provider spend cap must be set separately.
    failed_attempt_usage_unknown: bool,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    compatibility: Scenario,
    unauthorized: Scenario,
    timeout: Scenario,
    retry: Scenario,
    approval: &'a str,
    accepted: bool,
}

fn no_evidence() -> Scenario {
    Scenario {
        outcome: Outcome::NotProven,
        status_class: "none",
        elapsed_ms: 0,
        request_attempts: 0,
    }
}

fn checked_sha(value: String) -> Result<String, &'static str> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid source SHA");
    }
    Ok(value)
}

fn sha256_file(path: &Path) -> Result<String, &'static str> {
    let mut file = File::open(path).map_err(|_| "cannot read validation binary")?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| "cannot hash validation binary")?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest.finalize() {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(output)
}

fn load_policy() -> Result<(SmokePolicy, String), &'static str> {
    let (policy, hash) = truenorth_mcp::engine::jev::bench::load_policy(Path::new(POLICY))
        .map_err(|_| "invalid or incomplete smoke policy")?;
    let policy = SmokePolicy::from(policy);
    policy.validate()?;
    Ok((policy, hash))
}

fn activation_from_environment(
    variable: &str,
    approved: &JevActivation,
) -> Result<Option<JevActivation>, &'static str> {
    let endpoint = match std::env::var(variable) {
        Ok(value) if !value.is_empty() => value,
        _ => return Ok(None),
    };
    let key = std::env::var(JEV_API_KEY_VAR).map_err(|_| "missing API key")?;
    let activation = JevActivation::from_values(Some(&endpoint), Some(&key))
        .map_err(|_| "invalid controlled endpoint")?;
    if !provider_smoke::same_origin(approved.endpoint(), activation.endpoint()) {
        return Err("controlled endpoint is outside approved provider origin");
    }
    if activation.endpoint() == approved.endpoint() {
        return Err("controlled endpoint must not be the production evaluation path");
    }
    Ok(Some(activation))
}

struct Preflight {
    policy: SmokePolicy,
    policy_sha: String,
    source_sha: String,
    binary_sha: String,
    activation: JevActivation,
    timeout_activation: Option<JevActivation>,
    retry_activation: Option<JevActivation>,
    controls_approved: bool,
    max_cost: f64,
}

fn preflight() -> Result<Preflight, &'static str> {
    let (policy, policy_sha) = load_policy()?;
    let source_sha = checked_sha(
        std::env::var("GITHUB_SHA")
            .or_else(|_| std::env::var("JEV_SMOKE_SOURCE_SHA"))
            .map_err(|_| "missing source SHA")?,
    )?;
    let binary_sha =
        sha256_file(&std::env::current_exe().map_err(|_| "cannot locate validation binary")?)?;
    let budget = std::env::var(BUDGET_VAR)
        .map_err(|_| "missing reviewed spending ceiling")?
        .parse::<f64>()
        .map_err(|_| "invalid spending ceiling")?;
    if !budget.is_finite() || budget <= 0.0 {
        return Err("invalid spending ceiling");
    }
    if !std::env::var(SPEND_CAP_VAR).is_ok_and(|value| value == "true") {
        return Err("provider-side spending cap not confirmed");
    }
    let activation = JevActivation::resolve().map_err(|_| "invalid provider activation")?;
    let controls_approved = std::env::var(CONTROL_APPROVED_VAR).is_ok_and(|value| value == "true");
    let (timeout, retry) = if controls_approved {
        (
            activation_from_environment(TIMEOUT_ENDPOINT, &activation)?,
            activation_from_environment(RETRY_ENDPOINT, &activation)?,
        )
    } else {
        (None, None)
    };
    ensure_report_directory()?;
    Ok(Preflight {
        policy,
        policy_sha,
        source_sha,
        binary_sha,
        activation,
        timeout_activation: timeout,
        retry_activation: retry,
        controls_approved,
        max_cost: budget,
    })
}

/// Refuse linked output directories before sending any paid request.
fn ensure_report_directory() -> Result<PathBuf, &'static str> {
    let root = std::env::current_dir().map_err(|_| "cannot locate repository")?;
    let agent = root.join(".agent");
    let telemetry = agent.join("telemetry");
    for path in [&agent, &telemetry] {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(path).map_err(|_| "cannot create telemetry directory")?;
            }
            _ => return Err("unsafe telemetry directory"),
        }
    }
    Ok(telemetry)
}

/// Create a fresh atomic aggregate report without printing absolute paths.
fn save_report(source_sha: &str, json: &str) -> Result<PathBuf, &'static str> {
    let telemetry = ensure_report_directory()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid clock")?
        .as_millis();
    let root = std::env::current_dir().map_err(|_| "cannot locate repository")?;
    let relative = PathBuf::from(format!(
        "telemetry/jev-provider-validation-{source_sha}-{stamp}.json"
    ));
    truenorth_mcp::engine::agent_ws::write_under_agent(&root, &relative, json)
        .map_err(|_| "cannot write aggregate report")?;
    Ok(telemetry.join(relative.file_name().ok_or("invalid report filename")?))
}

fn elapsed(start: Instant) -> u128 {
    start.elapsed().as_millis()
}

async fn probe() -> Result<bool, &'static str> {
    // Every environment value, price, endpoint, and report destination is validated before HTTP.
    let Preflight {
        policy,
        policy_sha,
        source_sha,
        binary_sha,
        activation,
        timeout_activation,
        retry_activation,
        controls_approved,
        max_cost,
    } = preflight()?;
    let config = JevConfig {
        retry_limit: 0,
        timeout: Duration::from_secs(15),
        ..JevConfig::default()
    };
    let client = HttpClient::new(activation.clone(), &config);
    let started = Instant::now();
    let before = client.attempt_count();
    let positive = tokio::time::timeout(
        Duration::from_secs(32),
        provider_smoke::compatibility(&client, &policy, max_cost),
    )
    .await;
    let counters: Option<Counters> = match positive {
        Ok(Ok(counts)) => Some(counts),
        _ => None,
    };
    let compatibility = Scenario {
        outcome: if counters.is_some() && client.attempt_count() - before == 2 {
            Outcome::Passed
        } else {
            Outcome::Failed
        },
        status_class: if counters.is_some() { "2xx" } else { "unknown" },
        elapsed_ms: elapsed(started),
        request_attempts: client.attempt_count() - before,
    };
    // If the authenticated call failed, do not send further requests to an unknown provider.
    let mut unauthorized = no_evidence();
    let mut timeout = no_evidence();
    let mut retry = no_evidence();
    if compatibility.outcome == Outcome::Passed {
        let real_key =
            std::env::var(JEV_API_KEY_VAR).map_err(|_| "key disappeared during validation")?;
        unsafe { std::env::set_var(JEV_API_KEY_VAR, "jev-validation-intentionally-invalid-key") };
        let start = Instant::now();
        let before = client.attempt_count();
        let rejected = tokio::time::timeout(
            Duration::from_secs(17),
            provider_smoke::unauthorized(&client),
        )
        .await;
        unsafe { std::env::set_var(JEV_API_KEY_VAR, real_key) };
        unauthorized = Scenario {
            outcome: if matches!(rejected, Ok(Ok(()))) && client.attempt_count() - before == 1 {
                Outcome::Passed
            } else {
                Outcome::Failed
            },
            status_class: if matches!(rejected, Ok(Ok(()))) {
                "4xx"
            } else {
                "unknown"
            },
            elapsed_ms: elapsed(start),
            request_attempts: client.attempt_count() - before,
        };
    }
    if unauthorized.outcome == Outcome::Passed {
        if let Some(control) = timeout_activation {
            let mut control_config = config.clone();
            control_config.timeout = Duration::from_secs(2);
            let client = HttpClient::new(control, &control_config);
            let start = Instant::now();
            let result =
                tokio::time::timeout(Duration::from_secs(4), provider_smoke::timeout(&client))
                    .await;
            timeout = Scenario {
                outcome: if matches!(result, Ok(Ok(()))) && client.attempt_count() == 1 {
                    Outcome::Passed
                } else {
                    Outcome::Failed
                },
                status_class: "none",
                elapsed_ms: elapsed(start),
                request_attempts: client.attempt_count(),
            };
        }
        if let Some(control) = retry_activation {
            let mut control_config = config;
            control_config.retry_limit = 1;
            control_config.max_backoff = Duration::from_secs(2);
            control_config.timeout = Duration::from_secs(8);
            let client = HttpClient::new(control, &control_config);
            let start = Instant::now();
            let result =
                tokio::time::timeout(Duration::from_secs(10), provider_smoke::retry(&client)).await;
            let status_class = match result {
                Ok(Ok(429)) => "4xx",
                Ok(Ok(529)) => "5xx",
                _ => "unknown",
            };
            retry = Scenario {
                outcome: if status_class != "unknown" && client.attempt_count() == 2 {
                    Outcome::Passed
                } else {
                    Outcome::Failed
                },
                status_class,
                elapsed_ms: elapsed(start),
                request_attempts: client.attempt_count(),
            };
        }
    }
    let accepted = provider_smoke::accepted(
        &policy,
        [
            compatibility.outcome,
            unauthorized.outcome,
            timeout.outcome,
            retry.outcome,
        ],
        started.elapsed(),
    );
    let evidence = Evidence {
        schema_version: 1,
        source_sha: &source_sha,
        binary_sha256: &binary_sha,
        policy_sha256: &policy_sha,
        contract: "https://docs.typesafe.ai/api",
        synthetic_fixture_version: "provider-smoke-rigor-drift-v1",
        control_modes_approved: controls_approved,
        requested_model: &policy.requested_model,
        concrete_model: &policy.concrete_model,
        price_version: &policy.pricing.version,
        price_source: &policy.pricing.source,
        currency: &policy.pricing.currency,
        max_estimated_usd: max_cost,
        estimated_success_cost_usd: counters.map(|v| v.estimated_cost_usd),
        failed_attempt_usage_unknown: true,
        input_tokens: counters.map(|v| v.input_tokens),
        output_tokens: counters.map(|v| v.output_tokens),
        compatibility,
        unauthorized,
        timeout,
        retry,
        approval: &policy.approval,
        accepted,
    };
    let report =
        serde_json::to_string_pretty(&evidence).map_err(|_| "cannot serialize aggregate report")?;
    let path = save_report(&source_sha, &report)?;
    println!(
        "aggregate evidence: .agent/telemetry/{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("report.json")
    );
    println!(
        "provider compatibility acceptance: {}",
        if accepted { "passed" } else { "not proven" }
    );
    Ok(accepted)
}

#[tokio::main]
async fn main() -> ExitCode {
    match probe().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(reason) => {
            eprintln!("provider compatibility preflight/report failed: {reason}");
            ExitCode::FAILURE
        }
    }
}
