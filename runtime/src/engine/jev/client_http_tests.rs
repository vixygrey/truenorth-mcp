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
    let client = super::HttpClient::new_for_test("http://127.0.0.1:1/unused", &fast_config());
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
    let client = super::HttpClient::new_for_test("http://127.0.0.1:1/unused", &fast_config());
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
    let client =
        super::HttpClient::new_for_test(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    assert!(matches!(result, Err(JevError::Unauthorized { .. })));
}

#[tokio::test]
async fn a_422_maps_to_validation_without_provider_field() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(JEV_API_KEY_VAR, "test-key");
    }
    let port = spawn_stub(vec![Canned {
        status: 422,
        body: r#"{"field":"https://secret.example?key=do-not-print"}"#,
    }])
    .await;
    let client =
        super::HttpClient::new_for_test(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    match result {
        Err(JevError::Validation) => {}
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn invalid_response_and_network_error_redact_provider_details() {
    let _guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var(JEV_API_KEY_VAR, "fake-test-key-only") };
    let port = spawn_stub(vec![Canned {
        status: 200,
        body: r#"{"model":"private-provider-field","answers":{"secret":"key"},"usage":{}}"#,
    }])
    .await;
    let client =
        super::HttpClient::new_for_test(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client
        .evaluate(a_request())
        .await
        .expect_err("invalid response");
    let detail = result.to_string();
    assert!(!detail.contains("private-provider-field"));
    assert!(!detail.contains("secret"));
    let client = super::HttpClient::new_for_test(
        "http://127.0.0.1:1/private?key=do-not-print",
        &fast_config(),
    );
    let error = client.evaluate(a_request()).await.expect_err("closed port");
    unsafe { std::env::remove_var(JEV_API_KEY_VAR) };
    assert!(!error.to_string().contains("do-not-print"));
    assert!(!error.to_string().contains("/private"));
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
    let client =
        super::HttpClient::new_for_test(format!("http://127.0.0.1:{port}/jev"), &fast_config());
    let result = client.evaluate(a_request()).await;
    unsafe {
        std::env::remove_var(JEV_API_KEY_VAR);
    }
    let response = result.expect("a 429-then-200 sequence must parse the 200 body");
    assert_eq!(response.model, "jev-latest");
    assert!(response.answers.is_empty());
    assert_eq!(client.attempt_count(), 2);
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
    let client =
        super::HttpClient::new_for_test(format!("http://127.0.0.1:{port}/jev"), &fast_config());
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
    assert_eq!(client.attempt_count(), 3);
}

#[tokio::test]
async fn retry_after_is_bounded_and_entire_call_has_a_timeout() {
    let _guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var(JEV_API_KEY_VAR, "fake-test-key-only") };
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let endpoint = format!("http://{}/jev", listener.local_addr().expect("address"));
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("first request");
        let mut buf = [0u8; 1024];
        assert_ne!(socket.read(&mut buf).await.expect("read request"), 0);
        socket
            .write_all(b"HTTP/1.1 429 STATUS\r\nRetry-After: 99999\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .expect("write retry");
        let (mut socket, _) = listener.accept().await.expect("retry request");
        assert_ne!(socket.read(&mut buf).await.expect("read retry"), 0);
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{OK_BODY}",
                    OK_BODY.len()
                )
                .as_bytes(),
            )
            .await
            .expect("write success");
    });
    let mut config = fast_config();
    config.retry_limit = 1;
    config.max_backoff = Duration::from_millis(10);
    config.timeout = Duration::from_millis(200);
    let result = super::HttpClient::new_for_test(endpoint, &config)
        .evaluate(a_request())
        .await;
    unsafe { std::env::remove_var(JEV_API_KEY_VAR) };
    assert_eq!(
        result.expect("Retry-After capped at 10ms").model,
        "jev-latest"
    );
    server.await.expect("server");
}

#[tokio::test]
async fn timeout_covers_retry_backoff() {
    let _guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var(JEV_API_KEY_VAR, "fake-test-key-only") };
    let port = spawn_stub(vec![Canned {
        status: 429,
        body: "{}",
    }])
    .await;
    let mut config = fast_config();
    config.retry_limit = 1;
    config.max_backoff = Duration::from_millis(500);
    config.timeout = Duration::from_millis(10);
    let result = super::HttpClient::new_for_test(format!("http://127.0.0.1:{port}/jev"), &config)
        .evaluate(a_request())
        .await;
    unsafe { std::env::remove_var(JEV_API_KEY_VAR) };
    assert!(matches!(result, Err(JevError::Timeout { .. })));
}

#[test]
fn map_status_covers_the_mapped_set_under_the_feature() {
    // The mapped statuses under the feature build match the offline property test.
    assert_eq!(map_status(200, JEV_API_KEY_VAR), StatusAction::Parse);
    assert!(matches!(
        map_status(401, JEV_API_KEY_VAR),
        StatusAction::Terminal(JevError::Unauthorized { .. })
    ));
    assert!(matches!(
        map_status(422, JEV_API_KEY_VAR),
        StatusAction::Terminal(JevError::Validation)
    ));
    assert_eq!(map_status(429, JEV_API_KEY_VAR), StatusAction::Retry);
    assert_eq!(map_status(529, JEV_API_KEY_VAR), StatusAction::Retry);
    assert!(matches!(
        map_status(503, JEV_API_KEY_VAR),
        StatusAction::Terminal(JevError::UnexpectedStatus { .. })
    ));
}

#[tokio::test]
async fn guard_path_rejections_send_no_http_request_but_valid_change_is_evaluated() {
    use crate::engine::jev::guard::evaluate::evaluate_guard;
    use crate::engine::jev::guard::{GuardDecision, ProposedChange};

    let _guard = ENV_LOCK.lock().await;
    let repo = tempfile::tempdir().expect("temp repo");
    std::fs::write(repo.path().join("good.rs"), "safe source").expect("file");
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("private.rs"), "private").expect("outside file");
    std::os::unix::fs::symlink(
        outside.path().join("private.rs"),
        repo.path().join("linked.rs"),
    )
    .expect("file symlink");
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener");
    let endpoint = format!("http://{}/jev", listener.local_addr().expect("address"));
    let client = super::HttpClient::new_for_test(endpoint, &fast_config());
    unsafe { std::env::set_var(JEV_API_KEY_VAR, "fake-test-key-only") };
    let protected = Vec::new();
    let config = fast_config();

    for bad in [
        "../outside.rs",
        "/etc/passwd",
        "src\\outside.rs",
        ".env",
        ".agent/telemetry/log.yml",
        "linked.rs",
    ] {
        let change = ProposedChange {
            paths: vec!["good.rs".to_string(), bad.to_string()],
            content: "clean".to_string(),
            task: None,
        };
        let result = evaluate_guard(
            &change,
            &protected,
            &config,
            true,
            true,
            Some(&client),
            repo.path(),
        )
        .await;
        assert!(
            matches!(&result, Err(JevError::InvalidGuardPath { .. })),
            "unsafe {bad} must be rejected: {result:?}"
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(30), listener.accept())
            .await
            .is_err(),
        "no HTTP request may leave the guard for rejected paths"
    );

    let server = tokio::spawn(async move {
        let bodies = [
            serde_json::json!({
                "model": "jev-latest",
                "answers": {
                    "hallucinated_import": {"type": "noul", "noul": 0.0},
                    "violates_conventions": {"type": "noul", "noul": 0.0},
                    "contains_secrets": {"type": "noul", "noul": 0.0},
                    "complexity": {"type": "score", "score": 0.0, "confidence": 0.9,
                                   "legend": {}, "probabilities": {}}
                },
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })
            .to_string(),
            serde_json::json!({
                "model": "jev-latest",
                "answers": {"drift": {"type": "noul", "noul": 0.1}},
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })
            .to_string(),
        ];
        let mut captured = Vec::new();
        for body in bodies {
            use tokio::io::{AsyncBufReadExt, BufReader};

            let (socket, _) = listener.accept().await.expect("valid guard HTTP request");
            let mut socket = BufReader::new(socket);
            let mut length = None;
            loop {
                let mut line = String::new();
                assert_ne!(
                    socket.read_line(&mut line).await.expect("request header"),
                    0
                );
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = Some(value.trim().parse::<usize>().expect("content length"));
                    }
                }
            }
            let mut request = vec![0; length.expect("request has content length")];
            socket
                .read_exact(&mut request)
                .await
                .expect("complete request body");
            captured
                .push(serde_json::from_slice::<serde_json::Value>(&request).expect("request JSON"));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .expect("response");
        }
        captured
    });
    let change = ProposedChange {
        paths: vec!["good.rs".to_string(), "new.rs".to_string()],
        content: "clean".to_string(),
        task: None,
    };
    let result = evaluate_guard(
        &change,
        &protected,
        &config,
        true,
        true,
        Some(&client),
        repo.path(),
    )
    .await;
    unsafe { std::env::remove_var(JEV_API_KEY_VAR) };
    assert_eq!(
        result.expect("valid evaluation"),
        GuardDecision::Allow { notes: Vec::new() }
    );
    let requests = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server served both aspects")
        .expect("server task");
    assert_eq!(
        requests[0]["state"],
        serde_json::json!([{"path": "good.rs", "content": "safe source"}])
    );
    assert_eq!(
        requests[1]["state"]["changed_paths"],
        serde_json::json!(["good.rs", "new.rs"])
    );
}
