//! Run environment-backed scenarios without mutating the parallel test process.

use std::ffi::OsString;
use std::process::Command;

pub(crate) fn in_child(name: &str, environment: &[(&str, OsString)]) -> bool {
    const CHILD_ENV: &str = "TRUENORTH_ISOLATED_TEST";
    if std::env::var(CHILD_ENV).as_deref() == Ok(name) {
        return true;
    }
    let mut command = Command::new(std::env::current_exe().expect("test executable"));
    command
        .args(["--exact", name, "--nocapture"])
        .env_clear()
        .env(CHILD_ENV, name);
    for key in ["PATH", "LLVM_PROFILE_FILE"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command.output().expect("run isolated test");
    assert!(
        output.status.success(),
        "isolated {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}
