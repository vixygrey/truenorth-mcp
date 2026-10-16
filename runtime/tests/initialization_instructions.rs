//! Process-level regression tests for initialization instruction delivery.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn initialize(root: &Path) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"));
    command
        .env_clear()
        .env("TRUENORTH_ROOT", root)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(profile_file) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile_file);
    }

    let mut child = command.spawn().expect("start MCP server");
    let mut stdin = child.stdin.take().expect("server stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("server stdout"));
    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "instruction-test", "version": "1" }
            }
        })
    )
    .expect("write initialize request");
    stdin.flush().expect("flush initialize request");

    let response = loop {
        let mut line = String::new();
        let read = stdout.read_line(&mut line).expect("read MCP response");
        assert_ne!(read, 0, "MCP server exited before initialize response");
        let value: Value = serde_json::from_str(line.trim()).expect("JSON MCP response");
        if value.get("id").and_then(Value::as_u64) == Some(1) {
            break value;
        }
    };

    child.kill().expect("kill MCP server");
    child.wait().expect("wait for MCP server");
    response
}

fn instructions(response: &Value) -> &str {
    response["result"]["instructions"]
        .as_str()
        .expect("initialize instructions")
}

#[test]
fn opt_out_and_reenable_take_effect_on_subsequent_connections() {
    let repo = tempfile::tempdir().expect("create repository");
    for directory in [".agent", "specs", "skills"] {
        fs::create_dir(repo.path().join(directory)).expect("create required workspace directory");
    }
    assert!(
        !repo
            .path()
            .join("skills/senior-developer/SKILL.md")
            .exists()
    );

    let default_response = initialize(repo.path());
    let default_instructions = instructions(&default_response);

    let config = repo.path().join(".agent/config");
    fs::create_dir_all(&config).expect("create config directory");
    fs::write(
        config.join("rules.yml"),
        "features:\n  senior_developer: false\n",
    )
    .expect("write opt-out");
    let disabled_response = initialize(repo.path());
    let disabled_instructions = instructions(&disabled_response);
    assert_eq!(
        default_instructions,
        format!(
            "{disabled_instructions}\n\n{}",
            include_str!("../../skills/senior-developer/SKILL.md")
        )
    );

    fs::write(
        config.join("rules.yml"),
        "features:\n  senior_developer: true\n",
    )
    .expect("write opt-in");
    let reenabled_response = initialize(repo.path());
    assert_eq!(instructions(&reenabled_response), default_instructions);
}
