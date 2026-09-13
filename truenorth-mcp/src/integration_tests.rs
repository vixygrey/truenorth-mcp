//! End-to-end integration test for the wired server (task 15.2).
//!
//! An in-process MCP client drives `resources/list`, `resources/read`, and a Discover to
//! Integrate tool sequence against a temp repo over an in-memory duplex transport. The
//! test asserts the tools write through to the cockpit files and that a `resources/read`
//! reflects the current on-disk content, including a change made after the first read.
//!
//! The watcher-detected `resources/updated` notification is exercised in the unit tests
//! for the debouncer; here the focus is the wired request path, which is deterministic.
//!
//! Requirements: 1.1, 2.1, 2.3, 5.1, 5.4, 5.5.

use std::fs;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ReadResourceRequestParams};
use tempfile::tempdir;

use crate::server::TrueNorthServer;

/// Seed a minimal repo so root resolution and the tools have something to work with.
fn seed_repo(root: &std::path::Path) {
    fs::create_dir_all(root.join("skills/develop-tdd")).expect("skills dir");
    fs::write(root.join("skills/develop-tdd/SKILL.md"), "# TDD\n").expect("skill");
    // The cockpit files live under .agent/tasks/ after the relocation (Requirement 2).
    fs::create_dir_all(root.join(".agent/tasks")).expect("tasks dir");
    fs::write(root.join(".agent/tasks/state.yml"), "active_epic: e01\n").expect("state");
    fs::write(
        root.join(".agent/tasks/release-plan.yml"),
        "build_order:\n- e01\n",
    )
    .expect("plan");
}

#[tokio::test]
async fn full_lifecycle_over_in_process_client() -> anyhow::Result<()> {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    seed_repo(&root);

    // Wire the server and a client over an in-memory duplex transport.
    let (server_transport, client_transport) = tokio::io::duplex(8192);
    let server = TrueNorthServer::new(root.clone());
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;

    // resources/list returns the four cockpit resources (Requirement 5.1).
    let resources = client.list_all_resources().await?;
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(uris.contains(&"truenorth://state"));
    assert!(uris.contains(&"truenorth://cockpit"));
    assert!(uris.contains(&"truenorth://ontology"));
    assert!(uris.contains(&"truenorth://conventions"));

    // resources/read returns the current on-disk state (Requirement 5.5).
    let state = read_text(&client, "truenorth://state").await?;
    assert!(state.contains("active_epic: e01"));

    // Drive a phase advance. The tool writes state.yaml (Requirements 2.1, 2.3).
    call_tool(
        &client,
        "truenorth_advance_phase",
        serde_json::json!({
            "from_phase": "discover",
            "to_phase": "design",
            "artifacts_summary": "Modeled the domain."
        }),
    )
    .await?;

    // The state resource now reflects the written phase (Requirement 5.4/5.5).
    let after_advance = read_text(&client, "truenorth://state").await?;
    assert!(
        after_advance.contains("phase: design"),
        "state reflects the advance"
    );

    // Record a task. The tool appends to release-plan.yaml (Requirements 2.1, 2.5).
    call_tool(
        &client,
        "truenorth_record_task",
        serde_json::json!({
            "epic_id": "e80",
            "task_name": "Wire the server",
            "verify_command": "cargo test"
        }),
    )
    .await?;

    // The cockpit resource reflects the appended task.
    let cockpit = read_text(&client, "truenorth://cockpit").await?;
    assert!(
        cockpit.contains("Wire the server"),
        "cockpit reflects the task"
    );

    // A direct disk edit is reflected on the next read (disk is the source of truth).
    fs::write(
        root.join(".agent/tasks/state.yml"),
        "active_epic: e99\nphase: integrate\n",
    )?;
    let after_edit = read_text(&client, "truenorth://state").await?;
    assert!(after_edit.contains("e99"), "read reflects the on-disk edit");

    // Tear down.
    client.cancel().await?;
    server_handle.abort();
    Ok(())
}

/// Read a resource's first text content block.
async fn read_text(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    uri: &str,
) -> anyhow::Result<String> {
    let result = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await?;
    let text = result
        .contents
        .first()
        .and_then(|c| match c {
            rmcp::model::ResourceContents::TextResourceContents { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    Ok(text)
}

/// Call a tool with JSON arguments and assert it did not error.
async fn call_tool(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &'static str,
    args: serde_json::Value,
) -> anyhow::Result<()> {
    let arguments = args.as_object().cloned().unwrap_or_default();
    let result = client
        .call_tool(CallToolRequestParams::new(name).with_arguments(arguments))
        .await?;
    assert!(
        !result.is_error.unwrap_or(false),
        "tool `{name}` returned an error: {result:?}"
    );
    Ok(())
}
