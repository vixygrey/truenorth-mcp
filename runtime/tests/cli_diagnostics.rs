//! Process-level regression tests for the read-only diagnostic commands.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"));
    command.env_clear().current_dir(root);
    command
}

fn run(root: &Path, args: &[&str], verify_command: Option<&str>) -> Output {
    let mut command = command(root);
    command.args(args);
    if let Some(verify_command) = verify_command {
        command.env("TRUENORTH_VERIFY_CMD", verify_command);
    }
    command.output().expect("run truenorth-mcp")
}

fn parse_report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("diagnostic output is JSON")
}

fn seed_complete_repo() -> TempDir {
    let repo = tempfile::tempdir().expect("create repository");
    fs::create_dir_all(repo.path().join("specs")).expect("create specs");
    fs::create_dir_all(repo.path().join("skills")).expect("create skills");

    let agent = repo.path().join(".agent");
    for directory in ["config", "spec", "tasks", "memories", "telemetry"] {
        fs::create_dir_all(agent.join(directory)).expect("create layout directory");
    }
    fs::write(agent.join("layout.yml"), "version: \"1\"\n").expect("write layout");
    fs::write(agent.join("profile.yml"), "profile: issue-per-task\n").expect("write profile");
    fs::write(
        agent.join("config/rules.yml"),
        "features:\n  ontology: false\n",
    )
    .expect("write rules");
    fs::write(agent.join("spec/requirements.md"), "# Requirements\n").expect("write requirements");
    fs::write(agent.join("tasks/state.yml"), "phase: discover\n").expect("write state");
    fs::write(agent.join("memories/lessons.md"), "# Lessons\n").expect("write lessons");
    fs::write(agent.join("memories/glossary.md"), "# Glossary\n").expect("write glossary");
    fs::write(agent.join("telemetry/runs.yml"), "runs: []\n").expect("write telemetry");

    repo
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    snapshot_directory(root, root, &mut entries);
    entries
}

fn snapshot_directory(root: &Path, directory: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in fs::read_dir(directory).expect("read workspace") {
        let entry = entry.expect("read workspace entry");
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("relative workspace path")
            .to_path_buf();
        if path.is_dir() {
            entries.insert(relative.clone(), b"directory".to_vec());
            snapshot_directory(root, &path, entries);
        } else {
            entries.insert(relative, fs::read(path).expect("read workspace file"));
        }
    }
}

#[test]
fn version_needs_no_repository() {
    let directory = tempfile::tempdir().expect("create working directory");
    let output = run(directory.path(), &["--version"], None);

    assert!(output.status.success(), "version command succeeds");
    assert_eq!(
        String::from_utf8(output.stdout).expect("version output is text"),
        format!("{}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(
        output.stderr.is_empty(),
        "version command emits no diagnostics"
    );
}

#[test]
fn complete_configuration_reports_ready_without_mutation() {
    let repo = seed_complete_repo();
    let before = snapshot(repo.path());

    let output = run(repo.path(), &["--check-config"], Some("cargo test"));
    let report = parse_report(&output);

    assert!(output.status.success(), "valid configuration succeeds");
    assert_eq!(report["repository_root"]["status"], "ok");
    assert_eq!(report["layout"]["status"], "ok");
    assert_eq!(report["backlog"]["status"], "ok");
    assert_eq!(report["features"]["status"], "ok");
    assert_eq!(report["features"]["ontology"], false);
    assert_eq!(report["token_caps"]["status"], "ok");
    assert_eq!(report["token_caps"]["skill_lean_tokens"], 1500);
    assert_eq!(report["token_caps"]["tool_payload_tokens"], 4000);
    assert_eq!(report["token_caps"]["estimation"], "ceil(characters / 4)");
    assert_eq!(report["verify_gate"]["status"], "ok");
    assert_eq!(report["verify_gate"]["configured"], true);
    assert_eq!(
        snapshot(repo.path()),
        before,
        "diagnostics leave the workspace unchanged"
    );
}

#[test]
fn missing_root_reports_actionable_remediation() {
    let directory = tempfile::tempdir().expect("create working directory");
    let output = run(directory.path(), &["--check-config"], Some("cargo test"));
    let report = parse_report(&output);

    assert!(!output.status.success(), "missing root fails");
    assert_eq!(report["repository_root"]["status"], "error");
    assert!(
        report["repository_root"]["error"]["remediation"]
            .as_str()
            .expect("root remediation")
            .contains("TRUENORTH_ROOT")
    );
    assert_eq!(report["layout"]["status"], "skipped");
}

#[test]
fn incomplete_layout_names_the_missing_entry() {
    let repo = seed_complete_repo();
    fs::remove_file(repo.path().join(".agent/tasks/state.yml")).expect("remove state");

    let output = run(repo.path(), &["--check-config"], Some("cargo test"));
    let report = parse_report(&output);

    assert!(!output.status.success(), "incomplete layout fails");
    assert_eq!(report["layout"]["status"], "error");
    assert!(
        report["layout"]["error"]["message"]
            .as_str()
            .expect("layout message")
            .contains(".agent/tasks/state.yml")
    );
}

#[test]
fn missing_verify_command_names_its_fix_without_echoing_values() {
    let repo = seed_complete_repo();
    let output = run(repo.path(), &["--check-config"], None);
    let report = parse_report(&output);

    assert!(!output.status.success(), "missing gate command fails");
    assert_eq!(report["verify_gate"]["status"], "error");
    assert_eq!(report["verify_gate"]["configured"], false);
    assert!(
        report["verify_gate"]["error"]["remediation"]
            .as_str()
            .expect("gate remediation")
            .contains("TRUENORTH_VERIFY_CMD")
    );
    assert!(
        !String::from_utf8(output.stdout)
            .expect("diagnostic output is text")
            .contains("cargo test")
    );
}

#[test]
fn external_backlog_rejects_cached_issue_entries() {
    let repo = seed_complete_repo();
    fs::write(
        repo.path().join(".agent/tasks/backlog.yml"),
        "version: '1'\nownership:\n  mode: external\n  provider: github\n  url: https://example.invalid/issues\nbacklog: []\n",
    )
    .expect("write invalid external backlog");

    let output = run(repo.path(), &["--check-config"], Some("true"));
    let report = parse_report(&output);

    assert!(!output.status.success(), "stale external cache must fail");
    assert_eq!(report["backlog"]["status"], "error");
    assert!(
        report["backlog"]["error"]["message"]
            .as_str()
            .expect("backlog error")
            .contains("must not contain a cached `backlog`")
    );
}

#[test]
fn token_cap_overrides_are_reported_and_invalid_values_fail() {
    let repo = seed_complete_repo();
    let rules = repo.path().join(".agent/config/rules.yml");
    fs::write(
        &rules,
        "features:\n  ontology: false\ntoken_caps:\n  skill_lean_tokens: 1200\n  tool_payload_tokens: 3000\nunrelated: true\n",
    )
    .expect("override rules");
    let output = run(repo.path(), &["--check-config"], Some("true"));
    assert!(output.status.success());
    let report = parse_report(&output);
    assert_eq!(report["features"]["ontology"], false);
    assert_eq!(report["token_caps"]["skill_lean_tokens"], 1200);
    assert_eq!(report["token_caps"]["tool_payload_tokens"], 3000);

    fs::write(&rules, "token_caps:\n  tool_payload_tokens: 0\n").expect("write invalid rules");
    let output = run(repo.path(), &["--check-config"], Some("true"));
    assert!(!output.status.success());
    let report = parse_report(&output);
    assert_eq!(report["token_caps"]["status"], "error");
    assert!(
        report["token_caps"]["error"]["message"]
            .as_str()
            .expect("token cap error")
            .contains("token_caps.tool_payload_tokens")
    );
}
