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
    let server = TrueNorthServer::test_server(root.clone());
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;

    // resources/list returns the five served resources (Requirements 5.1, 9.2).
    let resources = client.list_all_resources().await?;
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(uris.contains(&"truenorth://state"));
    assert!(uris.contains(&"truenorth://cockpit"));
    assert!(uris.contains(&"truenorth://ontology"));
    assert!(uris.contains(&"truenorth://conventions"));
    assert!(uris.contains(&"truenorth://adr"));

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

/// A connected in-process client and its server task, for a repo at `root`.
type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

async fn connect(
    root: std::path::PathBuf,
) -> anyhow::Result<(Client, tokio::task::JoinHandle<anyhow::Result<()>>)> {
    let (server_transport, client_transport) = tokio::io::duplex(8192);
    let server = TrueNorthServer::test_server(root);
    let handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;
    Ok((client, handle))
}

/// Connect a client to a server built with explicit feature flags.
async fn connect_with(
    root: std::path::PathBuf,
    features: crate::engine::features::Features,
) -> anyhow::Result<(Client, tokio::task::JoinHandle<anyhow::Result<()>>)> {
    let (server_transport, client_transport) = tokio::io::duplex(8192);
    let ctx = crate::server::ServerContext::with_features(root, features);
    let server = TrueNorthServer::from_context(ctx);
    let handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;
    Ok((client, handle))
}

#[tokio::test]
async fn reads_a_legacy_specs_cockpit_over_the_client() -> anyhow::Result<()> {
    // Requirement 2.9: a legacy specs/ cockpit is served when the .agent/ file is absent.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("skills")).expect("skills");
    fs::create_dir_all(root.join("specs")).expect("specs");
    fs::write(
        root.join("specs/state.yaml"),
        "active_epic: legacy1\nbigpowers_version: 2.88.2\n",
    )?;

    let (client, handle) = connect(root).await?;

    let state = read_text(&client, "truenorth://state").await?;
    assert!(state.contains("legacy1"), "serves the legacy cockpit");
    assert!(
        state.contains("bigpowers_version: 2.88.2"),
        "preserves the version key"
    );

    client.cancel().await?;
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn reads_the_adr_resource_over_the_client() -> anyhow::Result<()> {
    // Requirement 9.2: the read-only ADR resource serves specs/adr/ content.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("skills")).expect("skills");
    fs::create_dir_all(root.join(".agent")).expect("agent");
    fs::create_dir_all(root.join("specs/adr")).expect("adr dir");
    fs::write(
        root.join("specs/adr/0001-verb-noun-naming.md"),
        "# ADR 0001: verb-noun naming\n",
    )?;

    let (client, handle) = connect(root).await?;

    let adr = read_text(&client, "truenorth://adr").await?;
    assert!(adr.contains("verb-noun naming"), "serves the ADR content");

    client.cancel().await?;
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn records_a_bug_over_the_client() -> anyhow::Result<()> {
    // Requirement 8.1: the bug tool stores a reference under .agent/tasks/bugs.yml.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    seed_repo(&root);
    // Seed a release-plan task so the linked_ref resolves.
    fs::write(
        root.join(".agent/tasks/release-plan.yml"),
        "tasks:\n- group_id: e01\n  task_name: A task\n  verify_command: cargo test\n",
    )?;

    let (client, handle) = connect(root.clone()).await?;

    call_tool(
        &client,
        "truenorth_record_bug",
        serde_json::json!({
            "id": "BUG-1",
            "external_link": "https://tracker.example/issues/1",
            "status": "open",
            "linked_ref": "e01",
            "tags": ["regression"]
        }),
    )
    .await?;

    let bugs = fs::read_to_string(root.join(".agent/tasks/bugs.yml"))?;
    assert!(bugs.contains("BUG-1"), "the bug reference is stored");
    assert!(bugs.contains("regression"), "the tag is stored");

    client.cancel().await?;
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn scaffolds_a_project_over_the_client() -> anyhow::Result<()> {
    // Requirement 5.3: the scaffold seeds the .agent/ tree and root docs.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();

    let (client, handle) = connect(root.clone()).await?;

    call_tool(
        &client,
        "truenorth_scaffold_project",
        serde_json::json!({ "profile": "kanban" }),
    )
    .await?;

    assert!(root.join(".agent/layout.yml").is_file());
    assert!(root.join(".agent/profile.yml").is_file());
    assert!(root.join("AGENTS.md").is_file());
    assert!(root.join(".githooks/commit-msg").is_file());
    // Language-agnostic: no code manifests.
    assert!(!root.join("Cargo.toml").exists());
    assert!(!root.join("package.json").exists());

    client.cancel().await?;
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn disabled_ontology_is_absent_over_the_client() -> anyhow::Result<()> {
    // Requirement 3.2, 3.3, 3.4: with the ontology feature disabled, the resource is not
    // listed, a read of its URI errors as unknown, and no ontology file is seeded.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();

    let (client, handle) = connect_with(
        root.clone(),
        crate::engine::features::Features { ontology: false },
    )
    .await?;

    // The ontology resource is not listed; the others still are (Requirement 3.2, 3.6).
    let resources = client.list_all_resources().await?;
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(
        !uris.contains(&"truenorth://ontology"),
        "ontology is not listed"
    );
    assert!(uris.contains(&"truenorth://state"), "state is still listed");
    assert!(
        uris.contains(&"truenorth://conventions"),
        "conventions is still listed"
    );

    // A read of the ontology URI errors as an unknown resource (Requirement 3.3).
    let read = client
        .read_resource(ReadResourceRequestParams::new("truenorth://ontology"))
        .await;
    assert!(read.is_err(), "reading a disabled ontology URI errors");

    // No ontology file was seeded (Requirement 3.4).
    assert!(
        !root.join(".agent/ontology.yml").exists(),
        "no ontology file is seeded when the feature is disabled"
    );

    client.cancel().await?;
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn enabled_ontology_is_listed_and_seeds_on_read() -> anyhow::Result<()> {
    // Requirement 3.1, 3.5: the enabled default lists the ontology resource and seeds the
    // backing file on first read.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();

    let (client, handle) = connect_with(
        root.clone(),
        crate::engine::features::Features { ontology: true },
    )
    .await?;

    let resources = client.list_all_resources().await?;
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(uris.contains(&"truenorth://ontology"), "ontology is listed");

    // First read seeds the backing file under .agent/ (Requirement 3.5).
    let ontology = read_text(&client, "truenorth://ontology").await?;
    assert!(ontology.contains("entities"), "the seeded ontology parses");
    assert!(
        root.join(".agent/ontology.yml").is_file(),
        "seeded under .agent/"
    );

    client.cancel().await?;
    handle.abort();
    Ok(())
}
