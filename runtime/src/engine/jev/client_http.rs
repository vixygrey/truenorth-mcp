//! The real HTTP Jev client and the pure status mapper.
//!
//! This module holds two layers. The pure status mapper ([`map_status`] and
//! [`StatusAction`]) is always compiled, so its property test runs offline with no
//! networking feature. The transport type ([`HttpClient`]) sits behind the `jev-http`
//! feature, so the default build and the offline test suite compile no reqwest client and
//! make no network call (ADR-J1).
//!
//! The mapper turns one HTTP status into one decision: parse the body, retry with backoff,
//! or return a typed terminal error. The transport reads the API key from the environment
//! at call time, builds one authenticated POST, applies the wall-clock timeout, and retries
//! a rate-limited or overloaded status up to the configured limit. The transport never
//! writes, echoes, or logs the API key, and no error detail carries the key (Requirement
//! 9.5, 9.6).
//!
//! Requirements: 2.5, 2.6, 2.8, 2.9, 2.10, 9.4, 9.5, 9.6, 10.1, 10.2, 10.3, 10.4, 10.5,
//! 10.6, 10.7. Design: jev-integration-eval, the HTTP client, Property 32 (error non-panic).

use super::JevError;

/// The environment variable that holds the Jev API key (Requirement 2.6, 9.4).
///
/// The transport reads this variable at call time and names it in the [`JevError::MissingApiKey`]
/// and [`JevError::Unauthorized`] messages. The const is always compiled, because the pure
/// mapper and the [`JevError::MissingApiKey`] construction both name it. Under the default
/// build the transport is absent, so the only callers are tests, and the const carries a
/// narrow non-test `allow` naming the future consumer.
///
/// The first non-test caller is the [`HttpClient`] transport under the `jev-http` feature
/// and the benchmark bin (task 12). Under the default build only tests reference it, so it
/// carries a narrow non-test `allow`, per the repo dead-code policy (main.rs).
#[cfg_attr(not(test), allow(dead_code))]
pub const JEV_API_KEY_VAR: &str = "TRUENORTH_JEV_API_KEY";

/// The decision for one HTTP status: parse the body, retry, or a terminal error.
///
/// The mapper returns this decision so the transport stays a thin dispatcher. The pure
/// mapping is total and panic-free, so an offline property test can prove it covers every
/// `u16` (Property 32).
///
/// The first non-test caller is the [`HttpClient`] transport under the `jev-http` feature
/// and the benchmark bin (task 12). Under the default build only the property test
/// references it, so it carries a narrow non-test `allow`, per the repo dead-code policy.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub enum StatusAction {
    /// 200: parse the response body into a [`super::JevResponse`].
    Parse,
    /// 429 or 529: retry with exponential backoff.
    Retry,
    /// 401, 422, or any other status: a typed terminal error.
    Terminal(JevError),
}

/// Map one HTTP status to a [`StatusAction`] (Requirement 10.1, 10.2, 10.6, 10.7).
///
/// The mapping is total and panic-free:
///
/// - 200 returns [`StatusAction::Parse`].
/// - 401 returns [`StatusAction::Terminal`] with [`JevError::Unauthorized`].
/// - 422 returns [`StatusAction::Terminal`] with [`JevError::Validation`]. The field name
///   comes from `validation_field`, or `"unknown"` when the caller extracted none.
/// - 429 or 529 returns [`StatusAction::Retry`].
/// - Any other status returns [`StatusAction::Terminal`] with [`JevError::UnexpectedStatus`].
///
/// The `var` names the API-key environment variable for the 401 message. The mapper never
/// panics and never calls `unwrap`, so the transport can rely on a decision for every status.
///
/// The first non-test caller is the [`HttpClient`] transport under the `jev-http` feature
/// and the benchmark bin (task 12). Under the default build only the property test
/// references it, so it carries a narrow non-test `allow`, per the repo dead-code policy.
#[cfg_attr(not(test), allow(dead_code))]
pub fn map_status(
    status: u16,
    var: &'static str,
    validation_field: Option<String>,
) -> StatusAction {
    match status {
        200 => StatusAction::Parse,
        401 => StatusAction::Terminal(JevError::Unauthorized { var }),
        422 => StatusAction::Terminal(JevError::Validation {
            field: validation_field.unwrap_or_else(|| "unknown".to_string()),
        }),
        429 | 529 => StatusAction::Retry,
        other => StatusAction::Terminal(JevError::UnexpectedStatus { status: other }),
    }
}

// The real transport pulls in reqwest, so it sits behind the `jev-http` feature. The module
// below compiles only under the feature, and its tests are `#[cfg(all(test, feature =
// "jev-http"))]`.
#[cfg(feature = "jev-http")]
mod transport {
    use std::time::Duration;

    use serde::Deserialize;

    use super::{JEV_API_KEY_VAR, StatusAction, map_status};
    use crate::engine::jev::config::JevConfig;
    use crate::engine::jev::{JevClient, JevError, JevRequest, JevResponse};

    /// The rate-limited status (429), the initial `last_status` before any call. The 429
    /// and 529 statuses share the retry path in [`map_status`] (Requirement 10.3).
    const STATUS_RATE_LIMITED: u16 = 429;
    /// The base backoff before the first retry, doubled on each further retry.
    const BASE_BACKOFF: Duration = Duration::from_millis(500);

    /// The real HTTP Jev client (Requirement 2.9, 2.10).
    ///
    /// The client holds the endpoint, a configured reqwest client, the retry and backoff
    /// limits, the call timeout, and the API-key variable name. It reads the key from the
    /// environment on each call, so a rotated key takes effect with no rebuild. It never
    /// stores, writes, echoes, or logs the key (Requirement 9.5, 9.6).
    ///
    /// The transport has no non-test caller until the benchmark bin lands (task 12), so it
    /// carries a narrow non-test `allow` naming that consumer, per the repo dead-code
    /// policy (main.rs). The feature-gated test build exercises the missing-key path, the
    /// mapped statuses, and the retry loop.
    #[cfg_attr(not(test), allow(dead_code))]
    pub struct HttpClient {
        /// The Jev endpoint URL the POST targets.
        endpoint: String,
        /// The configured reqwest client, built with the call timeout.
        http: reqwest::Client,
        /// The retry limit for a rate-limited or overloaded status.
        retry_limit: u32,
        /// The cap on each backoff sleep.
        max_backoff: Duration,
        /// The per-call wall-clock timeout.
        timeout: Duration,
        /// The API-key environment variable name.
        api_key_var: &'static str,
    }

    /// The 422 error body shape, read best-effort to name the offending field.
    ///
    /// The endpoint can return a JSON body naming the invalid field. The transport reads
    /// this shape best-effort; an absent or unparsable body leaves the field as `None`, so
    /// the mapper substitutes `"unknown"`.
    #[derive(Debug, Deserialize)]
    struct ValidationBody {
        /// The offending field name, when the body carries one.
        field: Option<String>,
    }

    // The inherent methods below have no non-test caller until the benchmark bin lands
    // (task 12). They carry a narrow non-test `allow`, per the repo dead-code policy.
    #[cfg_attr(not(test), allow(dead_code))]
    impl HttpClient {
        /// Build a client for `endpoint` from the resolved config (Requirement 2.5).
        ///
        /// The reqwest client is built with the config timeout, so a slow call is cut at the
        /// wall-clock bound. A client build failure falls back to the default client, so the
        /// constructor never panics.
        pub fn new(endpoint: impl Into<String>, config: &JevConfig) -> Self {
            let http = reqwest::Client::builder()
                .timeout(config.timeout)
                .build()
                .unwrap_or_default();
            Self {
                endpoint: endpoint.into(),
                http,
                retry_limit: config.retry_limit,
                max_backoff: config.max_backoff,
                timeout: config.timeout,
                api_key_var: JEV_API_KEY_VAR,
            }
        }

        /// The configured timeout in milliseconds, for a [`JevError::Timeout`] message.
        fn timeout_ms(&self) -> u64 {
            u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX)
        }

        /// The backoff for retry attempt `attempt`, capped at [`HttpClient::max_backoff`].
        ///
        /// The backoff doubles from [`BASE_BACKOFF`] on each attempt. The cap holds the sleep
        /// at the configured maximum, so a high retry count does not sleep without bound.
        fn backoff_for(&self, attempt: u32) -> Duration {
            let factor = 2u32.saturating_pow(attempt);
            let scaled = BASE_BACKOFF.checked_mul(factor).unwrap_or(self.max_backoff);
            scaled.min(self.max_backoff)
        }

        /// Send one request and return the raw status and body text.
        ///
        /// A send error that is a timeout maps to [`JevError::Timeout`]. Any other send error
        /// maps to [`JevError::Network`]. The `key` is never placed in an error detail, so no
        /// message leaks it (Requirement 9.6).
        async fn send_once(
            &self,
            key: &str,
            request: &JevRequest,
        ) -> Result<(u16, String), JevError> {
            let response = self
                .http
                .post(&self.endpoint)
                .bearer_auth(key)
                .json(request)
                .send()
                .await
                .map_err(|error| self.map_send_error(error))?;

            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|error| self.map_send_error(error))?;
            Ok((status, body))
        }

        /// Map a reqwest send error to a typed [`JevError`], never leaking the key.
        fn map_send_error(&self, error: reqwest::Error) -> JevError {
            if error.is_timeout() {
                return JevError::Timeout {
                    timeout_ms: self.timeout_ms(),
                };
            }
            // reqwest's Display can name the URL, but never the bearer token, so the detail
            // carries no secret (Requirement 9.6).
            JevError::Network {
                detail: error.to_string(),
            }
        }

        /// Parse a 200 body into a [`JevResponse`], mapping a deser error to [`JevError::Network`].
        fn parse_body(body: &str) -> Result<JevResponse, JevError> {
            serde_json::from_str::<JevResponse>(body).map_err(|error| JevError::Network {
                detail: format!("could not parse the Jev response body: {error}"),
            })
        }

        /// Extract the offending field name from a 422 body, best-effort.
        fn field_from_body(body: &str) -> Option<String> {
            serde_json::from_str::<ValidationBody>(body)
                .ok()
                .and_then(|parsed| parsed.field)
        }
    }

    impl JevClient for HttpClient {
        async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError> {
            // Read the key at call time. An absent or empty key makes no request
            // (Requirement 2.6, 9.4).
            let key = match std::env::var(self.api_key_var) {
                Ok(value) if !value.is_empty() => value,
                _ => {
                    return Err(JevError::MissingApiKey {
                        var: self.api_key_var,
                    });
                }
            };

            let mut last_status = STATUS_RATE_LIMITED;
            // One initial attempt plus `retry_limit` retries.
            for attempt in 0..=self.retry_limit {
                let (status, body) = self.send_once(&key, &request).await?;
                let field = if status == 422 {
                    Self::field_from_body(&body)
                } else {
                    None
                };
                match map_status(status, self.api_key_var, field) {
                    StatusAction::Parse => return Self::parse_body(&body),
                    StatusAction::Terminal(error) => return Err(error),
                    StatusAction::Retry => {
                        last_status = status;
                        if attempt < self.retry_limit {
                            let sleep = self.backoff_for(attempt);
                            tokio::time::sleep(sleep).await;
                        }
                    }
                }
            }

            // Every attempt returned a retriable status (Requirement 10.5).
            Err(JevError::RateLimitedOrOverloaded {
                last_status,
                attempts: self.retry_limit + 1,
            })
        }
    }
}

// Re-export the transport at the module root, so a caller uses `client_http::HttpClient`.
// The first non-test caller is the benchmark bin (task 12); under the feature build with no
// bench, only the feature-gated tests reference it, so the re-export carries a narrow
// non-test `allow`, per the repo dead-code policy (main.rs).
#[cfg(feature = "jev-http")]
#[cfg_attr(not(test), allow(unused_imports))]
pub use transport::HttpClient;

// The Property 32 test is always compiled and runs offline, because the mapper is always
// compiled. The `#[path]` include keeps it a child module of `client_http`.
#[cfg(test)]
#[path = "client_http_prop_tests.rs"]
mod prop_tests;

// The transport tests are feature-gated, because they touch the feature-gated transport.
#[cfg(all(test, feature = "jev-http"))]
#[path = "client_http_tests.rs"]
mod tests;
