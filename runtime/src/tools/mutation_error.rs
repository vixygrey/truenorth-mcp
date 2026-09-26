//! Stable MCP error mapping for repository mutation failures.

use rmcp::ErrorData;

use crate::engine::mutation::MutationError;

/// Map a repository lease failure to a machine-readable MCP error.
pub fn mutation_error(error: MutationError) -> ErrorData {
    match error {
        MutationError::Contended {
            lock_path,
            holder_pid,
            ..
        } => ErrorData::invalid_request(
            format!(
                "another TrueNorth server owns `{lock_path}`. Use that server or use a separate Git worktree."
            ),
            Some(serde_json::json!({
                "type": "writer_lease_conflict",
                "lock_path": lock_path,
                "holder_pid": holder_pid,
                "remediation": "Use the existing TrueNorth server or use a separate Git worktree."
            })),
        ),
        MutationError::Unavailable { lock_path, detail } => ErrorData::internal_error(
            format!(
                "could not acquire the TrueNorth writer lease `{lock_path}`: {detail}. Check `.agent/` permissions or use a separate Git worktree."
            ),
            Some(serde_json::json!({
                "type": "writer_lease_unavailable",
                "lock_path": lock_path,
                "remediation": "Check `.agent/` permissions or use a separate Git worktree."
            })),
        ),
    }
}

/// Map a guarded-write failure, preserving stale writes as semantic conflicts.
pub fn write_error(error: crate::engine::agent_ws::WriteGuardError) -> ErrorData {
    match error {
        crate::engine::agent_ws::WriteGuardError::Conflict(conflict) => ErrorData::invalid_request(
            conflict.to_string(),
            Some(serde_json::json!({
                "type": "stale_write_conflict",
                "path": conflict.target,
                "changed_path": conflict.changed,
                "expected_digest": conflict.expected,
                "actual_digest": conflict.actual,
                "remediation": "Read the current file and retry the operation."
            })),
        ),
        other => ErrorData::internal_error(other.to_string(), None),
    }
}
