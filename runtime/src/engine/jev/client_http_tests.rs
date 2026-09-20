//! Feature-gated transport tests for the HTTP Jev client.
//!
//! Included from `client_http.rs` via `#[path]` behind `#[cfg(all(test, feature =
//! "jev-http"))]`, so `super` is the client_http module. The tests need no external HTTP
//! server: a hand-rolled `tokio` TcpListener returns canned responses on the loopback
//! interface, so the retry loop and the status mapping run against a real socket with no new
//! dependency.
//!
//! Feature: jev-integration-eval, the HTTP client. These tests cover the missing-key path,
//! the mapped statuses, and the 429-then-200 retry loop (Requirement 2.6, 9.4, 10.1, 10.2,
//! 10.5).

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use super::{JEV_API_KEY_VAR, StatusAction, map_status};
use crate::engine::jev::config::JevConfig;
use crate::engine::jev::{JevClient, JevError, JevRequest};

/// A process-wide async lock that serializes the env-var tests. The API-key variable is
/// process global, so two tests that set or clear it must not overlap. An async-aware mutex
/// can be held across an `await`, so a test sets the key, runs the call, and clears the key
/// under one guard with no blocking-guard-across-await hazard.
static ENV_LOCK: Mutex<()> = Mutex::const_new(());

/// A canned HTTP response body and status a stub returns.
struct Canned {
    /// The HTTP status line code.
    status: u16,
    /// The response body.
    body: &'static str,
}

/// Serve one queued canned response per connection, in order, then stop.
///
/// The stub binds a loopback port, returns each queued response on its own connection, and
/// returns the bound port. It reads and discards the request bytes before it replies, so the
/// client's POST completes.
async fn spawn_stub(responses: Vec<Canned>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let port = listener.local_addr().expect("local addr").port();
    tokio::spawn(async move {
        for canned in responses {
            let (mut socket, _) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => return,
            };
            // Read the request head so the client's write completes. A small read is enough;
            // the client does not need the body echoed.
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let reason = if canned.status == 200 { "OK" } else { "STATUS" };
            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                canned.status,
                reason,
                canned.body.len(),
                canned.body,
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    port
}

/// A config with a short timeout and a small backoff, so the retry test runs fast.
fn fast_config() -> JevConfig {
    JevConfig {
        retry_limit: 2,
        max_backoff: Duration::from_millis(10),
        timeout: Duration::from_secs(5),
        ..JevConfig::default()
    }
}

/// A minimal request the stub does not inspect.
fn a_request() -> JevRequest {
    JevRequest::new(
        serde_json::json!("state"),
        std::collections::BTreeMap::new(),
    )
}

/// A well-formed JevResponse body the client can parse on a 200.
const OK_BODY: &str =
    r#"{"model":"jev-latest","answers":{},"usage":{"input_tokens":1,"output_tokens":0}}"#;

#[tokio::test]
async fn missing_key_returns_missing_api_key_and_makes_no_request() {
    let _guard = ENV_LOCK.lock().await;
    // Clear the key so the client makes no request. The endpoint is a black-hole port that
    // would fail loudly if the client tried to reach it.
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    let client = super::HttpClient::new("http://127.0.0.1:1/unused", &fast_config());
    let result = client.evaluate(a_request()).await;
    match result {
        Err(JevError::MissingApiKey { var }) => assert_eq!(var, JEV_API_KEY_VAR),
        other => panic!("expected MissingApiKey, got {other:?}"),
    }
}

#[tokio::test]
async fn empty_key_returns_missing_api_key() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(JEV_API_KEY_VAR, "");
    }
    let client = super::HttpClient::new("http://127.0.0.1:1/unused", &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    assert!(matches!(result, Err(JevError::MissingApiKey { .. })));
}

#[tokio::test]
async fn a_401_maps_to_unauthorized() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(JEV_API_KEY_VAR, "test-key");
    }
    let port = spawn_stub(vec![Canned {
        status: 401,
        body: "{}",
    }])
    .await;
    let client = super::HttpClient::new(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    assert!(matches!(result, Err(JevError::Unauthorized { .. })));
}

#[tokio::test]
async fn a_422_maps_to_validation_with_the_body_field() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(JEV_API_KEY_VAR, "test-key");
    }
    let port = spawn_stub(vec![Canned {
        status: 422,
        body: r#"{"field":"questions"}"#,
    }])
    .await;
    let client = super::HttpClient::new(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    match result {
        Err(JevError::Validation { field }) => assert_eq!(field, "questions"),
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn a_429_then_200_retries_and_parses() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(JEV_API_KEY_VAR, "test-key");
    }
    // First connection returns 429, the second returns a parseable 200. The client must
    // retry once and then parse.
    let port = spawn_stub(vec![
        Canned {
            status: 429,
            body: "{}",
        },
        Canned {
            status: 200,
            body: OK_BODY,
        },
    ])
    .await;
    let client = super::HttpClient::new(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    let response = result.expect("a 429-then-200 sequence must parse the 200 body");
    assert_eq!(response.model, "jev-latest");
    assert!(response.answers.is_empty());
}

#[tokio::test]
async fn exhausted_retries_return_rate_limited_or_overloaded() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(JEV_API_KEY_VAR, "test-key");
    }
    // Every attempt returns 429. With retry_limit 2, the client makes 3 attempts, so the
    // stub queues 3 responses. The client then returns RateLimitedOrOverloaded.
    let port = spawn_stub(vec![
        Canned {
            status: 429,
            body: "{}",
        },
        Canned {
            status: 429,
            body: "{}",
        },
        Canned {
            status: 429,
            body: "{}",
        },
    ])
    .await;
    let client = super::HttpClient::new(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    match result {
        Err(JevError::RateLimitedOrOverloaded {
            last_status,
            attempts,
        }) => {
            assert_eq!(last_status, 429);
            assert_eq!(attempts, 3);
        }
        other => panic!("expected RateLimitedOrOverloaded, got {other:?}"),
    }
}

#[test]
fn map_status_covers_the_mapped_set_under_the_feature() {
    // The mapped statuses under the feature build match the offline property test.
    assert_eq!(map_status(200, JEV_API_KEY_VAR, None), StatusAction::Parse);
    assert!(matches!(
        map_status(401, JEV_API_KEY_VAR, None),
        StatusAction::Terminal(JevError::Unauthorized { .. })
    ));
    assert!(matches!(
        map_status(422, JEV_API_KEY_VAR, None),
        StatusAction::Terminal(JevError::Validation { .. })
    ));
    assert_eq!(map_status(429, JEV_API_KEY_VAR, None), StatusAction::Retry);
    assert_eq!(map_status(529, JEV_API_KEY_VAR, None), StatusAction::Retry);
    assert!(matches!(
        map_status(503, JEV_API_KEY_VAR, None),
        StatusAction::Terminal(JevError::UnexpectedStatus { .. })
    ));
}
