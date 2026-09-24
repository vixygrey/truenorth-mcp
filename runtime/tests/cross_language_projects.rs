//! End-to-end compatibility fixtures for language-agnostic project initialization.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ContentBlock};
use serde_json::json;
use tempfile::{TempDir, tempdir};
use truenorth_mcp::config::VERIFY_CMD_ENV;
use truenorth_mcp::server::TrueNorthServer;

const PROFILE: &str = "issue-per-task";

struct ProjectCase {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    protected_paths: &'static [&'static str],
    verify_command: &'static str,
}

const RUST_FILES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        "[package]\nname = \"fixture-rust\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    ),
    (
        "src/lib.rs",
        "pub fn answer() -> u8 { 42 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn returns_the_answer() {\n        assert_eq!(super::answer(), 42);\n    }\n}\n",
    ),
];

const NODE_FILES: &[(&str, &str)] = &[
    (
        "package.json",
        "{\n  \"name\": \"fixture-node\",\n  \"private\": true,\n  \"scripts\": {\n    \"test\": \"node --test\"\n  }\n}\n",
    ),
    ("src/index.js", "export const answer = () => 42;\n"),
    (
        "test/index.test.js",
        "import test from 'node:test';\nimport assert from 'node:assert/strict';\nimport { answer } from '../src/index.js';\n\ntest('returns the answer', () => assert.equal(answer(), 42));\n",
    ),
];

const PYTHON_FILES: &[(&str, &str)] = &[
    (
        "pyproject.toml",
        "[project]\nname = \"fixture-python\"\nversion = \"0.1.0\"\nrequires-python = \">=3.11\"\n",
    ),
    (
        "fixture_python/__init__.py",
        "def answer():\n    return 42\n",
    ),
    (
        "tests/test_project.py",
        "import unittest\n\nfrom fixture_python import answer\n\n\nclass ProjectTest(unittest.TestCase):\n    def test_returns_the_answer(self):\n        self.assertEqual(answer(), 42)\n",
    ),
];

const GO_FILES: &[(&str, &str)] = &[
    ("go.mod", "module example.invalid/fixture\n\ngo 1.22\n"),
    (
        "answer.go",
        "package fixture\n\nfunc Answer() int { return 42 }\n",
    ),
    (
        "answer_test.go",
        "package fixture\n\nimport \"testing\"\n\nfunc TestAnswer(t *testing.T) {\n\tif Answer() != 42 {\n\t\tt.Fatalf(\"Answer() = %d, want 42\", Answer())\n\t}\n}\n",
    ),
];

const DOCS_FILES: &[(&str, &str)] = &[
    ("README.md", "# Documentation fixture\n"),
    ("docs/guide.md", "# Guide\n\nVerified documentation.\n"),
    (
        "verify-docs.sh",
        "#!/bin/sh\nset -eu\ntest -s README.md\ntest -s docs/guide.md\n",
    ),
];

const MONOREPO_FILES: &[(&str, &str)] = &[
    (
        "rust-app/Cargo.toml",
        "[package]\nname = \"fixture-monorepo-rust\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    ),
    (
        "rust-app/src/lib.rs",
        "pub fn healthy() -> bool { true }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn is_healthy() {\n        assert!(super::healthy());\n    }\n}\n",
    ),
    (
        "node-app/package.json",
        "{\n  \"name\": \"fixture-monorepo-node\",\n  \"private\": true,\n  \"scripts\": {\n    \"test\": \"node --test\"\n  }\n}\n",
    ),
    (
        "node-app/test/smoke.test.js",
        "import test from 'node:test';\nimport assert from 'node:assert/strict';\n\ntest('monorepo node package', () => assert.equal(6 * 7, 42));\n",
    ),
    (
        "verify-workspace.sh",
        "#!/bin/sh\nset -eu\n(cd rust-app && cargo test --quiet)\n(cd node-app && npm test --silent)\n",
    ),
];

const CASES: &[ProjectCase] = &[
    ProjectCase {
        name: "rust",
        files: RUST_FILES,
        protected_paths: &["Cargo.toml", "src"],
        verify_command: "cargo test --quiet",
    },
    ProjectCase {
        name: "node",
        files: NODE_FILES,
        protected_paths: &["package.json", "src", "test"],
        verify_command: "npm test --silent",
    },
    ProjectCase {
        name: "python",
        files: PYTHON_FILES,
        protected_paths: &["pyproject.toml", "fixture_python", "tests"],
        verify_command: "python3 -B -m unittest discover -s tests",
    },
    ProjectCase {
        name: "go",
        files: GO_FILES,
        protected_paths: &["go.mod", "answer.go", "answer_test.go"],
        verify_command: "go test ./...",
    },
    ProjectCase {
        name: "documentation",
        files: DOCS_FILES,
        protected_paths: &["README.md", "docs", "verify-docs.sh"],
        verify_command: "sh verify-docs.sh",
    },
    ProjectCase {
        name: "mixed-monorepo",
        files: MONOREPO_FILES,
        protected_paths: &[
            "rust-app/Cargo.toml",
            "rust-app/src",
            "node-app/package.json",
            "node-app/test",
            "verify-workspace.sh",
        ],
        verify_command: "sh verify-workspace.sh",
    },
];

#[tokio::test]
async fn cross_language_projects_complete_the_workflow_without_source_mutation()
-> anyhow::Result<()> {
    let skills = minimal_skill_bundle()?;
    let _verify_env = EnvironmentGuard::capture(VERIFY_CMD_ENV);

    for case in CASES {
        exercise_project(case, skills.path()).await?;
    }
    Ok(())
}

async fn exercise_project(case: &ProjectCase, skills: &Path) -> anyhow::Result<()> {
    let repo = tempdir()?;
    seed_project(repo.path(), case)?;
    let before = snapshot_paths(repo.path(), case.protected_paths)?;

    let init = run_binary(
        repo.path(),
        &[
            "init",
            "--skills-dir",
            path_text(skills)?,
            "--profile",
            PROFILE,
        ],
        None,
    );
    assert_success(case, "init", &init);
    assert_eq!(
        fs::read_to_string(repo.path().join(".agent/profile.yml"))?,
        format!("profile: {PROFILE}\n"),
        "fixture={} rule=profile-declaration",
        case.name
    );

    let workspace_before_check = snapshot_directory(repo.path())?;
    let diagnostics = run_binary(repo.path(), &["--check-config"], Some(case.verify_command));
    assert_success(case, "check-config", &diagnostics);
    let report: serde_json::Value = serde_json::from_slice(&diagnostics.stdout)?;
    for field in ["repository_root", "layout", "backlog", "verify_gate"] {
        assert_eq!(
            report[field]["status"], "ok",
            "fixture={} rule=check-config field={field} report={report}",
            case.name
        );
    }
    assert_eq!(
        snapshot_directory(repo.path())?,
        workspace_before_check,
        "fixture={} rule=check-config-read-only",
        case.name
    );

    unsafe {
        std::env::set_var(VERIFY_CMD_ENV, case.verify_command);
    }
    let (client, handle) = connect(repo.path().to_path_buf()).await?;
    let tools = client.list_all_tools().await?;
    for name in [
        "truenorth_advance_phase",
        "truenorth_record_task",
        "truenorth_verify_gate",
    ] {
        assert!(
            tools.iter().any(|tool| tool.name == name),
            "fixture={} rule=tool-advertised tool={name}",
            case.name
        );
    }

    call_tool(
        &client,
        case,
        "truenorth_advance_phase",
        json!({
            "from_phase": "discover",
            "to_phase": "design",
            "artifacts_summary": format!("Validated the {} fixture.", case.name),
        }),
    )
    .await?;
    let state: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(
        repo.path().join(".agent/tasks/state.yml"),
    )?)?;
    assert_eq!(
        state["phase"].as_str(),
        Some("design"),
        "fixture={} rule=lifecycle",
        case.name
    );

    let task_name = format!("Verify {} compatibility", case.name);
    call_tool(
        &client,
        case,
        "truenorth_record_task",
        json!({
            "group_id": "t393",
            "group_kind": "ticket",
            "task_name": task_name,
            "verify_command": case.verify_command,
        }),
    )
    .await?;
    let plan = fs::read_to_string(repo.path().join(".agent/tasks/release-plan.yml"))?;
    assert!(
        plan.contains(&task_name) && plan.contains(case.verify_command),
        "fixture={} rule=record-task plan={plan}",
        case.name
    );

    let gate = call_tool(
        &client,
        case,
        "truenorth_verify_gate",
        json!({ "phase": "design" }),
    )
    .await?;
    let result: serde_json::Value = serde_json::from_str(tool_text(&gate)?)?;
    assert_eq!(
        result["passed"], true,
        "fixture={} rule=verify-gate result={result}",
        case.name
    );
    assert_eq!(result["phase"], "design");
    assert_eq!(result["mode"], "execute");

    client.cancel().await?;
    handle.abort();

    assert_eq!(
        snapshot_paths(repo.path(), case.protected_paths)?,
        before,
        "fixture={} rule=project-files-unchanged",
        case.name
    );
    Ok(())
}

fn minimal_skill_bundle() -> anyhow::Result<TempDir> {
    let bundle = tempdir()?;
    let path = bundle.path().join("using-truenorth/SKILL.md");
    fs::create_dir_all(path.parent().expect("skill parent"))?;
    fs::write(path, "# Using TrueNorth\n")?;
    Ok(bundle)
}

fn seed_project(root: &Path, case: &ProjectCase) -> anyhow::Result<()> {
    for (relative, content) in case.files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
    }
    Ok(())
}

fn run_binary(root: &Path, args: &[&str], verify_command: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"));
    command.env_clear().current_dir(root).args(args);
    if let Some(command_value) = verify_command {
        command.env(VERIFY_CMD_ENV, command_value);
    }
    command.output().expect("run truenorth-mcp")
}

fn assert_success(case: &ProjectCase, operation: &str, output: &Output) {
    assert!(
        output.status.success(),
        "fixture={} rule={operation} stderr={}",
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn path_text(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::anyhow!("fixture path is not UTF-8: {}", path.display()))
}

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

async fn connect(
    root: PathBuf,
) -> anyhow::Result<(Client, tokio::task::JoinHandle<anyhow::Result<()>>)> {
    let (server_transport, client_transport) = tokio::io::duplex(8192);
    let server = TrueNorthServer::resolve(root)?;
    let handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;
    Ok((client, handle))
}

async fn call_tool(
    client: &Client,
    case: &ProjectCase,
    name: &'static str,
    args: serde_json::Value,
) -> anyhow::Result<rmcp::model::CallToolResult> {
    let arguments = args
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("tool arguments must be an object"))?;
    let result = client
        .call_tool(CallToolRequestParams::new(name).with_arguments(arguments))
        .await?;
    assert!(
        !result.is_error.unwrap_or(false),
        "fixture={} rule=tool-call tool={name} result={result:?}",
        case.name
    );
    Ok(result)
}

fn tool_text(result: &rmcp::model::CallToolResult) -> anyhow::Result<&str> {
    match result.content.first() {
        Some(ContentBlock::Text(text)) => Ok(&text.text),
        other => Err(anyhow::anyhow!("tool returned no text result: {other:?}")),
    }
}

fn snapshot_paths(
    root: &Path,
    relative_paths: &[&str],
) -> anyhow::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut snapshot = BTreeMap::new();
    for relative in relative_paths {
        snapshot_path(root, &root.join(relative), &mut snapshot)?;
    }
    Ok(snapshot)
}

fn snapshot_directory(root: &Path) -> anyhow::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut snapshot = BTreeMap::new();
    snapshot_path(root, root, &mut snapshot)?;
    Ok(snapshot)
}

fn snapshot_path(
    root: &Path,
    path: &Path,
    snapshot: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> anyhow::Result<()> {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            snapshot_path(root, &entry.path(), snapshot)?;
        }
    } else {
        let relative = path.strip_prefix(root)?.to_path_buf();
        snapshot.insert(relative, fs::read(path)?);
    }
    Ok(())
}

struct EnvironmentGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvironmentGuard {
    fn capture(name: &'static str) -> Self {
        Self {
            name,
            previous: std::env::var_os(name),
        }
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}
