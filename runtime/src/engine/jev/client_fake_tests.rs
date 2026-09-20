//! Example tests for the named fake Jev client.
//!
//! Included from `client_fake.rs` via `#[path]`, so `super` is the client_fake module.
//! These pin the queue behavior, the call recording, the error replay, and the
//! empty-queue guard (Requirement 2.2, 2.4).

use std::collections::BTreeMap;

use super::super::{Answer, JevRequest, JevResponse, Usage};
use super::*;

/// Build a minimal successful response carrying one Noul answer.
fn noul_response(noul: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("q".to_string(), Answer::Noul { noul })]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    }
}

#[tokio::test]
async fn a_queued_response_is_returned_in_order() {
    let fake = FakeClient::new();
    fake.push_response(noul_response(0.1));
    fake.push_response(noul_response(0.9));

    let first = fake
        .evaluate(JevRequest::new(serde_json::json!("a"), BTreeMap::new()))
        .await
        .expect("first response");
    let second = fake
        .evaluate(JevRequest::new(serde_json::json!("b"), BTreeMap::new()))
        .await
        .expect("second response");

    assert_eq!(first.answers.get("q"), Some(&Answer::Noul { noul: 0.1 }));
    assert_eq!(second.answers.get("q"), Some(&Answer::Noul { noul: 0.9 }));
}

#[tokio::test]
async fn calls_are_recorded_in_order() {
    let fake = FakeClient::new();
    fake.push_response(noul_response(0.5));
    fake.push_response(noul_response(0.5));

    assert_eq!(fake.call_count(), 0, "no calls before evaluate");
    let _ = fake
        .evaluate(JevRequest::new(serde_json::json!("first"), BTreeMap::new()))
        .await;
    let _ = fake
        .evaluate(JevRequest::new(
            serde_json::json!("second"),
            BTreeMap::new(),
        ))
        .await;

    assert_eq!(fake.call_count(), 2);
    let recorded = fake.calls();
    assert_eq!(recorded[0].state, serde_json::json!("first"));
    assert_eq!(recorded[1].state, serde_json::json!("second"));
}

#[tokio::test]
async fn a_queued_error_replays_as_the_typed_error() {
    let fake = FakeClient::new();
    fake.push_error(FakeError::SecretResidual);
    fake.push_error(FakeError::Timeout { timeout_ms: 30_000 });

    let residual = fake
        .evaluate(JevRequest::new(serde_json::json!("a"), BTreeMap::new()))
        .await
        .expect_err("residual error");
    assert!(matches!(residual, JevError::SecretResidual));

    let timeout = fake
        .evaluate(JevRequest::new(serde_json::json!("b"), BTreeMap::new()))
        .await
        .expect_err("timeout error");
    assert!(matches!(timeout, JevError::Timeout { timeout_ms: 30_000 }));

    fake.push_error(FakeError::Network {
        detail: "connection reset".to_string(),
    });
    let network = fake
        .evaluate(JevRequest::new(serde_json::json!("c"), BTreeMap::new()))
        .await
        .expect_err("network error");
    match network {
        JevError::Network { detail } => assert_eq!(detail, "connection reset"),
        other => panic!("expected a Network error, got {other:?}"),
    }
}

#[tokio::test]
async fn an_empty_queue_fails_loudly() {
    let fake = FakeClient::new();
    let error = fake
        .evaluate(JevRequest::new(serde_json::json!("a"), BTreeMap::new()))
        .await
        .expect_err("empty queue errors");
    match error {
        JevError::Network { detail } => assert!(detail.contains("empty")),
        other => panic!("expected a Network error naming the empty queue, got {other:?}"),
    }
    // The call is still recorded, so a test can see the over-call.
    assert_eq!(fake.call_count(), 1);
}
