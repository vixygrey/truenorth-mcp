//! Tests for the tdd_cycle tool (task 11.2).
//!
//! Included from `tools/tdd.rs` via `#[path]`, so `super` is the tools tdd module. The
//! field checks and the red-stage exit-code semantics are tested directly. The ordering
//! guarantee is tested through the cockpit state round-trip.
//!
//! Requirements: 2.6, 2.8, 2.9, 2.10.

use super::*;
use std::sync::Arc;
use tempfile::tempdir;

/// Build a server rooted at a temp dir.
fn server_at(root: &std::path::Path) -> TrueNorthServer {
    TrueNorthServer {
        ctx: Arc::new(crate::server::ServerContext::with_features(
            root.to_path_buf(),
            crate::engine::features::Features::default(),
        )),
        tool_router: rmcp::handler::server::router::tool::ToolRouter::new(),
    }
}

#[test]
fn failing_test_cmd_length_is_checked() {
    assert!(check_failing_test_cmd("cargo test").is_ok());
    assert!(check_failing_test_cmd("").is_err());
    let long = "x".repeat(MAX_FAILING_TEST_CMD + 1);
    assert!(check_failing_test_cmd(&long).is_err());
}

#[test]
fn files_to_modify_is_checked() {
    assert!(check_files_to_modify(&["src/lib.rs".to_string()]).is_ok());
    // Empty array.
    assert!(check_files_to_modify(&[]).is_err());
    // An empty entry.
    assert!(check_files_to_modify(&["ok.rs".to_string(), "  ".to_string()]).is_err());
    // Over the cap.
    let many: Vec<String> = (0..MAX_FILES_TO_MODIFY + 1)
        .map(|i| format!("f{i}.rs"))
        .collect();
    assert!(check_files_to_modify(&many).is_err());
}

#[test]
fn red_stage_passes_when_test_fails() {
    // Requirement 2.9: a non-zero exit reports red passed. `false` exits 1.
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    assert!(server.run_red_stage("false").is_ok());
}

#[test]
fn red_stage_fails_when_test_passes() {
    // Requirement 2.10: an exit code 0 reports red failed. `true` exits 0.
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let error = server
        .run_red_stage("true")
        .expect_err("red must fail when the test passes");
    assert!(error.message.contains("did not fail as required"));
}

#[test]
fn ordering_persists_and_invalid_transition_leaves_state_unchanged() {
    // Requirement 2.8: an invalid transition does not change the recorded step.
    use crate::engine::cockpit::{read_tdd_step, write_tdd_step};
    use crate::engine::tdd::{TddStep, next_step};

    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    // Start a cycle: none -> red.
    let step = next_step(read_tdd_step(root).unwrap(), TddStep::Red).unwrap();
    write_tdd_step(root, step).unwrap();
    assert_eq!(read_tdd_step(root).unwrap(), Some(TddStep::Red));

    // An invalid request (red -> refactor) is rejected, and the recorded step stays red.
    let current = read_tdd_step(root).unwrap();
    assert!(next_step(current, TddStep::Refactor).is_err());
    assert_eq!(read_tdd_step(root).unwrap(), Some(TddStep::Red));

    // A valid request (red -> green) advances and persists.
    let step = next_step(current, TddStep::Green).unwrap();
    write_tdd_step(root, step).unwrap();
    assert_eq!(read_tdd_step(root).unwrap(), Some(TddStep::Green));
}

#[test]
fn write_tdd_step_preserves_other_state_fields() {
    // Requirement 9.3: recording the step preserves other state fields.
    use crate::engine::cockpit::{state_path, write_tdd_step};
    use crate::engine::tdd::TddStep;

    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let state = state_path(root);
    std::fs::create_dir_all(state.parent().expect("state parent")).expect("tasks dir");
    std::fs::write(&state, "active_epic: e01\nbigpowers_version: 2.88.2\n").expect("seed state");

    write_tdd_step(root, TddStep::Red).expect("write step");

    let written = std::fs::read_to_string(state_path(root)).expect("read state");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse");
    assert_eq!(
        value.get("active_epic").and_then(|v| v.as_str()),
        Some("e01")
    );
    assert_eq!(
        value.get("bigpowers_version").and_then(|v| v.as_str()),
        Some("2.88.2")
    );
    assert_eq!(
        value
            .get("tdd")
            .and_then(|t| t.get("step"))
            .and_then(|v| v.as_str()),
        Some("red")
    );
}
