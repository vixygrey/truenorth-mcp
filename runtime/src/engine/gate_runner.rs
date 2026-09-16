//! Sandboxed subprocess execution for quality gates (ADR-1, design §5).
//!
//! `run_gate` runs a project verify or test command under a sandbox and reports whether
//! the gate passed. The sandbox bounds the subprocess with a wall-clock timeout and a
//! hard kill, a working directory pinned under the repository root, an allowlist on the
//! command's first token, and an environment sanitized of secret values.
//!
//! The gate passes only on a real exit-0 observation by the server (Property 2). A
//! non-zero exit, a timeout, or an allowlist miss always yields an error with remediation
//! hints. Command execution sits behind the [`CommandRunner`] trait, so the sandbox logic
//! is testable with a fake runner and only the real runner touches processes.
//!
//! Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6. Design: Part II §5.

use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::config::{SandboxConfig, secret_denylist};

/// The maximum stderr tail returned on a gate failure (Requirement 3.3).
const MAX_STDERR_TAIL_BYTES: usize = 2 * 1024;

/// The outcome of a gate run (design §5).
///
/// A pass carries `passed = true` and no error. A failure carries `passed = false`, an
/// error message, and remediation hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    /// Whether the gate passed.
    pub passed: bool,
    /// The error message on failure, or `None` on a pass.
    pub error: Option<String>,
    /// Actionable remediation hints on failure.
    pub remediation_hints: Vec<String>,
}

impl GateOutcome {
    /// A passing outcome.
    fn passed() -> Self {
        Self {
            passed: true,
            error: None,
            remediation_hints: Vec::new(),
        }
    }

    /// A failing outcome with a message and hints.
    fn failed(error: impl Into<String>, hints: &[&str]) -> Self {
        Self {
            passed: false,
            error: Some(error.into()),
            remediation_hints: hints.iter().map(|hint| hint.to_string()).collect(),
        }
    }
}

/// The result of running a command, produced by a [`CommandRunner`].
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// The process exit code, or `None` when the process was killed without one.
    pub exit_code: Option<i32>,
    /// The captured standard-error output.
    pub stderr: Vec<u8>,
    /// Whether the wall-clock timeout fired and the process was hard-killed.
    pub timed_out: bool,
}

/// A command execution backend.
///
/// The trait lets the sandbox logic run against a fake in tests, so the allowlist,
/// evidence, stderr-tail, and hint behavior is testable without spawning a process.
pub trait CommandRunner {
    /// Run `command` with the sandbox config, returning the observed result.
    ///
    /// # Errors
    ///
    /// Returns a spawn error when the process cannot start.
    fn run(&self, command: &str, cfg: &SandboxConfig) -> std::io::Result<CommandResult>;
}

/// Run a gate command under the sandbox (design §5).
///
/// The steps are: reject when execution is disabled (evidence-only, Requirement 3.1),
/// reject when the first token is not in the allowlist without executing
/// (Requirement 3.5), then run the command and map the result. The gate passes only on
/// exit code 0 within the timeout (Requirement 3.2). A non-zero exit returns the stderr
/// tail (Requirement 3.3). A timeout returns a timeout error (Requirement 3.4).
///
/// # Example
///
/// ```ignore
/// let outcome = gate_runner::run_gate("cargo test", &cfg, &runner);
/// ```
pub fn run_gate<R: CommandRunner>(command: &str, cfg: &SandboxConfig, runner: &R) -> GateOutcome {
    if !cfg.execution_enabled {
        return GateOutcome::failed(
            "gate execution is disabled; supply test evidence in evidence mode",
            &[
                "run the gate with mode=execute, or call with mode=evidence and supply test_evidence",
            ],
        );
    }

    let Some(binary) = first_token(command) else {
        return GateOutcome::failed(
            "the gate command is empty",
            &["supply a non-empty verify or test command"],
        );
    };

    if !cfg.allowlist.iter().any(|allowed| allowed == binary) {
        // Requirement 3.5: do not execute a command whose binary is not allowlisted.
        return GateOutcome::failed(
            format!("command `{binary}` is not in the allowlist"),
            &["add the binary to the sandbox allowlist, or use mode=evidence"],
        );
    }

    match runner.run(command, cfg) {
        Ok(result) => map_result(result, cfg.timeout),
        Err(error) => GateOutcome::failed(
            format!("could not start the gate command `{binary}`: {error}"),
            &["make sure that the binary is installed and on PATH"],
        ),
    }
}

/// Map a command result to a gate outcome.
fn map_result(result: CommandResult, timeout: Duration) -> GateOutcome {
    if result.timed_out {
        // Requirement 3.4.
        return GateOutcome::failed(
            format!("the gate timed out after {} seconds", timeout.as_secs()),
            &[
                "reduce the test scope so the gate finishes within the timeout",
                "raise the sandbox timeout for this gate",
            ],
        );
    }

    if result.exit_code == Some(0) {
        // Requirement 3.2: pass only on a real exit-0 observation.
        return GateOutcome::passed();
    }

    // Requirement 3.3: non-zero exit returns at most the final 2 KB of stderr.
    let tail = stderr_tail(&result.stderr);
    let code = result
        .exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "unknown (killed by signal)".to_string());
    GateOutcome::failed(
        format!("the gate command exited with code {code}:\n{tail}"),
        &[
            "read the stderr tail above and fix the reported failure",
            "re-run the gate after the fix",
        ],
    )
}

/// The first whitespace-separated token of a command, when present.
fn first_token(command: &str) -> Option<&str> {
    command.split_whitespace().next()
}

/// Return at most the final [`MAX_STDERR_TAIL_BYTES`] of stderr as a lossy string.
///
/// The cut lands on a UTF-8 boundary, so the tail is always valid text.
fn stderr_tail(stderr: &[u8]) -> String {
    let start = stderr.len().saturating_sub(MAX_STDERR_TAIL_BYTES);
    String::from_utf8_lossy(&stderr[start..]).into_owned()
}

/// The real command runner. It spawns the command with `std::process`, pins the working
/// directory under the repository root, sanitizes the environment, and enforces the
/// wall-clock timeout with a hard kill (Requirements 3.4, 3.6).
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, command: &str, cfg: &SandboxConfig) -> std::io::Result<CommandResult> {
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(&cfg.working_dir)
            .env_clear()
            .envs(sanitized_env())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        // Drain stderr on a thread while waiting. A command that writes more than the
        // pipe buffer would otherwise block before exit and trip a false timeout.
        let stderr_reader = spawn_stderr_reader(&mut child);

        // Wait with the wall-clock timeout. A `None` status means the timeout fired.
        let status = child.wait_timeout(cfg.timeout)?;
        match status {
            Some(status) => {
                let stderr = stderr_reader.map(join_stderr).unwrap_or_default();
                Ok(CommandResult {
                    exit_code: status.code(),
                    stderr,
                    timed_out: false,
                })
            }
            None => {
                // Requirement 3.4: hard-kill on timeout.
                child.kill()?;
                let _ = child.wait();
                // The reader thread ends once the killed process closes its pipe.
                drop(stderr_reader.map(join_stderr));
                Ok(CommandResult {
                    exit_code: None,
                    stderr: Vec::new(),
                    timed_out: true,
                })
            }
        }
    }
}

/// Spawn a thread that reads the child's stderr to end, so a full pipe never blocks the
/// child before exit. Returns `None` when the child has no stderr pipe.
fn spawn_stderr_reader(
    child: &mut std::process::Child,
) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    use std::io::Read;
    let mut stderr = child.stderr.take()?;
    Some(std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        buffer
    }))
}

/// Join a stderr reader thread, returning its buffer or an empty one on a join failure.
fn join_stderr(handle: std::thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    handle.join().unwrap_or_default()
}

/// The current environment with secret-matching values dropped (Requirement 3.6).
///
/// A variable whose value or name matches the secret denylist is excluded, so a token or
/// credential never reaches the gate subprocess.
fn sanitized_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(key, value)| !is_secret_env(key, value))
        .collect()
}

/// Report whether an environment entry matches the secret denylist by name or value.
fn is_secret_env(key: &str, value: &str) -> bool {
    secret_denylist()
        .iter()
        .any(|pattern| pattern.is_match(key) || pattern.is_match(value))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `gate_runner`.
#[cfg(test)]
#[path = "gate_runner_tests.rs"]
mod tests;
