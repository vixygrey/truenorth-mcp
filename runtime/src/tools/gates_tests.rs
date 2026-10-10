//! Tests for the verify-gate tool (task 10.2).
//!
//! Included from `gates.rs` via `#[path]`, so `super` is the gates module. The tests
//! drive the command, allowlist, and result helpers. The env-backed command
//! test runs serially and owns its variables.
//!
//! Requirements: 3.1, 3.7, 3.8.

use super::*;

#[test]
fn legacy_evidence_fields_are_rejected() {
    for request in [
        r#"{"phase":"review","mode":"evidence","test_evidence":"tests passed"}"#,
        r#"{"phase":"review","test_evidence":"tests passed"}"#,
    ] {
        serde_json::from_str::<VerifyGateArgs>(request)
            .expect_err("legacy evidence fields must be rejected");
    }
}

#[test]
fn allowlist_includes_the_command_binary() {
    // The command's own binary is always allowed.
    let allow = allowlist("cargo test --workspace");
    assert!(allow.contains(&"cargo".to_string()));
}

#[test]
fn gate_result_pass_is_success() {
    let outcome = GateOutcome {
        status: GateOutcomeStatus::Pass,
        exit_code: Some(0),
        duration_ms: 12,
        error: None,
        remediation_hints: Vec::new(),
    };
    let result = gate_result(
        &outcome,
        "review",
        &rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        crate::engine::features::TokenCaps::default(),
    )
    .expect("pass is a success result");
    assert!(!result.is_error.unwrap_or(false));
    let text = result.content[0].as_text().expect("text result");
    let json: serde_json::Value = serde_json::from_str(&text.text).expect("JSON result");
    assert_eq!(json["receipt"]["gate"]["status"], "pass");
    assert_eq!(json["receipt"]["gate"]["duration_ms"], 12);
    assert_eq!(json["receipt"]["gate"]["exit_code"], 0);
    assert!(json["receipt"].get("command").is_none());
}

#[test]
fn gate_result_failure_is_error_with_hints() {
    let outcome = GateOutcome {
        status: GateOutcomeStatus::Failure,
        exit_code: Some(1),
        duration_ms: 9,
        error: Some("the gate command exited with code 1".to_string()),
        remediation_hints: vec!["fix the failure".to_string()],
    };
    let error = gate_result(
        &outcome,
        "review",
        &rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        crate::engine::features::TokenCaps::default(),
    )
    .expect_err("failure is an MCP error");
    assert!(error.message.contains("exited with code 1"));
    let data = error.data.expect("structured error data");
    assert_eq!(data["receipt"]["gate"]["status"], "failure");
    assert_eq!(data["receipt"]["gate"]["exit_code"], 1);
    assert!(data.to_string().contains("fix the failure"));
}

#[tokio::test]
async fn missing_verify_command_rejects_the_public_gate() {
    if !crate::isolated_process_tests::in_child(
        "tools::gates::tests::missing_verify_command_rejects_the_public_gate",
        &[],
    ) {
        return;
    }

    let repo = tempfile::tempdir().expect("temp repo");
    let server = TrueNorthServer::test_server(repo.path().to_path_buf());
    let error = server
        .truenorth_verify_gate(
            Parameters(VerifyGateArgs {
                phase: "review".to_string(),
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(3)),
        )
        .await
        .expect_err("missing verify command");
    assert!(error.message.contains(VERIFY_CMD_ENV));
}

#[test]
fn gate_result_timeout_uses_fallback_message_and_structured_receipt() {
    let outcome = GateOutcome {
        status: GateOutcomeStatus::Timeout,
        exit_code: None,
        duration_ms: 250,
        error: None,
        remediation_hints: vec!["reduce the command scope".to_string()],
    };
    let error = gate_result(
        &outcome,
        "integrate",
        &rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(2)),
        crate::engine::features::TokenCaps::default(),
    )
    .expect_err("timeout is an MCP error");

    assert_eq!(error.message, "the gate failed");
    let data = error.data.expect("structured timeout data");
    assert_eq!(data["receipt"]["gate"]["status"], "timeout");
    assert_eq!(data["receipt"]["gate"]["duration_ms"], 250);
    assert!(data["receipt"]["gate"]["exit_code"].is_null());
    assert_eq!(data["remediation_hints"][0], "reduce the command scope");
}

#[test]
fn gate_receipt_error_mapping_is_internal() {
    let error = receipt_error("invalid receipt".to_string());
    assert_eq!(error.code.0, -32603);
    assert!(error.message.contains("gate receipt"));
}
