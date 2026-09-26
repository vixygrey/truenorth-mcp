//! Process-level regression tests for the one-writer-per-worktree contract.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

struct McpServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpServer {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
            .env_clear()
            .env("TRUENORTH_ROOT", root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start MCP server");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = BufReader::new(child.stdout.take().expect("server stdout"));
        let mut server = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let initialized = server.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "writer-lease-test", "version": "1" }
            }),
        );
        assert!(
            initialized.get("result").is_some(),
            "initialize: {initialized}"
        );
        server.notify("notifications/initialized", json!({}));
        server
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        )
        .expect("write MCP request");
        self.stdin.flush().expect("flush MCP request");

        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read MCP response");
            assert_ne!(read, 0, "MCP server exited before response {id}");
            let value: Value = serde_json::from_str(line.trim()).expect("JSON MCP response");
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        writeln!(
            self.stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "method": method, "params": params })
        )
        .expect("write MCP notification");
        self.stdin.flush().expect("flush MCP notification");
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }

    fn kill_and_wait(&mut self) {
        self.child.kill().expect("kill MCP server");
        self.child.wait().expect("wait for MCP server");
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn seed_repo() -> TempDir {
    let repo = tempfile::tempdir().expect("create repository");
    fs::create_dir_all(repo.path().join("specs")).expect("create specs");
    fs::create_dir_all(repo.path().join("skills/example")).expect("create skills");
    fs::write(
        repo.path().join("skills/example/SKILL.md"),
        "---\nname: example\ndescription: example\n---\n# Example\n",
    )
    .expect("write skill");
    fs::create_dir_all(repo.path().join(".agent/config")).expect("create config");
    fs::create_dir_all(repo.path().join(".agent/tasks")).expect("create tasks");
    fs::write(repo.path().join(".agent/config/rules.yml"), "{}\n").expect("write config");
    fs::write(
        repo.path().join(".agent/profile.yml"),
        "profile: issue-per-task\n",
    )
    .expect("write profile");
    fs::write(
        repo.path().join(".agent/tasks/state.yml"),
        "phase: discover\n",
    )
    .expect("write state");
    fs::write(
        repo.path().join(".agent/tasks/release-plan.yml"),
        "tasks: []\n",
    )
    .expect("write release plan");
    repo
}

fn assert_lease_conflict(response: &Value, tool: &str) {
    let error = response
        .get("error")
        .unwrap_or_else(|| panic!("{tool} unexpectedly mutated: {response}"));
    assert_eq!(
        error.pointer("/data/type").and_then(Value::as_str),
        Some("writer_lease_conflict"),
        "typed lease error for {tool}: {response}"
    );
}

#[test]
fn second_process_reads_but_cannot_mutate_and_recovers_after_owner_crash() {
    let repo = seed_repo();
    let mut owner = McpServer::start(repo.path());
    let mut contender = McpServer::start(repo.path());

    let read = contender.request("resources/read", json!({ "uri": "truenorth://state" }));
    assert!(
        read.get("result").is_some(),
        "contender read failed: {read}"
    );

    let mutations = [
        (
            "truenorth_record_task",
            json!({ "task_name": "contended task", "verify_command": "true" }),
        ),
        (
            "truenorth_advance_phase",
            json!({
                "from_phase": "discover",
                "to_phase": "design",
                "artifacts_summary": "contended"
            }),
        ),
        (
            "truenorth_tdd_cycle",
            json!({
                "step": "red",
                "failing_test_cmd": "false",
                "files_to_modify": ["src/lib.rs"]
            }),
        ),
        (
            "truenorth_record_bug",
            json!({
                "id": "430-test",
                "external_link": "https://example.test/430",
                "status": "open",
                "linked_ref": "contended task",
                "tags": []
            }),
        ),
        (
            "truenorth_generate_ontology",
            json!({ "domain": "test", "source_paths": ["src"] }),
        ),
        ("build_skill_graph", json!({})),
    ];
    for (tool, arguments) in mutations {
        let response = contender.call_tool(tool, arguments);
        assert_lease_conflict(&response, tool);
    }

    owner.kill_and_wait();
    let recovered = contender.call_tool(
        "truenorth_record_task",
        json!({ "task_name": "recovered task", "verify_command": "true" }),
    );
    assert!(
        recovered.get("result").is_some(),
        "contender did not acquire released lease: {recovered}"
    );
    let plan = fs::read_to_string(repo.path().join(".agent/tasks/release-plan.yml"))
        .expect("read release plan");
    assert!(plan.contains("recovered task"));
    assert!(!plan.contains("contended task"));
}
