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
    let environment = verify_command
        .map(|value| vec![("TRUENORTH_VERIFY_CMD", value)])
        .unwrap_or_default();
    run_with_environment(root, args, &environment)
}

fn run_with_environment(root: &Path, args: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = command(root);
    command.args(args).envs(environment.iter().copied());
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
    for directory in ["config", "spec", "tasks", "memories"] {
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
        report["verify_gate"]["environment"]["allowed_names"],
        serde_json::json!(["PATH", "HOME", "TMPDIR", "TMP", "TEMP"])
    );
    assert_eq!(
        report["verify_gate"]["environment"]["configured_names"],
        serde_json::json!([])
    );
    assert_eq!(
        snapshot(repo.path()),
        before,
        "diagnostics leave the workspace unchanged"
    );
}

#[test]
fn safe_gate_environment_names_are_reported_without_values() {
    let repo = seed_complete_repo();
    let marker = "diagnostic-value-must-stay-private";
    let output = run_with_environment(
        repo.path(),
        &["--check-config"],
        &[
            ("TRUENORTH_VERIFY_CMD", "true"),
            ("TRUENORTH_GATE_ENV_ALLOWLIST", "JAVA_HOME"),
            ("JAVA_HOME", marker),
        ],
    );
    let report = parse_report(&output);
    let text = String::from_utf8(output.stdout).expect("diagnostic output is text");

    assert!(output.status.success());
    assert_eq!(
        report["verify_gate"]["environment"]["configured_names"],
        serde_json::json!(["JAVA_HOME"])
    );
    assert!(!text.contains(marker));
}

#[test]
fn credential_gate_environment_name_fails_without_echoing_its_value() {
    let repo = seed_complete_repo();
    let marker = "credential-value-must-stay-private";
    let output = run_with_environment(
        repo.path(),
        &["--check-config"],
        &[
            ("TRUENORTH_VERIFY_CMD", "true"),
            ("TRUENORTH_GATE_ENV_ALLOWLIST", "GITHUB_TOKEN"),
            ("GITHUB_TOKEN", marker),
        ],
    );
    let report = parse_report(&output);
    let text = String::from_utf8(output.stdout).expect("diagnostic output is text");

    assert!(!output.status.success());
    assert_eq!(report["verify_gate"]["status"], "error");
    assert_eq!(
        report["verify_gate"]["environment"]["rejected_names"],
        serde_json::json!(["GITHUB_TOKEN"])
    );
    assert!(text.contains("GITHUB_TOKEN"));
    assert!(!text.contains(marker));
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
        "version: 2\nfeatures:\n  ontology: false\nruntime:\n  token_caps:\n    skill_lean_tokens: 1200\n    tool_payload_tokens: 3000\nadvisory:\n  custom: true\n",
    )
    .expect("override rules");
    let output = run(repo.path(), &["--check-config"], Some("true"));
    assert!(output.status.success());
    let report = parse_report(&output);
    assert_eq!(report["features"]["ontology"], false);
    assert_eq!(report["token_caps"]["skill_lean_tokens"], 1200);
    assert_eq!(report["token_caps"]["tool_payload_tokens"], 3000);
    assert_eq!(report["rules_config"]["version"], 2);
    assert_eq!(report["rules_config"]["blocks"]["runtime"], "enforced");
    assert_eq!(report["rules_config"]["blocks"]["advisory"], "advisory");

    fs::write(
        &rules,
        "version: 2\nruntime:\n  token_caps:\n    tool_payload_tokens: 0\n",
    )
    .expect("write invalid rules");
    let output = run(repo.path(), &["--check-config"], Some("true"));
    assert!(!output.status.success());
    let report = parse_report(&output);
    assert_eq!(report["token_caps"]["status"], "error");
    assert!(
        report["token_caps"]["error"]["message"]
            .as_str()
            .expect("token cap error")
            .contains("runtime.token_caps.tool_payload_tokens")
    );
}

#[test]
fn strict_mode_rejects_unknown_keys_and_legacy_unnamespaced_blocks() {
    let repo = seed_complete_repo();
    let rules = repo.path().join(".agent/config/rules.yml");

    // Strict check passes on clean v2 rules
    fs::write(
        &rules,
        "version: 2\nfeatures:\n  ontology: true\nruntime:\n  token_caps:\n    skill_lean_tokens: 1500\n    tool_payload_tokens: 4000\nadvisory:\n  quality_gates:\n    coverage: 80\n",
    )
    .expect("write clean v2 rules");
    let output = run(repo.path(), &["--check-config", "--strict"], Some("true"));
    assert!(output.status.success(), "clean v2 passes strict check");

    // Strict check fails on unknown keys
    fs::write(
        &rules,
        "version: 2\nfeatures:\n  ontology: true\nunknown_section:\n  bar: 1\n",
    )
    .expect("write unknown section");
    let output = run(repo.path(), &["--check-config", "--strict"], Some("true"));
    assert!(
        !output.status.success(),
        "strict check fails on unknown key"
    );
    let report = parse_report(&output);
    assert!(
        report["rules_config"]["errors"][0]
            .as_str()
            .unwrap()
            .contains("unknown configuration key `unknown_section`")
    );
}
