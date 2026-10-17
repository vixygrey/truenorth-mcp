//! Tests for the tdd_cycle tool (task 11.2).
//!
//! Included from `tools/tdd.rs` via `#[path]`, so `super` is the tools tdd module. The
//! field checks and the red-stage exit-code semantics are tested directly. The ordering
//! guarantee is tested through the cockpit state round-trip.
//!
//! Requirements: 2.6, 2.8, 2.9, 2.10.

use super::*;
use std::{fs, sync::Arc};
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
fn failing_test_cmd_and_group_id_lengths_are_checked() {
    assert!(check_failing_test_cmd("cargo test").is_ok());
    assert!(check_failing_test_cmd("").is_err());
    let long_command = "x".repeat(MAX_FAILING_TEST_CMD + 1);
    assert!(check_failing_test_cmd(&long_command).is_err());

    assert!(check_group_id(None).is_ok());
    assert!(check_group_id(Some("552")).is_ok());
    assert!(check_group_id(Some("")).is_err());
    let long_group = "x".repeat(MAX_GROUP_ID + 1);
    assert!(check_group_id(Some(&long_group)).is_err());
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
fn red_stage_observes_test_exit_status() {
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let failure = server.run_red_stage("false").expect("run failing test");
    assert!(red_expectation_met(&failure));
    assert_eq!(failure.exit_code, Some(1));

    let pass = server.run_red_stage("true").expect("run passing test");
    assert!(!red_expectation_met(&pass));
    assert_eq!(pass.status, GateOutcomeStatus::Pass);
}

#[test]
fn red_stage_timeout_does_not_satisfy_expected_failure() {
    let timeout = GateOutcome {
        status: GateOutcomeStatus::Timeout,
        exit_code: None,
        duration_ms: 5,
        error: Some("timed out".to_string()),
        remediation_hints: Vec::new(),
        diagnostic: None,
    };
    assert!(!red_expectation_met(&timeout));
    let receipt = gate_receipt(&timeout, true);
    assert_eq!(receipt.status, GateStatus::Timeout);
    assert_eq!(receipt.expectation, GateExpectation::Nonzero);
}

#[tokio::test]
async fn public_red_step_records_only_an_observed_failure() {
    use crate::engine::cockpit::read_tdd_step;
    use crate::engine::tdd::TddStep;

    let failed_test_dir = tempdir().expect("temp dir");
    let failed_test_server = server_at(failed_test_dir.path());
    let result = failed_test_server
        .truenorth_tdd_cycle(
            Parameters(TddCycleArgs {
                step: "red".to_string(),
                failing_test_cmd: "false".to_string(),
                group_id: None,
                files_to_modify: vec!["src/lib.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect("a failing test establishes red");
    let response = result.content[0]
        .as_text()
        .expect("text content")
        .text
        .clone();
    let response: serde_json::Value = serde_json::from_str(&response).expect("JSON response");
    assert_eq!(response["step"], "red");
    assert_eq!(response["step_ok"], true);
    assert_eq!(response["receipt"]["operation"], "truenorth_tdd_cycle");
    assert_eq!(response["receipt"]["gate"]["status"], "failure");
    assert_eq!(response["receipt"]["gate"]["expectation"], "nonzero");
    let state =
        fs::read(failed_test_dir.path().join(".agent/tasks/state.yml")).expect("read state bytes");
    assert_eq!(
        response["receipt"]["changes"][0]["sha256"],
        crate::engine::digest::sha256(&state)
    );
    assert_eq!(
        read_tdd_step(failed_test_dir.path()).expect("read red state"),
        Some(TddStep::Red)
    );

    let passing_test_dir = tempdir().expect("temp dir");
    let passing_test_server = server_at(passing_test_dir.path());
    let error = passing_test_server
        .truenorth_tdd_cycle(
            Parameters(TddCycleArgs {
                step: "red".to_string(),
                failing_test_cmd: "true".to_string(),
                group_id: None,
                files_to_modify: vec!["src/lib.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect_err("a passing test cannot establish red");
    assert!(error.message.contains("did not fail as required"));
    assert_eq!(
        read_tdd_step(passing_test_dir.path()).expect("read unchanged state"),
        None
    );
}

#[tokio::test]
async fn scoped_cycle_starts_without_mutating_legacy_state() {
    let dir = tempdir().expect("temp dir");
    let state_path = dir.path().join(".agent/tasks/state.yml");
    fs::create_dir_all(state_path.parent().expect("state parent")).expect("create state parent");
    fs::write(&state_path, "tdd:\n  step: green\n").expect("seed legacy state");

    let server = server_at(dir.path());
    let result = server
        .truenorth_tdd_cycle(
            Parameters(TddCycleArgs {
                step: "red".to_string(),
                failing_test_cmd: "false".to_string(),
                group_id: Some("552".to_string()),
                files_to_modify: vec!["src/lib.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(2)),
        )
        .await
        .expect("a new group starts at red");
    let response: serde_json::Value =
        serde_json::from_str(&result.content[0].as_text().expect("text content").text)
            .expect("JSON response");
    assert_eq!(response["group_id"], "552");

    server
        .truenorth_tdd_cycle(
            Parameters(TddCycleArgs {
                step: "green".to_string(),
                failing_test_cmd: "ignored".to_string(),
                group_id: Some("552".to_string()),
                files_to_modify: vec!["src/lib.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(3)),
        )
        .await
        .expect("the selected group advances to green");
    server
        .truenorth_tdd_cycle(
            Parameters(TddCycleArgs {
                step: "red".to_string(),
                failing_test_cmd: "false".to_string(),
                group_id: Some("515".to_string()),
                files_to_modify: vec!["src/lib.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(4)),
        )
        .await
        .expect("a different group starts independently");
    let before_rejected_transition = fs::read(&state_path).expect("read independent scopes");
    let error = server
        .truenorth_tdd_cycle(
            Parameters(TddCycleArgs {
                step: "refactor".to_string(),
                failing_test_cmd: "ignored".to_string(),
                group_id: Some("515".to_string()),
                files_to_modify: vec!["src/lib.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(5)),
        )
        .await
        .expect_err("red cannot skip green");
    assert!(error.message.contains("transition"));
    assert_eq!(
        fs::read(&state_path).expect("state unchanged after rejected transition"),
        before_rejected_transition
    );

    let state: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(&state_path).expect("read state")).expect("parse");
    assert_eq!(state["tdd"]["step"], "green");
    assert_eq!(state["tdd"]["scopes"]["552"]["step"], "green");
    assert_eq!(state["tdd"]["scopes"]["515"]["step"], "red");
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

fn cycle_args(step: &str, command: &str) -> TddCycleArgs {
    TddCycleArgs {
        step: step.to_string(),
        failing_test_cmd: command.to_string(),
        group_id: None,
        files_to_modify: vec!["src/lib.rs".to_string()],
    }
}

#[tokio::test]
async fn public_cycle_enforces_every_transition_and_preserves_rejected_state() {
    use crate::engine::cockpit::read_tdd_step;
    use crate::engine::tdd::TddStep;

    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let request = || rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(7));

    server
        .truenorth_tdd_cycle(Parameters(cycle_args("red", "false")), request())
        .await
        .expect("none to red");
    let state_path = dir.path().join(".agent/tasks/state.yml");
    let red_state = fs::read(&state_path).expect("red state");

    let error = server
        .truenorth_tdd_cycle(Parameters(cycle_args("refactor", "ignored")), request())
        .await
        .expect_err("red to refactor is invalid");
    assert!(error.message.contains("transition"));
    assert_eq!(
        fs::read(&state_path).expect("unchanged red state"),
        red_state
    );

    server
        .truenorth_tdd_cycle(
            Parameters(cycle_args("green", "command is not run")),
            request(),
        )
        .await
        .expect("red to green");
    assert_eq!(
        read_tdd_step(dir.path()).expect("green state"),
        Some(TddStep::Green)
    );
    server
        .truenorth_tdd_cycle(
            Parameters(cycle_args("refactor", "command is not run")),
            request(),
        )
        .await
        .expect("green to refactor");
    assert_eq!(
        read_tdd_step(dir.path()).expect("refactor state"),
        Some(TddStep::Refactor)
    );
    server
        .truenorth_tdd_cycle(Parameters(cycle_args("red", "false")), request())
        .await
        .expect("refactor to next red");
    assert_eq!(
        read_tdd_step(dir.path()).expect("new red state"),
        Some(TddStep::Red)
    );
}

#[tokio::test]
async fn public_cycle_rejects_invalid_fields_without_creating_state() {
    let invalid = [
        cycle_args("blue", "false"),
        cycle_args("red", ""),
        TddCycleArgs {
            step: "red".to_string(),
            failing_test_cmd: "false".to_string(),
            group_id: None,
            files_to_modify: vec![" ".to_string()],
        },
        TddCycleArgs {
            step: "red".to_string(),
            failing_test_cmd: "false".to_string(),
            group_id: Some("".to_string()),
            files_to_modify: vec!["src/lib.rs".to_string()],
        },
    ];

    for args in invalid {
        let dir = tempdir().expect("temp dir");
        let error = server_at(dir.path())
            .truenorth_tdd_cycle(
                Parameters(args),
                rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(8)),
            )
            .await
            .expect_err("invalid request");
        assert_eq!(error.code.0, -32602);
        assert!(!dir.path().join(".agent/tasks/state.yml").exists());
    }
}

#[test]
fn gate_receipt_maps_pass_failure_and_both_expectations() {
    let pass = GateOutcome {
        status: GateOutcomeStatus::Pass,
        exit_code: Some(0),
        duration_ms: 1,
        error: None,
        remediation_hints: Vec::new(),
        diagnostic: None,
    };
    let receipt = gate_receipt(&pass, false);
    assert_eq!(receipt.status, GateStatus::Pass);
    assert_eq!(receipt.expectation, GateExpectation::Zero);

    let failure = GateOutcome {
        status: GateOutcomeStatus::Failure,
        exit_code: Some(2),
        duration_ms: 2,
        error: None,
        remediation_hints: Vec::new(),
        diagnostic: None,
    };
    let receipt = gate_receipt(&failure, true);
    assert_eq!(receipt.status, GateStatus::Failure);
    assert_eq!(receipt.expectation, GateExpectation::Nonzero);
}

#[test]
fn cockpit_error_maps_transition_write_and_io_categories() {
    let transition = cockpit_error(CockpitError::Transition {
        current: "red",
        from: "red",
        to: "refactor",
        expected: "green",
    });
    assert_eq!(transition.code.0, -32600);

    let io = cockpit_error(CockpitError::Io {
        file: "state.yml".to_string(),
        source: std::io::Error::other("read failed"),
    });
    assert_eq!(io.code.0, -32603);

    let write = cockpit_error(CockpitError::Write(
        crate::engine::agent_ws::WriteGuardError::OutsideAgent {
            target: "state.yml".to_string(),
        },
    ));
    assert_eq!(write.code.0, -32603);
}
