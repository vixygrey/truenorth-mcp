//! Behavioral tests for mutation error conversion into MCP error data.

use super::*;
use crate::engine::agent_ws::{RevisionConflict, WriteGuardError};

#[test]
fn contended_lease_is_a_typed_retryable_conflict() {
    let error = mutation_error(MutationError::Contended {
        lock_path: ".agent/.writer.lock",
        holder_pid: Some(4242),
        holder: " (process 4242)".to_string(),
    });

    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_REQUEST);
    let data = error.data.expect("structured lease conflict");
    assert_eq!(data["type"], "writer_lease_conflict");
    assert_eq!(data["lock_path"], ".agent/.writer.lock");
    assert_eq!(data["holder_pid"], 4242);
    let remediation = data["remediation"]
        .as_str()
        .expect("contention remediation");
    assert!(remediation.contains("TRUENORTH_ROOT"));
    assert!(remediation.contains("connection"));
}

#[test]
fn contended_lease_preserves_an_unknown_holder_as_null() {
    let error = mutation_error(MutationError::Contended {
        lock_path: ".agent/.writer.lock",
        holder_pid: None,
        holder: String::new(),
    });
    assert!(error.data.expect("conflict data")["holder_pid"].is_null());
}

#[test]
fn unavailable_lease_is_an_internal_error_without_exposing_detail_in_data() {
    let error = mutation_error(MutationError::Unavailable {
        lock_path: ".agent/.writer.lock",
        detail: "permission denied".to_string(),
    });

    assert_eq!(error.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    let data = error.data.expect("structured unavailable error");
    assert_eq!(data["type"], "writer_lease_unavailable");
    assert_eq!(data["lock_path"], ".agent/.writer.lock");
    let remediation = data["remediation"]
        .as_str()
        .expect("unavailable remediation");
    assert!(remediation.contains("TRUENORTH_ROOT"));
    assert!(remediation.contains("connection"));
    assert!(data.get("detail").is_none());
}

#[test]
fn stale_write_is_a_typed_conflict_with_both_revisions() {
    let error = write_error(WriteGuardError::Conflict(RevisionConflict {
        target: ".agent/tasks/backlog.yml".to_string(),
        changed: ".agent/state.yaml".to_string(),
        expected: "sha256:old".to_string(),
        actual: "sha256:new".to_string(),
    }));

    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_REQUEST);
    let data = error.data.expect("structured stale-write conflict");
    assert_eq!(data["type"], "stale_write_conflict");
    assert_eq!(data["path"], ".agent/tasks/backlog.yml");
    assert_eq!(data["changed_path"], ".agent/state.yaml");
    assert_eq!(data["expected_digest"], "sha256:old");
    assert_eq!(data["actual_digest"], "sha256:new");
}

#[test]
fn non_conflict_write_failures_are_internal_errors_without_conflict_data() {
    let outside = write_error(WriteGuardError::OutsideAgent {
        target: "../escape".to_string(),
    });
    assert_eq!(outside.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    assert!(outside.data.is_none());

    let io = write_error(WriteGuardError::Io {
        target: ".agent/tasks/backlog.yml".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    });
    assert_eq!(io.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    assert!(io.data.is_none());
}
