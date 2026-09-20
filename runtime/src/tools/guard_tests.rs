//! Tests for the `truenorth_guard_change` tool (task 4, issue #296).
//!
//! Included from `guard.rs` via `#[path]`, so `super` is the guard tool module. These pin the
//! tool mapping: an Allow or an Annotate maps to a success result carrying the notes, a Block
//! maps to an MCP error carrying the neutralization packet, and a malformed input returns a
//! typed error naming the field with no check run (jev-active-guardrail R1.4, R1.5).
//!
//! The default build has no HTTP client and no key, so the probabilistic layer fails open. A
//! deterministic block still holds, so the Block mapping is driven by a protected-path input.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use tempfile::tempdir;

use super::*;

/// Build a server rooted at `root` with the jev feature on.
fn server_with_jev(root: &std::path::Path) -> TrueNorthServer {
    let features = crate::engine::features::Features {
        ontology: false,
        jev: true,
    };
    TrueNorthServer {
        ctx: Arc::new(crate::server::ServerContext::with_features(
            root.to_path_buf(),
            features,
        )),
        tool_router: rmcp::handler::server::router::tool::ToolRouter::new(),
    }
}

#[tokio::test]
async fn a_protected_path_write_maps_to_a_block_error_with_the_packet() {
    let dir = tempdir().expect("temp dir");
    let server = server_with_jev(dir.path());

    let result = server
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: vec!["specs/plan.md".to_string()],
            content: "clean body".to_string(),
        }))
        .await;

    let error = result.expect_err("a protected-path write blocks");
    // The MCP error data carries the neutralization packet.
    let data = error.data.expect("the block carries packet data");
    assert_eq!(data["violated_check"], "protected-path");
    assert_eq!(data["offending_value"], "specs/plan.md");
}

#[tokio::test]
async fn a_secret_content_write_maps_to_a_block_error() {
    let dir = tempdir().expect("temp dir");
    let server = server_with_jev(dir.path());

    let result = server
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: vec!["src/config.rs".to_string()],
            content: "const TOKEN = \"my secret value\";".to_string(),
        }))
        .await;

    let error = result.expect_err("a secret write blocks");
    let data = error.data.expect("the block carries packet data");
    assert_eq!(data["violated_check"], "secret");
    // The packet names a marker, never the secret value (R6.3).
    let offending = data["offending_value"].as_str().unwrap_or_default();
    assert_eq!(offending, "secret-marker");
}

#[tokio::test]
async fn a_clean_change_maps_to_an_allow_success() {
    let dir = tempdir().expect("temp dir");
    let server = server_with_jev(dir.path());

    let result = server
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: vec!["src/main.rs".to_string()],
            content: "fn main() {}".to_string(),
        }))
        .await;

    let call = result.expect("a clean change succeeds");
    // A success result is not an error. The default build has no client, so the probabilistic
    // layer failed open with a note. The decision is allow.
    assert_eq!(call.is_error, Some(false));
    let text = call.content[0]
        .as_text()
        .expect("text content")
        .text
        .clone();
    assert!(
        text.contains("\"decision\":\"allow\""),
        "the decision is allow: {text}"
    );
}

#[tokio::test]
async fn an_empty_paths_input_returns_a_typed_error_with_no_check() {
    let dir = tempdir().expect("temp dir");
    let server = server_with_jev(dir.path());

    let result = server
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: Vec::new(),
            content: "anything".to_string(),
        }))
        .await;

    let error = result.expect_err("an empty paths input is rejected");
    assert!(
        error
            .message
            .contains("`paths` must contain at least one entry"),
        "names the offending field: {}",
        error.message
    );
}

#[tokio::test]
async fn an_empty_path_entry_returns_a_typed_error_naming_the_index() {
    let dir = tempdir().expect("temp dir");
    let server = server_with_jev(dir.path());

    let result = server
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: vec!["src/main.rs".to_string(), String::new()],
            content: "clean".to_string(),
        }))
        .await;

    let error = result.expect_err("an empty path entry is rejected");
    assert!(
        error.message.contains("`paths[1]` must not be empty"),
        "names the offending index: {}",
        error.message
    );
}
