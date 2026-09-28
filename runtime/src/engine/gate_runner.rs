//! Bounded subprocess execution for quality gates (ADR-1, design §5).
//!
//! `run_gate` runs a project verify or test command and reports whether the gate passed.
//! The executor bounds the subprocess with a wall-clock timeout and hard kill, a working
//! directory pinned under the repository root, a first-token command allowlist, and an
//! exact-name inherited environment allowlist.
//!
//! The gate passes only on a real exit-0 observation by the server (Property 2). A
//! non-zero exit, a timeout, or an allowlist miss always yields an error with remediation
//! hints. Command execution sits behind the [`CommandRunner`] trait, so policy logic is
//! testable with a fake runner and only the real runner touches processes.
//!
//! # Trust model
//!
//! `truenorth_verify_gate` reads its command from trusted operator configuration and accepts
//! only a lifecycle phase from the MCP caller. `truenorth_tdd_cycle` also uses this executor
//! for its caller-provided red-stage command. Environment isolation applies to both paths.
//!
//! Commands run through `/bin/sh -c`, so shell features work and the first-token allowlist
//! does not inspect later commands. The executor does not restrict filesystem or network
//! access and is not a containment boundary for hostile commands. Environment values are
//! inherited only by exact configured name and are redacted from captured standard error.
//!
//! Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6. Design: Part II §5.

use std::ffi::OsString;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

use crate::config::GateExecutionConfig;

/// The maximum stderr tail returned on a gate failure (Requirement 3.3).
const MAX_STDERR_TAIL_BYTES: usize = 2 * 1024;

/// The structured outcome class retained for tool receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcomeStatus {
    Pass,
    Failure,
    Timeout,
}

/// The outcome of a gate run (design §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    pub status: GateOutcomeStatus,
    /// The process exit code when one was observed.
    pub exit_code: Option<i32>,
    /// Monotonic elapsed wall-clock time, rounded down to milliseconds.
    pub duration_ms: u64,
    /// The error message on failure, or `None` on a pass.
    pub error: Option<String>,
    /// Actionable remediation hints on failure.
    pub remediation_hints: Vec<String>,
}

impl GateOutcome {
    pub fn passed(&self) -> bool {
        self.status == GateOutcomeStatus::Pass
    }

    fn pass(exit_code: i32, duration_ms: u64) -> Self {
        Self {
            status: GateOutcomeStatus::Pass,
            exit_code: Some(exit_code),
            duration_ms,
            error: None,
            remediation_hints: Vec::new(),
        }
    }

    fn failed(
        status: GateOutcomeStatus,
        exit_code: Option<i32>,
        duration_ms: u64,
        error: impl Into<String>,
        hints: &[&str],
    ) -> Self {
        Self {
            status,
            exit_code,
            duration_ms,
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
/// The trait lets bounded execution run against a fake in tests, so the command allowlist,
/// stderr-tail, and outcome behavior are testable without spawning a process.
pub trait CommandRunner {
    /// Run `command` with the bounded execution config, returning the observed result.
    ///
    /// # Errors
    ///
    /// Returns a spawn error when the process cannot start.
    fn run(&self, command: &str, cfg: &GateExecutionConfig) -> std::io::Result<CommandResult>;
}

/// Run a gate command through the bounded executor (design §5).
///
/// The steps are: reject when the first token is not in the command allowlist without
/// executing (Requirement 3.5), then run the command and map the result. The gate passes
/// only on exit code 0 within the timeout (Requirement 3.2). A non-zero exit returns the
/// stderr tail (Requirement 3.3). A timeout returns a timeout error (Requirement 3.4).
///
/// # Example
///
/// ```ignore
/// let outcome = gate_runner::run_gate("cargo test", &cfg, &runner);
/// ```
pub fn run_gate<R: CommandRunner>(
    command: &str,
    cfg: &GateExecutionConfig,
    runner: &R,
) -> GateOutcome {
    let started = Instant::now();
    let Some(binary) = first_token(command) else {
        return GateOutcome::failed(
            GateOutcomeStatus::Failure,
            None,
            elapsed_ms(started),
            "the gate command is empty",
            &["supply a non-empty verify or test command"],
        );
    };

    if !cfg
        .command_allowlist
        .iter()
        .any(|allowed| allowed == binary)
    {
        return GateOutcome::failed(
            GateOutcomeStatus::Failure,
            None,
            elapsed_ms(started),
            format!("command `{binary}` is not in the allowlist"),
            &["add the binary to the gate command allowlist"],
        );
    }

    match runner.run(command, cfg) {
        Ok(result) => map_result(result, cfg.timeout, elapsed_ms(started)),
        Err(error) => GateOutcome::failed(
            GateOutcomeStatus::Failure,
            None,
            elapsed_ms(started),
            format!("could not start the gate command `{binary}`: {error}"),
            &["make sure that the binary is installed and on PATH"],
        ),
    }
}

/// Map a command result to a typed gate outcome.
fn map_result(result: CommandResult, timeout: Duration, duration_ms: u64) -> GateOutcome {
    if result.timed_out {
        return GateOutcome::failed(
            GateOutcomeStatus::Timeout,
            None,
            duration_ms,
            format!("the gate timed out after {} seconds", timeout.as_secs()),
            &[
                "reduce the test scope so the gate finishes within the timeout",
                "raise the bounded executor timeout for this gate",
            ],
        );
    }

    if result.exit_code == Some(0) {
        return GateOutcome::pass(0, duration_ms);
    }

    let tail = stderr_tail(&result.stderr);
    let code = result
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown (killed by signal)".to_string());
    GateOutcome::failed(
        GateOutcomeStatus::Failure,
        result.exit_code,
        duration_ms,
        format!("the gate command exited with code {code}:\n{tail}"),
        &[
            "read the stderr tail above and fix the reported failure",
            "re-run the gate after the fix",
        ],
    )
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
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
/// directory under the repository root, inherits only exact allowed environment names,
/// redacts their values from stderr, and enforces the wall-clock timeout with a hard kill
/// (Requirements 3.4, 3.6).
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, command: &str, cfg: &GateExecutionConfig) -> std::io::Result<CommandResult> {
        let environment = selected_environment(&cfg.environment_allowlist);
        let mut builder = Command::new("/bin/sh");
        builder
            .arg("-c")
            .arg(command)
            .current_dir(&cfg.working_dir)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (name, value) in &environment {
            builder.env(name, value);
        }

        // On Unix, run the command in its own process group (pgid = child pid). A timeout
        // then signals the whole group, so a backgrounded descendant cannot outlive the
        // kill (#184). Windows is out of scope for the binary, so it keeps the default.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            builder.process_group(0);
        }

        let mut child = builder.spawn()?;

        // Drain stderr on a thread while waiting. A command that writes more than the
        // pipe buffer would otherwise block before exit and trip a false timeout.
        let stderr_reader = spawn_stderr_reader(&mut child);

        // Wait with the wall-clock timeout. A `None` status means the timeout fired.
        let status = child.wait_timeout(cfg.timeout)?;
        match status {
            Some(status) => {
                let stderr = stderr_reader
                    .map(join_stderr)
                    .map(|stderr| redact_environment_values(stderr, &environment))
                    .unwrap_or_default();
                Ok(CommandResult {
                    exit_code: status.code(),
                    stderr,
                    timed_out: false,
                })
            }
            None => {
                // Requirement 3.4 and #184: hard-kill the whole process group on timeout,
                // so a backgrounded descendant does not outlive the direct child.
                hard_kill(&mut child);
                let _ = child.wait();
                // Do not join the stderr reader here. On timeout the stderr tail is
                // discarded anyway, and a surviving descendant that inherited the pipe
                // would block the join. Detaching keeps `run` prompt on timeout even when
                // a descendant lingers. The group kill above normally closes the pipe, so
                // the detached thread ends on its own.
                drop(stderr_reader);
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

/// Hard-kill a timed-out gate child and its descendants.
///
/// On Unix the child ran in its own process group (see `SystemCommandRunner::run`), so
/// this signals the whole group with `SIGKILL`. A backgrounded descendant is in the same
/// group, so it dies too (#184). A group kill that fails (the group is already gone)
/// falls back to killing the direct child.
///
/// On a non-Unix target this kills the direct child only, matching the prior behavior.
#[cfg(unix)]
fn hard_kill(child: &mut std::process::Child) {
    // The group id equals the child pid, because the child leads its own group. Signal
    // the negative pid to reach every process in the group.
    let pgid = child.id() as libc::pid_t;
    // SAFETY: `kill` with a negative pid signals the process group. A stale group id at
    // worst returns ESRCH, which is harmless. The child is reaped by the caller's `wait`.
    let group_killed = unsafe { libc::kill(-pgid, libc::SIGKILL) } == 0;
    if !group_killed {
        // The group is already gone or could not be signaled. Kill the direct child.
        let _ = child.kill();
    }
}

/// Hard-kill a timed-out gate child (non-Unix fallback: the direct child only).
#[cfg(not(unix))]
fn hard_kill(child: &mut std::process::Child) {
    let _ = child.kill();
}

/// Select only environment entries named by the validated allowlist.
fn selected_environment(names: &[String]) -> Vec<(String, OsString)> {
    names
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (name.clone(), value)))
        .collect()
}

/// Remove exact inherited environment values from captured stderr.
fn redact_environment_values(mut stderr: Vec<u8>, environment: &[(String, OsString)]) -> Vec<u8> {
    let mut values: Vec<Vec<u8>> = environment
        .iter()
        .map(|(_, value)| environment_value_bytes(value))
        .filter(|value| !value.is_empty())
        .collect();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();

    for value in values {
        stderr = replace_bytes(stderr, &value, b"[REDACTED_ENV]");
    }
    stderr
}

#[cfg(unix)]
fn environment_value_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn environment_value_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    value.to_string_lossy().into_owned().into_bytes()
}

fn replace_bytes(input: Vec<u8>, needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() || !input.windows(needle.len()).any(|window| window == needle) {
        return input;
    }

    let mut output = Vec::with_capacity(input.len());
    let mut offset = 0;
    while offset < input.len() {
        if input[offset..].starts_with(needle) {
            output.extend_from_slice(replacement);
            offset += needle.len();
        } else {
            output.push(input[offset]);
            offset += 1;
        }
    }
    output
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `gate_runner`.
#[cfg(test)]
#[path = "gate_runner_tests.rs"]
mod tests;
