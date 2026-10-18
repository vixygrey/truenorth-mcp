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
        senior_developer: true,
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
            task: None,
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
            task: None,
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
            task: None,
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
            task: None,
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
            task: None,
        }))
        .await;

    let error = result.expect_err("an empty path entry is rejected");
    assert!(
        error.message.contains("`paths[1]` must not be empty"),
        "names the offending index: {}",
        error.message
    );
}

#[tokio::test]
async fn too_many_paths_is_rejected_at_the_guard_boundary() {
    let dir = tempdir().expect("temp dir");
    let paths = (0..=MAX_PATHS)
        .map(|index| format!("src/file-{index}.rs"))
        .collect();
    let error = server_with_jev(dir.path())
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths,
            content: "clean".to_string(),
            task: None,
        }))
        .await
        .expect_err("path count over cap");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[tokio::test]
async fn content_byte_cap_counts_utf8_bytes() {
    let dir = tempdir().expect("temp dir");
    let content = "é".repeat(MAX_CONTENT_BYTES / 2 + 1);
    assert!(content.chars().count() < MAX_CONTENT_BYTES);
    assert!(content.len() > MAX_CONTENT_BYTES);
    let error = server_with_jev(dir.path())
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: vec!["src/main.rs".to_string()],
            content,
            task: None,
        }))
        .await
        .expect_err("UTF-8 byte cap");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[tokio::test]
async fn malformed_guard_config_is_an_internal_decision_error() {
    let dir = tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join(".agent/config")).expect("config dir");
    std::fs::write(
        dir.path().join(".agent/config/rules.yml"),
        "jev: [unterminated\n",
    )
    .expect("malformed config");

    let error = server_with_jev(dir.path())
        .truenorth_guard_change(Parameters(GuardChangeArgs {
            paths: vec!["src/main.rs".to_string()],
            content: "clean".to_string(),
            task: None,
        }))
        .await
        .expect_err("config resolution");
    assert_eq!(error.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
}

#[test]
fn annotate_decision_returns_structured_success_and_obeys_payload_cap() {
    let caps = crate::engine::features::TokenCaps {
        skill_lean_tokens: 100,
        tool_payload_tokens: 100,
    };
    let result = decision_to_result(
        GuardDecision::Annotate {
            notes: vec!["review the proposed scope".to_string()],
        },
        caps,
    )
    .expect("annotation result");
    let text = result.content[0].as_text().expect("text");
    let value: serde_json::Value = serde_json::from_str(&text.text).expect("JSON");
    assert_eq!(value["decision"], "annotate");
    assert_eq!(
        value["notes"],
        serde_json::json!(["review the proposed scope"])
    );

    let tiny_caps = crate::engine::features::TokenCaps {
        skill_lean_tokens: 100,
        tool_payload_tokens: 1,
    };
    let error = decision_to_result(
        GuardDecision::Annotate {
            notes: vec!["long advisory note".to_string()],
        },
        tiny_caps,
    )
    .expect_err("payload cap");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[tokio::test]
async fn unsafe_path_is_invalid_params_in_the_default_offline_build() {
    let dir = tempdir().expect("temp dir");
    for path in [
        "../outside.rs",
        "/etc/passwd",
        ".env",
        ".agent/telemetry/log.yml",
    ] {
        let error = server_with_jev(dir.path())
            .truenorth_guard_change(Parameters(GuardChangeArgs {
                paths: vec!["src/main.rs".to_string(), path.to_string()],
                content: "clean".to_string(),
                task: None,
            }))
            .await
            .expect_err("unsafe path must not fail open");
        assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(
            error.message.contains(path),
            "error names {path}: {}",
            error.message
        );
    }
}
