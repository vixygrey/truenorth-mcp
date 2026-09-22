//! Read-only command-line diagnostics for the native runtime.
//!
//! The MCP server owns standard output when it runs normally. The explicit CLI modes in
//! this module return before transport or watcher startup, so their JSON output is safe for
//! shell use and cannot mutate the governed workspace.

use std::path::PathBuf;
use std::process::ExitCode;

use serde::Serialize;

use truenorth_mcp::config;
use truenorth_mcp::engine::agent_ws::read_layout;
use truenorth_mcp::engine::features;

const USAGE: &str = "usage: truenorth-mcp [--version | --check-config]";

/// The operation requested by the process arguments.
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// Start the stdio MCP server.
    Serve,
    /// Print the compiled package version.
    Version,
    /// Report the read-only configuration diagnostics.
    CheckConfig,
}

/// Parse the supported command-line modes.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Mode, String> {
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(Mode::Serve),
        [flag] if flag == "--version" => Ok(Mode::Version),
        [flag] if flag == "--check-config" => Ok(Mode::CheckConfig),
        _ => Err(USAGE.to_string()),
    }
}

/// Print the package version without reading the workspace.
pub fn print_version() {
    println!("{}", env!("CARGO_PKG_VERSION"));
}

/// Inspect the configuration without starting the server or running a gate command.
pub fn check_config() -> ExitCode {
    let report = DiagnosticReport::collect();
    println!(
        "{}",
        serde_json::to_string(&report).expect("diagnostic report serializes")
    );

    if report.ready() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[derive(Debug, Serialize)]
struct DiagnosticReport {
    package: PackageReport,
    platform: PlatformReport,
    repository_root: RepositoryRootReport,
    layout: CheckReport,
    features: FeaturesReport,
    verify_gate: VerifyGateReport,
}

impl DiagnosticReport {
    fn collect() -> Self {
        let root = config::get_repo_root();
        let verify_gate = VerifyGateReport::collect();

        match root {
            Ok(path) => Self {
                package: PackageReport::current(),
                platform: PlatformReport::current(),
                repository_root: RepositoryRootReport::ok(path.clone()),
                layout: layout_report(&path),
                features: features_report(&path),
                verify_gate,
            },
            Err(error) => Self {
                package: PackageReport::current(),
                platform: PlatformReport::current(),
                repository_root: RepositoryRootReport::error(error.to_string()),
                layout: CheckReport::skipped("repository root could not be resolved"),
                features: FeaturesReport::skipped("repository root could not be resolved"),
                verify_gate,
            },
        }
    }

    fn ready(&self) -> bool {
        self.repository_root.status == Status::Ok
            && self.layout.status == Status::Ok
            && self.features.status == Status::Ok
            && self.verify_gate.status == Status::Ok
    }
}

#[derive(Debug, Serialize)]
struct PackageReport {
    name: &'static str,
    version: &'static str,
}

impl PackageReport {
    fn current() -> Self {
        Self {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

#[derive(Debug, Serialize)]
struct PlatformReport {
    os: &'static str,
    arch: &'static str,
    family: &'static str,
}

impl PlatformReport {
    fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            family: std::env::consts::FAMILY,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Ok,
    Error,
    Skipped,
}

#[derive(Debug, Serialize)]
struct RepositoryRootReport {
    status: Status,
    path: Option<String>,
    error: Option<ErrorReport>,
}

impl RepositoryRootReport {
    fn ok(path: PathBuf) -> Self {
        Self {
            status: Status::Ok,
            path: Some(path.display().to_string()),
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            status: Status::Error,
            path: None,
            error: Some(ErrorReport::new(
                message,
                "Set TRUENORTH_ROOT to the repository root, or run the command from inside the repository.",
            )),
        }
    }
}

#[derive(Debug, Serialize)]
struct CheckReport {
    status: Status,
    error: Option<ErrorReport>,
    reason: Option<&'static str>,
}

impl CheckReport {
    fn ok() -> Self {
        Self {
            status: Status::Ok,
            error: None,
            reason: None,
        }
    }

    fn error(message: String, remediation: &'static str) -> Self {
        Self {
            status: Status::Error,
            error: Some(ErrorReport::new(message, remediation)),
            reason: None,
        }
    }

    fn skipped(reason: &'static str) -> Self {
        Self {
            status: Status::Skipped,
            error: None,
            reason: Some(reason),
        }
    }
}

fn layout_report(root: &std::path::Path) -> CheckReport {
    match read_layout(root) {
        Ok(_) => CheckReport::ok(),
        Err(error) => CheckReport::error(
            error.to_string(),
            "Restore the named .agent layout entry or scaffold the project with truenorth_scaffold_project.",
        ),
    }
}

#[derive(Debug, Serialize)]
struct FeaturesReport {
    status: Status,
    ontology: Option<bool>,
    jev: Option<bool>,
    error: Option<ErrorReport>,
    reason: Option<&'static str>,
}

impl FeaturesReport {
    fn ok(ontology: bool, jev: bool) -> Self {
        Self {
            status: Status::Ok,
            ontology: Some(ontology),
            jev: Some(jev),
            error: None,
            reason: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            status: Status::Error,
            ontology: None,
            jev: None,
            error: Some(ErrorReport::new(
                message,
                "Fix the named .agent/config/rules.yml file.",
            )),
            reason: None,
        }
    }

    fn skipped(reason: &'static str) -> Self {
        Self {
            status: Status::Skipped,
            ontology: None,
            jev: None,
            error: None,
            reason: Some(reason),
        }
    }
}

fn features_report(root: &std::path::Path) -> FeaturesReport {
    match features::resolve(root) {
        Ok(features) => FeaturesReport::ok(features.ontology, features.jev),
        Err(error) => FeaturesReport::error(error.to_string()),
    }
}

#[derive(Debug, Serialize)]
struct VerifyGateReport {
    status: Status,
    configured: bool,
    error: Option<ErrorReport>,
}

impl VerifyGateReport {
    fn collect() -> Self {
        if config::verify_command().is_some() {
            Self {
                status: Status::Ok,
                configured: true,
                error: None,
            }
        } else {
            Self {
                status: Status::Error,
                configured: false,
                error: Some(ErrorReport::new(
                    "no verify command configured".to_string(),
                    "Set TRUENORTH_VERIFY_CMD to the project verify or test command.",
                )),
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorReport {
    message: String,
    remediation: &'static str,
}

impl ErrorReport {
    fn new(message: String, remediation: &'static str) -> Self {
        Self {
            message,
            remediation,
        }
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
