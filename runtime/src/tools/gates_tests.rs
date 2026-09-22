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
        passed: true,
        error: None,
        remediation_hints: Vec::new(),
    };
    let result = gate_result(&outcome, "review").expect("pass is a success result");
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn gate_result_failure_is_error_with_hints() {
    let outcome = GateOutcome {
        passed: false,
        error: Some("the gate command exited with code 1".to_string()),
        remediation_hints: vec!["fix the failure".to_string()],
    };
    let error = gate_result(&outcome, "review").expect_err("failure is an MCP error");
    assert!(error.message.contains("exited with code 1"));
    // The hints ride in the error data.
    let data = error.data.expect("hints in data");
    assert!(data.to_string().contains("fix the failure"));
}

#[test]
fn verify_command_and_allowlist_env_serialized() {
    // SAFETY: this test owns these env vars and restores them. No other test reads them.
    unsafe {
        std::env::set_var(VERIFY_CMD_ENV, "pnpm test");
        std::env::set_var(ALLOWLIST_ENV, "node, npm");
    }

    assert_eq!(verify_command().as_deref(), Some("pnpm test"));
    let allow = allowlist("pnpm test");
    assert!(allow.contains(&"pnpm".to_string()));
    assert!(allow.contains(&"node".to_string()));
    assert!(allow.contains(&"npm".to_string()));

    unsafe {
        std::env::remove_var(VERIFY_CMD_ENV);
        std::env::remove_var(ALLOWLIST_ENV);
    }
    // With the command unset, resolution yields None.
    assert!(verify_command().is_none());
}
