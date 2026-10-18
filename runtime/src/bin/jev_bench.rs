//! The Jev benchmark bin (`jev-bench`).
//!
//! Runs anonymized versioned rigor/drift cases using the production state builders and
//! evaluator contracts. The default fake client makes no network request and cannot pass
//! production acceptance. An explicit `JEV_BENCH_CLIENT=http` requires a positive reviewed
//! `JEV_BENCH_MAX_ESTIMATED_USD` and `JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED=true`;
//! pending policies also require `JEV_BENCH_CALIBRATE=1` and always fail acceptance.
//! The estimate stops further calls after known usage reaches the limit; only the
//! provider-side cap bounds billing, including failed calls and hidden transport retries.
//! Inputs default to `benchmarks/jev/fixtures-v1.yml` and `policy-v1.yml` under the repo
//! root; override with `JEV_BENCH_FIXTURES` and `JEV_BENCH_POLICY`. Only aggregate reports
//! are written through the guarded `.agent/telemetry/` writer.

// The whole body is guarded on the feature. The `required-features` entry already limits the
// bin to the feature build, and this guard makes the intent explicit at the file level.
#[cfg(feature = "jev-http")]
fn main() -> std::process::ExitCode {
    imp::run()
}

// A default build never compiles the bin (the `required-features` gate), so this fallback is
// present only for a direct `--bin jev-bench` without the feature. It states the cause and
// exits non-zero rather than build an empty bin.
#[cfg(not(feature = "jev-http"))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "jev-bench: this bin requires the `jev-http` feature. \
         Re-run with `cargo run --features jev-http --bin jev-bench`."
    );
    std::process::ExitCode::FAILURE
}

#[cfg(feature = "jev-http")]
mod imp {
    use std::path::PathBuf;
    use std::process::ExitCode;

    use std::io::Read;

    use truenorth_mcp::config::get_repo_root;
    use truenorth_mcp::engine::jev::bench;
    use truenorth_mcp::engine::jev::client_fake::FakeClient;
    use truenorth_mcp::engine::jev::client_http::{HttpClient, JevActivation};
    use truenorth_mcp::engine::jev::config::{resolve, resolve_protected_paths};
    use truenorth_mcp::engine::jev::guard::ProposedChange;
    use truenorth_mcp::engine::jev::guard::evaluate::evaluate_guard;
    use truenorth_mcp::engine::jev::guard_cli::{block_message, exit_code_for};
    /// Offline fake remains the default; an HTTP run requires an explicit operator choice.
    const CLIENT_VAR: &str = "JEV_BENCH_CLIENT";
    const FIXTURES_VAR: &str = "JEV_BENCH_FIXTURES";
    const POLICY_VAR: &str = "JEV_BENCH_POLICY";
    const DEFAULT_FIXTURES: &str = "benchmarks/jev/fixtures-v1.yml";
    const DEFAULT_POLICY: &str = "benchmarks/jev/policy-v1.yml";

    pub fn run() -> ExitCode {
        match std::env::args().nth(1).as_deref() {
            Some("guard") => run_guard(),
            None | Some("bench") => run_bench(),
            _ => fail("unknown subcommand. Use `bench` or `guard`."),
        }
    }

    /// Validate all versioned inputs before any network call. Invalid input leaves prior
    /// aggregate reports untouched. Acceptance failure writes the rejected report first.
    fn run_bench() -> ExitCode {
        let repo_root = match get_repo_root() {
            Ok(root) => root,
            Err(error) => return fail(&format!("could not resolve the repository root: {error}")),
        };
        let config = match resolve(&repo_root) {
            Ok(config) => config,
            Err(error) => return fail(&format!("could not resolve the Jev config: {error}")),
        };
        let fixtures_path = match input_path(&repo_root, FIXTURES_VAR, DEFAULT_FIXTURES) {
            Ok(path) => path,
            Err(message) => return fail(&message),
        };
        let policy_path = match input_path(&repo_root, POLICY_VAR, DEFAULT_POLICY) {
            Ok(path) => path,
            Err(message) => return fail(&message),
        };
        let (fixtures, fixture_sha256) = match bench::load_fixtures(&fixtures_path) {
            Ok(value) => value,
            Err(error) => return fail(&error.to_string()),
        };
        if let Err(error) = bench::verify_source(&fixtures_path, &fixtures.source) {
            return fail(&error.to_string());
        }
        let (policy, policy_sha256) = match bench::load_policy(&policy_path) {
            Ok(value) => value,
            Err(error) => return fail(&error.to_string()),
        };
        if let Err(error) = bench::validate_coverage(&fixtures, &policy, &fixtures_path) {
            return fail(&error.to_string());
        }
        let client_kind = std::env::var(CLIENT_VAR).unwrap_or_else(|_| "fake".into());
        // Calibration is explicitly operator-authorized and cannot pass production
        // acceptance while approval remains pending.
        if client_kind == "http"
            && policy.approval != "approved"
            && std::env::var("JEV_BENCH_CALIBRATE").as_deref() != Ok("1")
        {
            return fail(
                "HTTP calibration under a pending policy requires JEV_BENCH_CALIBRATE=1; no call was made",
            );
        }
        let estimated_budget_usd = if client_kind == "http" {
            let budget = match std::env::var("JEV_BENCH_MAX_ESTIMATED_USD")
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                Some(value) => value,
                None => {
                    return fail(
                        "HTTP benchmark requires a positive reviewed JEV_BENCH_MAX_ESTIMATED_USD; no call was made",
                    );
                }
            };
            if std::env::var("JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED").as_deref() != Ok("true") {
                return fail(
                    "HTTP benchmark requires JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED=true; no call was made",
                );
            }
            Some(
                policy
                    .limits
                    .as_ref()
                    .map_or(budget, |limits| budget.min(limits.maximum_total_cost)),
            )
        } else {
            None
        };
        let (measurements, transport_attempts) = match run_client(
            &client_kind,
            &fixtures,
            &config,
            &policy.pricing,
            estimated_budget_usd,
        ) {
            Ok(value) => value,
            Err(message) => return fail(&message),
        };
        let scoring = [
            ("rigor_failure_boundary", config.rigor_failure_boundary),
            ("complexity_boundary", config.complexity_boundary),
            ("drift_boundary", config.drift_boundary),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let transport = [
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
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let mut report = bench::BenchmarkReport {
            schema_version: 2,
            provenance: bench::Provenance {
                source_name: fixtures.source.name.clone(),
                source_version: fixtures.source.version.clone(),
                source_sha256: fixtures.source.sha256.clone(),
                fixture_sha256,
                policy_sha256,
                requested_model: policy.requested_model.clone(),
                concrete_model: policy.concrete_model.clone(),
                observed_models: measurements.models,
                pricing: policy.pricing.clone(),
                scoring,
                transport,
            },
            client_kind,
            metrics: measurements.metrics,
            case_diagnostics: measurements.case_diagnostics,
            transport_attempts,
            maximum_fixture_cost: measurements.maximum_fixture_cost,
            run_estimated_budget_usd: estimated_budget_usd,
            budget_interrupted: measurements.budget_interrupted,
            accepted: false,
            failures: Vec::new(),
        };
        bench::evaluate_acceptance(&mut report, &policy);
        match bench::write_report(&repo_root, &report) {
            Ok(rel_path) => {
                println!("{rel_path}");
                if report.accepted {
                    ExitCode::SUCCESS
                } else {
                    fail("production acceptance failed; see aggregate report")
                }
            }
            Err(error) => fail(&error.to_string()),
        }
    }

    /// Run the guardrail over a proposed change on standard input, returning the exit code.
    ///
    /// The steps run in order: read the proposed change JSON on standard input, resolve the
    /// repository root, config, protected paths, and validated Jev activation, then run the
    /// guardrail. An invalid activation exits non-zero before any HTTP request.
    fn run_guard() -> ExitCode {
        let mut input = String::new();
        if let Err(error) = std::io::stdin().read_to_string(&mut input) {
            return fail(&format!(
                "could not read the proposed change on standard input: {error}"
            ));
        }

        let change: ProposedChange = match serde_json::from_str(&input) {
            Ok(change) => change,
            Err(error) => {
                return fail(&format!(
                    "could not parse the proposed change JSON: {error}. Expected an object with \
                     `paths` and `content`."
                ));
            }
        };

        let repo_root = match get_repo_root() {
            Ok(root) => root,
            Err(error) => return fail(&format!("could not resolve the repository root: {error}")),
        };
        let config = match resolve(&repo_root) {
            Ok(config) => config,
            Err(error) => return fail(&format!("could not resolve the Jev config: {error}")),
        };
        let protected = match resolve_protected_paths(&repo_root) {
            Ok(paths) => paths,
            Err(error) => return fail(&format!("could not resolve the protected paths: {error}")),
        };

        let activation = match JevActivation::resolve() {
            Ok(activation) => activation,
            Err(error) => return fail(&format!("could not resolve the Jev activation: {error}")),
        };

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => return fail(&format!("could not start the async runtime: {error}")),
        };

        let client = HttpClient::new(activation, &config);
        let decision = match runtime.block_on(evaluate_guard(
            &change,
            &protected,
            &config,
            true,
            true,
            Some(&client),
            &repo_root,
        )) {
            Ok(decision) => decision,
            Err(error) => return fail(&format!("invalid guard input: {error}")),
        };

        // Print the block reason to standard error on a block; print nothing on a pass (R7.2).
        if let Some(message) = block_message(&decision) {
            eprintln!("jev-guard: {message}");
        }
        ExitCode::from(exit_code_for(&decision))
    }

    fn input_path(
        repo_root: &std::path::Path,
        variable: &str,
        default: &str,
    ) -> Result<PathBuf, String> {
        let selected = std::env::var(variable).unwrap_or_else(|_| default.into());
        let path = PathBuf::from(selected);
        let path = if path.is_absolute() {
            path
        } else {
            repo_root.join(path)
        };
        let base = repo_root.join("benchmarks/jev");
        if path.parent() != Some(base.as_path())
            || path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(format!(
                "{variable} must name an input directly under benchmarks/jev/"
            ));
        }
        // Inputs stay in the versioned benchmark directory, outside telemetry. Refuse
        // symlinks at each boundary rather than following a redirected manifest.
        for part in [repo_root.join("benchmarks"), base.clone(), path.clone()] {
            if std::fs::symlink_metadata(part).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(format!("{variable} must not traverse a symlink"));
            }
        }
        if !path.is_file() {
            return Err(format!(
                "{variable} must name an existing regular file under benchmarks/jev/"
            ));
        }
        Ok(path)
    }

    fn run_client(
        kind: &str,
        fixtures: &bench::FixtureSet,
        config: &truenorth_mcp::engine::jev::config::JevConfig,
        pricing: &bench::Pricing,
        estimated_budget_usd: Option<f64>,
    ) -> Result<(bench::BenchmarkMeasurements, Option<u64>), String> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| format!("could not start the async runtime: {error}"))?;
        match kind {
            "fake" => {
                let client = FakeClient::new();
                let measurements = runtime
                    .block_on(bench::run_benchmark(&client, fixtures, config, pricing))
                    .map_err(|_| "could not build anonymized production state".to_string())?;
                Ok((measurements, None))
            }
            "http" => {
                let activation = JevActivation::resolve()
                    .map_err(|error| format!("could not resolve the Jev activation: {error}"))?;
                let client = HttpClient::new(activation, config);
                let measurements = runtime
                    .block_on(bench::run_benchmark_with_budget(
                        &client,
                        fixtures,
                        config,
                        pricing,
                        estimated_budget_usd,
                    ))
                    .map_err(|_| "could not build anonymized production state".to_string())?;
                Ok((measurements, Some(client.attempt_count())))
            }
            other => Err(format!(
                "unknown `{CLIENT_VAR}` value `{other}`. Use `fake` or `http`."
            )),
        }
    }

    /// Print a named cause to stderr and return the failure exit code.
    fn fail(message: &str) -> ExitCode {
        eprintln!("jev-bench: {message}");
        ExitCode::FAILURE
    }
}
