//! Process-level regressions for unsafe paths in both guard CLI entry points.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
#[cfg(feature = "jev-http")]
fn invalid_guard_path_exits_nonzero_before_http() {
    let repo = tempfile::tempdir().expect("repository");
    for marker in [".agent", "specs", "skills"] {
        std::fs::create_dir(repo.path().join(marker)).expect("root marker");
    }

    for unsafe_path in ["../outside.rs", "/etc/passwd", ".env"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_jev-bench"))
            .arg("guard")
            .current_dir(repo.path())
            .env_clear()
            .env("TRUENORTH_ROOT", repo.path())
            .env("TRUENORTH_JEV_ENDPOINT", "https://example.invalid/jev")
            .env("TRUENORTH_JEV_API_KEY", "fake-test-key-only")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn guard CLI");
        let input = serde_json::json!({
            "paths": ["src/good.rs", unsafe_path],
            "content": "clean",
        });
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.to_string().as_bytes())
            .expect("write proposal");
        let output = child.wait_with_output().expect("guard result");
        assert!(
            !output.status.success(),
            "unsafe guard path must exit nonzero"
        );
        let stderr = String::from_utf8(output.stderr).expect("diagnostic UTF-8");
        assert!(stderr.contains(unsafe_path), "{stderr}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn runtime_guard_rejects_unsafe_paths_and_accepts_valid_proposals() {
    let repo = tempfile::tempdir().expect("repository");
    let outside = tempfile::tempdir().expect("outside");
    for marker in [".agent", "specs", "skills"] {
        std::fs::create_dir(repo.path().join(marker)).expect("root marker");
    }
    std::fs::write(repo.path().join("good.rs"), "safe source").expect("source");
    std::fs::write(outside.path().join("canary"), "outside bytes").expect("canary");
    std::os::unix::fs::symlink(outside.path().join("canary"), repo.path().join("linked.rs"))
        .expect("symlink");
    for (path, allowed) in [
        ("../outside.rs", false),
        ("/etc/passwd", false),
        (".env", false),
        (".agent/telemetry/log.json", false),
        ("linked.rs", false),
        ("good.rs", true),
        ("new/directory/source.rs", true),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
            .arg("guard")
            .current_dir(repo.path())
            .env_clear()
            .env("TRUENORTH_ROOT", repo.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("runtime guard");
        let input = serde_json::json!({"paths": ["good.rs", path], "content": "safe"});
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.to_string().as_bytes())
            .expect("proposal");
        let output = child.wait_with_output().expect("guard result");
        assert_eq!(
            output.status.success(),
            allowed,
            "{path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
    assert_eq!(
        std::fs::read(outside.path().join("canary")).expect("canary"),
        b"outside bytes"
    );
}
