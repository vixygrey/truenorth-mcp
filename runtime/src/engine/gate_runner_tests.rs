//! Tests for the sandboxed gate runner (task 5.2).
//!
//! Included from `gate_runner.rs` via `#[path]`, so `super` is the gate_runner module.
//!
//! Property 2: `run_gate` passes only on a real exit-0 observation. The fake runner
//! covers pass, fail, timeout, and allowlist-reject paths. The real runner covers `cwd`
//! pinning and environment sanitization.
//!
//! Requirements: 3.2, 3.3, 3.4, 3.5, 3.6.

use super::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::time::Duration;

/// A fake command runner that returns a canned result and records whether it ran.
/// The `ran` flag proves an allowlist rejection never executes a command.
struct FakeCommandRunner {
    result: CommandResult,
    ran: Cell<bool>,
}

impl FakeCommandRunner {
    fn new(result: CommandResult) -> Self {
        Self {
            result,
            ran: Cell::new(false),
        }
    }

    /// A runner that returns a clean exit-0 result.
    fn passing() -> Self {
        Self::new(CommandResult {
            exit_code: Some(0),
            stderr: Vec::new(),
            timed_out: false,
        })
    }
}

impl CommandRunner for FakeCommandRunner {
    fn run(&self, _command: &str, _cfg: &SandboxConfig) -> std::io::Result<CommandResult> {
        self.ran.set(true);
        Ok(self.result.clone())
    }
}

/// A sandbox config with `cargo` allowlisted.
fn cfg_with_cargo() -> SandboxConfig {
    SandboxConfig::new(PathBuf::from("/repo"), vec!["cargo".to_string()])
}

#[test]
fn pass_only_on_exit_zero() {
    // Property 2: exit 0 passes.
    let runner = FakeCommandRunner::passing();
    let outcome = run_gate("cargo test", &cfg_with_cargo(), &runner);
    assert!(outcome.passed);
    assert!(outcome.error.is_none());
    assert!(runner.ran.get());
}

#[test]
fn nonzero_exit_fails_with_stderr_tail() {
    // Requirement 3.3: a non-zero exit returns the stderr tail and hints.
    let runner = FakeCommandRunner::new(CommandResult {
        exit_code: Some(101),
        stderr: b"error: test `foo` failed\n".to_vec(),
        timed_out: false,
    });
    let outcome = run_gate("cargo test", &cfg_with_cargo(), &runner);
    assert!(!outcome.passed);
    let error = outcome.error.expect("failure carries an error");
    assert!(error.contains("101"));
    assert!(error.contains("test `foo` failed"));
    assert!(!outcome.remediation_hints.is_empty());
}

#[test]
fn stderr_tail_is_capped_at_2kb() {
    // Requirement 3.3: at most the final 2 KB of stderr.
    let big = vec![b'x'; 10 * 1024];
    let runner = FakeCommandRunner::new(CommandResult {
        exit_code: Some(1),
        stderr: big,
        timed_out: false,
    });
    let outcome = run_gate("cargo test", &cfg_with_cargo(), &runner);
    let error = outcome.error.expect("failure carries an error");
    // The error prefixes the tail after a newline. The tail is the run of `x` bytes and
    // must be capped at 2 KB. The fixed prefix ("...exited...") carries its own letters,
    // so measure the tail as the longest trailing `x` run.
    let tail_x = error.chars().rev().take_while(|&c| c == 'x').count();
    assert_eq!(
        tail_x, MAX_STDERR_TAIL_BYTES,
        "stderr tail must cap at 2 KB"
    );
}

#[test]
fn timeout_fails_with_timeout_hints() {
    // Requirement 3.4: a timeout returns a timeout error with reduce-scope/raise hints.
    let runner = FakeCommandRunner::new(CommandResult {
        exit_code: None,
        stderr: Vec::new(),
        timed_out: true,
    });
    let outcome = run_gate("cargo test", &cfg_with_cargo(), &runner);
    assert!(!outcome.passed);
    let error = outcome.error.expect("failure carries an error");
    assert!(error.contains("timed out"));
    let hints = outcome.remediation_hints.join(" ");
    assert!(hints.contains("reduce the test scope"));
    assert!(hints.contains("raise the sandbox timeout"));
}

#[test]
fn allowlist_miss_rejects_without_executing() {
    // Requirement 3.5: a command not in the allowlist is rejected without execution.
    let runner = FakeCommandRunner::passing();
    let cfg = SandboxConfig::new(PathBuf::from("/repo"), vec!["cargo".to_string()]);
    let outcome = run_gate("rm -rf /", &cfg, &runner);
    assert!(!outcome.passed);
    assert!(
        !runner.ran.get(),
        "an allowlist miss must not execute the command"
    );
    let error = outcome.error.expect("failure carries an error");
    assert!(error.contains("not in the allowlist"));
}

#[test]
fn empty_command_is_rejected() {
    let runner = FakeCommandRunner::passing();
    let outcome = run_gate("   ", &cfg_with_cargo(), &runner);
    assert!(!outcome.passed);
    assert!(!runner.ran.get());
}

// The following tests use the real runner. They shell out to `/bin/sh`, so they run on
// the Unix targets this project supports (Windows is out of scope per Requirement 8.8).

#[test]
fn real_runner_reports_exit_code_and_cwd() {
    // Requirement 3.6: the working directory is pinned under the repo root. The command
    // prints its cwd, which must match the sandbox working_dir.
    let temp = std::env::temp_dir();
    let cfg = SandboxConfig::new(temp.clone(), vec!["pwd".to_string()]);
    let runner = SystemCommandRunner;
    let result = runner.run("pwd 1>&2", &cfg).expect("run pwd");
    assert_eq!(result.exit_code, Some(0));
    // On macOS the temp dir can be a symlink, so compare the canonical forms.
    let reported = String::from_utf8_lossy(&result.stderr);
    let reported_path = std::fs::canonicalize(reported.trim()).expect("canonicalize reported");
    let expected_path = std::fs::canonicalize(&temp).expect("canonicalize temp");
    assert_eq!(reported_path, expected_path);
}

#[test]
fn real_runner_drops_secret_env_values() {
    // Requirement 3.6: an environment value matching the denylist is dropped before the
    // subprocess spawns.
    // SAFETY: the test sets and removes a process-local env var it owns. No other test
    // reads `TRUENORTH_TEST_TOKEN`, so the mutation is isolated.
    unsafe {
        std::env::set_var("TRUENORTH_TEST_TOKEN", "/tmp/my-secret-value.pem");
    }
    let cfg = SandboxConfig::new(std::env::temp_dir(), vec!["printenv".to_string()]);
    let runner = SystemCommandRunner;
    let result = runner
        .run("printenv TRUENORTH_TEST_TOKEN 1>&2", &cfg)
        .expect("run printenv");
    unsafe {
        std::env::remove_var("TRUENORTH_TEST_TOKEN");
    }
    // `printenv` exits non-zero when the variable is absent, which is the point: the
    // secret-valued variable was dropped from the sanitized environment.
    assert_ne!(result.exit_code, Some(0));
}

#[test]
fn real_runner_nonzero_exit_is_observed() {
    // Property 2: the runner observes a real non-zero exit code.
    let cfg = SandboxConfig::new(std::env::temp_dir(), vec!["false".to_string()]);
    let runner = SystemCommandRunner;
    let result = runner.run("false", &cfg).expect("run false");
    assert_eq!(result.exit_code, Some(1));
    assert!(!result.timed_out);
}

#[test]
fn real_runner_hard_kills_on_timeout() {
    // Requirement 3.4: a command that exceeds the timeout is hard-killed.
    let mut cfg = SandboxConfig::new(std::env::temp_dir(), vec!["sleep".to_string()]);
    cfg.timeout = Duration::from_millis(200);
    let runner = SystemCommandRunner;
    let result = runner.run("sleep 30", &cfg).expect("run sleep");
    assert!(result.timed_out, "the long sleep must time out");
    assert_eq!(result.exit_code, None);
}

#[cfg(unix)]
#[test]
fn real_runner_kills_a_backgrounded_descendant_on_timeout() {
    // #184: a timeout kills the whole process group, so a backgrounded descendant does
    // not outlive the direct `/bin/sh` child.
    let dir = tempfile::tempdir().expect("temp dir");
    let pidfile = dir.path().join("descendant.pid");
    let pidfile_arg = pidfile.display().to_string();

    // The subshell records its pid, then both it and the parent sleep past the timeout.
    // The subshell redirects its stdio off the inherited stderr pipe, and the timeout path
    // detaches the stderr reader, so `run` returns promptly. The descendant sleep is
    // bounded, so a descendant that ever escapes the kill self-cleans in seconds.
    //
    // Note: this survival check has teeth on Linux, where a parent-only kill leaves the
    // backgrounded descendant alive. On macOS a parent-only kill already tears the
    // descendant down, so there the check passes either way. The
    // `real_runner_spawns_the_child_in_its_own_process_group` test below covers the
    // mechanism deterministically on every Unix target.
    let command = format!("(echo $$ > '{pidfile_arg}'; exec sleep 10) >/dev/null 2>&1 & sleep 10");
    let mut cfg = SandboxConfig::new(std::env::temp_dir(), vec!["(".to_string()]);
    cfg.timeout = Duration::from_millis(300);
    // The allowlist gates the first token, which is `(` here. Allow it so the command runs.
    let runner = SystemCommandRunner;
    let result = runner.run(&command, &cfg).expect("run backgrounded sleep");
    assert!(result.timed_out, "the command must time out");

    // Read the descendant pid the subshell recorded.
    let pid: i32 = {
        // The subshell writes the pid almost immediately, but allow a brief moment.
        let mut attempts = 0;
        loop {
            if let Ok(text) = std::fs::read_to_string(&pidfile)
                && let Ok(pid) = text.trim().parse::<i32>()
            {
                break pid;
            }
            attempts += 1;
            assert!(attempts < 50, "the descendant never recorded its pid");
            std::thread::sleep(Duration::from_millis(20));
        }
    };

    // After the group kill, the descendant must be gone. Poll `kill(pid, 0)`, which
    // returns an error once the process no longer exists.
    let mut attempts = 0;
    let alive = loop {
        // SAFETY: signal 0 performs no kill; it only checks whether the pid is live.
        let live = unsafe { libc::kill(pid, 0) } == 0;
        if !live {
            break false;
        }
        attempts += 1;
        if attempts >= 100 {
            break true;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        !alive,
        "the backgrounded descendant (pid {pid}) outlived the group kill"
    );
}

#[cfg(unix)]
#[test]
fn real_runner_spawns_the_child_in_its_own_process_group() {
    // #184: the child leads its own process group (pgid == child pid). This is the
    // mechanism the timeout group-kill relies on, so assert it directly. This is
    // deterministic on every Unix target, unlike the descendant-survival check above.
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("pgid.txt");
    let out_arg = out.display().to_string();

    // `ps -o pgid= -p $$` prints this shell's process-group id, then the shell prints its
    // own pid. A `/bin/sh` that leads its own group prints equal values.
    let command = format!("ps -o pgid= -p $$ > '{out_arg}'; echo $$ >> '{out_arg}'");
    let cfg = SandboxConfig::new(std::env::temp_dir(), vec!["ps".to_string()]);
    let runner = SystemCommandRunner;
    let result = runner.run(&command, &cfg).expect("run ps");
    assert_eq!(result.exit_code, Some(0), "the ps command must succeed");

    let text = std::fs::read_to_string(&out).expect("read pgid output");
    let mut lines = text.lines();
    let pgid: i32 = lines
        .next()
        .expect("pgid line")
        .trim()
        .parse()
        .expect("parse pgid");
    let pid: i32 = lines
        .next()
        .expect("pid line")
        .trim()
        .parse()
        .expect("parse pid");
    assert_eq!(
        pgid, pid,
        "the child must lead its own process group (pgid == pid)"
    );
}
