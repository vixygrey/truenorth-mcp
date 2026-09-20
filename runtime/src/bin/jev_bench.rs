//! The Jev benchmark bin (`jev-bench`).
//!
//! This bin runs the Jev evaluation benchmark and writes the report under
//! `.agent/telemetry/`. It builds only under the `jev-http` feature (see the `[[bin]]`
//! `required-features` entry in `Cargo.toml`), so `cargo test` on the default build neither
//! compiles nor runs it (R11.10). It reuses the `truenorth_mcp` library crate to reach the
//! benchmark runner in `engine::jev::bench`.
//!
//! The bin selects the client from the `JEV_BENCH_CLIENT` environment variable: `fake`
//! (the default) drives the named fake and makes no network call (R11.2, R11.7); `http`
//! drives the real HTTP client, which reads the API key at call time. It reads the fixture
//! path from `JEV_BENCH_FIXTURES`, or a default under the repository root. It resolves the
//! `JevConfig` from `.agent/config/rules.yml`.
//!
//! Requirements: 11.2, 11.7, 11.10. Design: jev-integration-eval, the benchmark bin.

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

    use truenorth_mcp::config::get_repo_root;
    use truenorth_mcp::engine::jev::bench;
    use truenorth_mcp::engine::jev::client_fake::FakeClient;
    use truenorth_mcp::engine::jev::client_http::HttpClient;
    use truenorth_mcp::engine::jev::config::resolve;

    /// The environment variable that selects the client: `fake` or `http`.
    const CLIENT_VAR: &str = "JEV_BENCH_CLIENT";
    /// The environment variable that holds the fixture-set path.
    const FIXTURES_VAR: &str = "JEV_BENCH_FIXTURES";
    /// The environment variable that holds the Jev endpoint URL for the `http` client.
    const ENDPOINT_VAR: &str = "JEV_BENCH_ENDPOINT";
    /// The default fixture path under the repository root, when `JEV_BENCH_FIXTURES` is unset.
    const DEFAULT_FIXTURES: &str = ".agent/telemetry/jev-fixtures.yml";

    /// Run the benchmark and write the report, returning the process exit code.
    ///
    /// The steps run in order: resolve the repository root, resolve the config, load the
    /// fixtures, run the chosen client over the fixtures, write the report, and print the
    /// report path. Any step failure prints the named cause to stderr and exits non-zero.
    pub fn run() -> ExitCode {
        let repo_root = match get_repo_root() {
            Ok(root) => root,
            Err(error) => return fail(&format!("could not resolve the repository root: {error}")),
        };

        let config = match resolve(&repo_root) {
            Ok(config) => config,
            Err(error) => return fail(&format!("could not resolve the Jev config: {error}")),
        };

        let fixtures_path = fixtures_path(&repo_root);
        let fixtures = match bench::load_fixtures(&fixtures_path) {
            Ok(fixtures) => fixtures,
            Err(error) => return fail(&error.to_string()),
        };

        let client_kind = std::env::var(CLIENT_VAR).unwrap_or_else(|_| "fake".to_string());
        let metrics = match run_client(&client_kind, &fixtures, &config) {
            Ok(metrics) => metrics,
            Err(message) => return fail(&message),
        };

        match bench::write_report(&repo_root, &metrics) {
            Ok(rel_path) => {
                println!("{rel_path}");
                ExitCode::SUCCESS
            }
            Err(error) => fail(&error.to_string()),
        }
    }

    /// The fixture path from `JEV_BENCH_FIXTURES`, or the default under the repository root.
    fn fixtures_path(repo_root: &std::path::Path) -> PathBuf {
        match std::env::var(FIXTURES_VAR) {
            Ok(path) => PathBuf::from(path),
            Err(_) => repo_root.join(DEFAULT_FIXTURES),
        }
    }

    /// Run the selected client over the fixtures, returning the metrics or an error message.
    ///
    /// The `fake` client makes no network call (R11.2, R11.7). The `http` client reads the
    /// endpoint from `JEV_BENCH_ENDPOINT` and reads the API key at call time. An unknown
    /// client name is an error, so a typo does not silently fall back.
    fn run_client(
        kind: &str,
        fixtures: &[bench::Fixture],
        config: &truenorth_mcp::engine::jev::config::JevConfig,
    ) -> Result<Vec<bench::AspectMetrics>, String> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| format!("could not start the async runtime: {error}"))?;

        match kind {
            "fake" => {
                let client = FakeClient::new();
                Ok(runtime.block_on(bench::run_benchmark(&client, fixtures, config)))
            }
            "http" => {
                let endpoint = std::env::var(ENDPOINT_VAR).map_err(|_| {
                    format!(
                        "the `{ENDPOINT_VAR}` environment variable is not set. Set it to the \
                         Jev endpoint URL for the `http` client."
                    )
                })?;
                let client = HttpClient::new(endpoint, config);
                Ok(runtime.block_on(bench::run_benchmark(&client, fixtures, config)))
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
