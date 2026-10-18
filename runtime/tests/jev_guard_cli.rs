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

#[test]
fn git_diff_guards_validate_metadata_without_reading_private_named_files() {
    let repo = tempfile::tempdir().expect("repository");
    for marker in [".agent", "specs", "skills", "src"] {
        std::fs::create_dir(repo.path().join(marker)).expect("root marker");
    }
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Guard Test",
                "-c",
                "user.email=guard@example.invalid",
            ])
            .args(args)
            .current_dir(repo.path())
            .output()
            .expect("git command");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let guard = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
            .arg("guard")
            .args(args)
            .current_dir(repo.path())
            .env_clear()
            .env("TRUENORTH_ROOT", repo.path())
            .output()
            .expect("Git diff guard")
    };
    git(&["init", "-q"]);
    git(&["commit", "--allow-empty", "-qm", "test: baseline"]);
    std::fs::write(repo.path().join("src/secret_filter.rs"), [0xff, 0xfe])
        .expect("non-text source");
    git(&["add", "src/secret_filter.rs"]);
    assert!(guard(&["--staged"]).status.success());
    git(&["commit", "-qm", "test: source change"]);
    assert!(guard(&["--range", "HEAD~1..HEAD"]).status.success());

    std::fs::create_dir(repo.path().join(".agent/config")).expect("config");
    std::fs::write(
        repo.path().join(".agent/config/rules.yml"),
        "protected_paths:\n  - src/\n",
    )
    .expect("protected paths");
    assert!(!guard(&["--range", "HEAD~1..HEAD"]).status.success());

    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("canary"), b"outside bytes").expect("canary");
    std::os::unix::fs::symlink(outside.path().join("canary"), repo.path().join("linked.rs"))
        .expect("symlink");
    git(&["add", "linked.rs"]);
    assert!(!guard(&["--staged"]).status.success());
    assert_eq!(
        std::fs::read(outside.path().join("canary")).expect("canary"),
        b"outside bytes"
    );
}

#[test]
fn stdin_guard_inspects_proposed_content_without_writing_the_target() {
    let repo = tempfile::tempdir().expect("repository");
    for marker in [".agent", "specs", "skills"] {
        std::fs::create_dir(repo.path().join(marker)).expect("root marker");
    }
    let original = "fn main() {}\n";
    std::fs::write(repo.path().join("main.rs"), original).expect("source");
    for (content, allowed) in [
        ("fn main() { let value = 1; }", true),
        ("// credentials placeholder only", false),
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
            .expect("stdin guard");
        let input = serde_json::json!({
            "paths": ["main.rs"],
            "content": content,
            "task": "Update the entry point",
        });
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.to_string().as_bytes())
            .expect("proposal");
        let output = child.wait_with_output().expect("decision");
        assert_eq!(output.status.success(), allowed, "{output:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            std::fs::read_to_string(repo.path().join("main.rs")).expect("source"),
            original
        );
    }
}

#[tokio::test]
async fn generated_hook_command_blocks_a_normalized_replacement_proposal() {
    use rmcp::handler::server::{tool::RequestId, wrapper::Parameters};
    use truenorth_mcp::server::TrueNorthServer;
    use truenorth_mcp::tools::scaffold::ScaffoldArgs;

    let repo = tempfile::tempdir().expect("repository");
    let server = TrueNorthServer::resolve(repo.path().to_path_buf()).expect("server");
    server
        .truenorth_scaffold_project(
            Parameters(ScaffoldArgs { profile: None }),
            RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect("scaffold");
    let hook: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repo.path().join(".kiro/hooks/jev-guard.json")).expect("hook"),
    )
    .expect("hook JSON");
    let command = hook["hooks"][0]["action"]["command"]
        .as_str()
        .expect("command");
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_truenorth-mcp"));
    let mut child = Command::new("/bin/sh")
        .args(["-c", command])
        .current_dir(repo.path())
        .env_clear()
        .env("PATH", binary.parent().expect("binary directory"))
        .env("TRUENORTH_ROOT", repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("emitted hook command");
    let input = serde_json::json!({
        "paths": ["new.rs"],
        "content": "// credentials placeholder only",
        "task": "Replace a function",
    });
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.to_string().as_bytes())
        .expect("normalized proposal");
    let output = child.wait_with_output().expect("hook decision");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(!repo.path().join("new.rs").exists());
}

#[test]
fn foreign_hook_payload_is_rejected_without_echoing_its_content() {
    let repo = tempfile::tempdir().expect("repository");
    for marker in [".agent", "specs", "skills"] {
        std::fs::create_dir(repo.path().join(marker)).expect("root marker");
    }
    let sentinel = "inert-private-payload-sentinel";
    let mut child = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
        .arg("guard")
        .current_dir(repo.path())
        .env_clear()
        .env("TRUENORTH_ROOT", repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("guard");
    let input = serde_json::json!({
        "tool_name": "str_replace",
        "tool_input": {"path": "new.rs", "old_str": "", "new_str": sentinel},
    });
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.to_string().as_bytes())
        .expect("foreign payload");
    let output = child.wait_with_output().expect("rejection");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(sentinel));
    assert!(!repo.path().join("new.rs").exists());
}

#[test]
fn stdin_guard_rejects_oversized_utf8_content_before_activation() {
    let repo = tempfile::tempdir().expect("repository");
    std::fs::create_dir_all(repo.path().join(".agent/config")).expect("config");
    for marker in ["specs", "skills"] {
        std::fs::create_dir(repo.path().join(marker)).expect("root marker");
    }
    std::fs::write(
        repo.path().join(".agent/config/rules.yml"),
        "features:\n  jev: true\n",
    )
    .expect("opt in without credentials");
    let mut child = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
        .arg("guard")
        .current_dir(repo.path())
        .env_clear()
        .env("TRUENORTH_ROOT", repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("guard");
    let input = serde_json::json!({
        "paths": ["new.rs"],
        "content": "é".repeat(500_001),
    });
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.to_string().as_bytes())
        .expect("oversized proposal");
    let output = child.wait_with_output().expect("rejection");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("content"), "{diagnostic}");
    assert!(
        !diagnostic.contains("TRUENORTH_JEV_ENDPOINT"),
        "{diagnostic}"
    );
    assert!(!repo.path().join("new.rs").exists());
}
