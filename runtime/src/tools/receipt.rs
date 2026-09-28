//! Stable, bounded receipts for mutating tools and bounded gate execution.

use std::path::{Component, Path};

use rmcp::handler::server::tool::RequestId;
use rmcp::model::NumberOrString;
use serde::Serialize;

use crate::engine::digest::sha256;

/// The first version of the additive tool-receipt contract.
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;

/// One repository-relative file changed by an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedContent {
    pub path: String,
    pub sha256: String,
}

impl ChangedContent {
    /// Build a change record from the exact bytes that will be written.
    pub fn new(path: impl Into<String>, contents: &[u8]) -> Result<Self, String> {
        let path = normalize_relative_path(path.into())?;
        Ok(Self {
            path,
            sha256: sha256(contents),
        })
    }
}

/// The semantic outcome of a bounded command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Pass,
    Failure,
    Timeout,
}

/// The process exit behavior required by a gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateExpectation {
    Zero,
    Nonzero,
}

/// Structured, command-free evidence from a bounded gate run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateReceipt {
    pub status: GateStatus,
    pub expectation: GateExpectation,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
}

/// The additive receipt included in every successful mutation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationReceipt {
    pub schema_version: u32,
    pub operation: &'static str,
    pub runtime_version: &'static str,
    pub correlation_id: String,
    pub changes: Vec<ChangedContent>,
    pub gate: Option<GateReceipt>,
}

impl OperationReceipt {
    pub fn new(
        operation: &'static str,
        request_id: &RequestId,
        mut changes: Vec<ChangedContent>,
        gate: Option<GateReceipt>,
    ) -> Result<Self, String> {
        changes.sort_by(|left, right| left.path.cmp(&right.path));
        if changes.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err("receipt changes must not contain duplicate paths".to_string());
        }
        Ok(Self {
            schema_version: RECEIPT_SCHEMA_VERSION,
            operation,
            runtime_version: env!("CARGO_PKG_VERSION"),
            correlation_id: correlation_id(request_id),
            changes,
            gate,
        })
    }
}

/// Add a receipt to an existing object-shaped result without changing legacy fields.
pub fn attach(mut result: serde_json::Value, receipt: OperationReceipt) -> serde_json::Value {
    result
        .as_object_mut()
        .expect("tool result payloads with receipts must be JSON objects")
        .insert(
            "receipt".to_string(),
            serde_json::to_value(receipt).expect("receipt serialization is infallible"),
        );
    result
}

fn correlation_id(request_id: &RequestId) -> String {
    let canonical = match &request_id.0 {
        NumberOrString::Number(value) => format!("n:{value}"),
        NumberOrString::String(value) => format!("s:{value}"),
    };
    format!("mcp-sha256:{}", sha256(canonical.as_bytes()))
}

fn normalize_relative_path(path: String) -> Result<String, String> {
    if path.is_empty() || path.contains('\\') {
        return Err("receipt paths must be non-empty repository-relative paths using `/`".into());
    }
    let parsed = Path::new(&path);
    if parsed.is_absolute()
        || parsed.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || parsed
            .components()
            .any(|component| matches!(component, Component::CurDir))
    {
        return Err(format!("receipt path `{path}` must be repository-relative"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn request_id(value: NumberOrString) -> RequestId {
        RequestId(value)
    }

    #[test]
    fn receipt_is_deterministic_and_sorts_changes() {
        let receipt = OperationReceipt::new(
            "test_operation",
            &request_id(NumberOrString::Number(7)),
            vec![
                ChangedContent::new("z.txt", b"z").unwrap(),
                ChangedContent::new(".agent/a.yml", b"a\n").unwrap(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(receipt.changes[0].path, ".agent/a.yml");
        assert_eq!(receipt.changes[1].path, "z.txt");
        assert_eq!(receipt.correlation_id.len(), 75);
    }

    #[test]
    fn correlation_does_not_echo_or_conflate_request_ids() {
        let secret = "secret-token-that-must-not-be-returned";
        let string_id = request_id(NumberOrString::String(Arc::from(secret)));
        let numeric = OperationReceipt::new(
            "test_operation",
            &request_id(NumberOrString::Number(1)),
            Vec::new(),
            None,
        )
        .unwrap();
        let string = OperationReceipt::new(
            "test_operation",
            &request_id(NumberOrString::String(Arc::from("1"))),
            Vec::new(),
            None,
        )
        .unwrap();
        let secret_receipt =
            OperationReceipt::new("test_operation", &string_id, Vec::new(), None).unwrap();
        assert_ne!(numeric.correlation_id, string.correlation_id);
        assert!(!secret_receipt.correlation_id.contains(secret));
        assert_eq!(secret_receipt.correlation_id.len(), 75);
    }

    #[test]
    fn changed_content_rejects_unsafe_or_ambiguous_paths() {
        for path in ["", "/tmp/file", "../file", "a/../file", "a\\file", "./file"] {
            assert!(ChangedContent::new(path, b"body").is_err(), "{path}");
        }
    }

    #[test]
    fn receipt_rejects_duplicate_paths() {
        let change = ChangedContent::new(".agent/tasks/state.yml", b"phase: design\n").unwrap();
        let error = OperationReceipt::new(
            "test_operation",
            &request_id(NumberOrString::Number(1)),
            vec![change.clone(), change],
            None,
        )
        .unwrap_err();
        assert!(error.contains("duplicate"));
    }
}
