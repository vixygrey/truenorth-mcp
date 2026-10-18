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

/// The environment variable that holds the Jev endpoint URL.
pub const JEV_ENDPOINT_VAR: &str = "TRUENORTH_JEV_ENDPOINT";

/// The environment variable that holds the Jev API key (Requirement 2.6, 9.4).
pub const JEV_API_KEY_VAR: &str = "TRUENORTH_JEV_API_KEY";

/// Validated operator configuration for one Jev HTTP client.
///
/// The activation retains only the endpoint. The API key is checked during activation and read
/// again for each request, so key rotation takes effect without storing a secret in the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JevActivation {
    endpoint: String,
}

impl JevActivation {
    /// Resolve the endpoint and API key from the operator environment.
    pub fn resolve() -> Result<Self, JevError> {
        let endpoint = std::env::var(JEV_ENDPOINT_VAR).ok();
        let api_key = std::env::var(JEV_API_KEY_VAR).ok();
        Self::from_values(endpoint.as_deref(), api_key.as_deref())
    }

    /// Validate the supplied environment values without retaining the API key.
    pub fn from_values(endpoint: Option<&str>, api_key: Option<&str>) -> Result<Self, JevError> {
        let endpoint =
            endpoint
                .filter(|value| !value.is_empty())
                .ok_or(JevError::MissingEndpoint {
                    var: JEV_ENDPOINT_VAR,
                })?;
        if !is_valid_https_endpoint(endpoint) {
            return Err(JevError::InvalidEndpoint {
                var: JEV_ENDPOINT_VAR,
            });
        }
        if api_key.is_none_or(|value| value.is_empty()) {
            return Err(JevError::MissingApiKey {
                var: JEV_API_KEY_VAR,
            });
        }
        Ok(Self {
            endpoint: endpoint.to_string(),
        })
    }

    /// Return the validated endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[cfg(feature = "jev-http")]
    fn into_endpoint(self) -> String {
        self.endpoint
    }
}

/// Accept a conservative absolute HTTPS endpoint without credentials, query data, or fragments.
fn is_valid_https_endpoint(endpoint: &str) -> bool {
    if endpoint
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return false;
    }
    let Some(remainder) = endpoint.strip_prefix("https://") else {
        return false;
    };
    if remainder.is_empty() || remainder.contains(['?', '#']) {
        return false;
    }
    let (authority, _) = remainder.split_once('/').unwrap_or((remainder, ""));
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    if host.contains(':') || !is_valid_dns_host(host) {
        return false;
    }
    port.is_none_or(|value| value.parse::<u16>().is_ok_and(|parsed| parsed != 0))
}

/// Accept an ASCII DNS host. This intentionally rejects IP literals and internationalized names.
fn is_valid_dns_host(host: &str) -> bool {
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}
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
/// - 422 returns [`StatusAction::Terminal`] with a redacted [`JevError::Validation`].
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
pub fn map_status(status: u16, var: &'static str) -> StatusAction {
    match status {
        200 => StatusAction::Parse,
        401 => StatusAction::Terminal(JevError::Unauthorized { var }),
        422 => StatusAction::Terminal(JevError::Validation),
        429 | 529 => StatusAction::Retry,
        other => StatusAction::Terminal(JevError::UnexpectedStatus { status: other }),
    }
}

// The real transport pulls in reqwest, so it sits behind the `jev-http` feature. The module
// below compiles only under the feature, and its tests are `#[cfg(all(test, feature =
// "jev-http"))]`.
#[cfg(feature = "jev-http")]
mod transport {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::{JEV_API_KEY_VAR, JevActivation, StatusAction, map_status};
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
    pub struct HttpClient {
        /// The Jev endpoint URL the POST targets.
        endpoint: String,
        /// The configured client, absent if its secure construction failed.
        http: Option<reqwest::Client>,
        /// The retry limit for a rate-limited or overloaded status.
        retry_limit: u32,
        /// The cap on each backoff sleep.
        max_backoff: Duration,
        /// The per-call wall-clock timeout.
        timeout: Duration,
        /// The API-key environment variable name.
        api_key_var: &'static str,
        /// Cumulative wire send attempts (not logical evaluations).
        attempts: AtomicU64,
    }

    impl HttpClient {
        /// Build a client from validated operator activation and the resolved configuration.
        ///
        /// The reqwest client is built with the config timeout and no redirects.
        /// Construction failure leaves the client unable to send rather than falling back
        /// to insecure defaults.
        pub fn new(activation: JevActivation, config: &JevConfig) -> Self {
            Self::from_endpoint(activation.into_endpoint(), config)
        }

        /// Build a loopback client for transport tests only.
        #[cfg(test)]
        pub fn new_for_test(endpoint: impl Into<String>, config: &JevConfig) -> Self {
            Self::from_endpoint(endpoint.into(), config)
        }

        fn from_endpoint(endpoint: String, config: &JevConfig) -> Self {
            let http = reqwest::Client::builder()
                .timeout(config.timeout)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .ok();
            Self {
                endpoint,
                http,
                retry_limit: config.retry_limit,
                max_backoff: config.max_backoff,
                timeout: config.timeout,
                api_key_var: JEV_API_KEY_VAR,
                attempts: AtomicU64::new(0),
            }
        }

        /// Count every HTTP send, including retries and attempts without usage.
        pub fn attempt_count(&self) -> u64 {
            self.attempts.load(Ordering::Relaxed)
        }

        /// The configured timeout in milliseconds, for a [`JevError::Timeout`] message.
        fn timeout_ms(&self) -> u64 {
            u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX)
        }
        /// Bound the retry delay by the operator-configured maximum.
        fn backoff_for(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
            let factor = 2u32.saturating_pow(attempt);
            let scaled = BASE_BACKOFF.checked_mul(factor).unwrap_or(self.max_backoff);
            retry_after
                .map_or(scaled, |delay| delay.max(scaled))
                .min(self.max_backoff)
        }

        /// Send one request, retaining only its status, bounded retry hint and body.
        async fn send_once(
            &self,
            key: &str,
            request: &JevRequest,
        ) -> Result<(u16, Option<Duration>, String), JevError> {
            let http = self.http.as_ref().ok_or_else(|| JevError::Network {
                detail: "transport unavailable (details redacted)".to_string(),
            })?;
            self.attempts.fetch_add(1, Ordering::Relaxed);
            let response = http
                .post(&self.endpoint)
                .bearer_auth(key)
                .json(request)
                .send()
                .await
                .map_err(|error| self.map_send_error(error))?;

            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            let body = if status == 200 {
                response
                    .text()
                    .await
                    .map_err(|error| self.map_send_error(error))?
            } else {
                String::new()
            };
            Ok((status, retry_after, body))
        }

        /// Map a reqwest send error to a typed [`JevError`], never leaking the key.
        fn map_send_error(&self, error: reqwest::Error) -> JevError {
            if error.is_timeout() {
                return JevError::Timeout {
                    timeout_ms: self.timeout_ms(),
                };
            }
            // reqwest errors can include the URL, including its path or credentials.
            JevError::Network {
                detail: "transport failure (details redacted)".to_string(),
            }
        }

        /// Never include the provider's response bytes or field names in diagnostics.
        fn parse_body(body: &str) -> Result<JevResponse, JevError> {
            serde_json::from_str::<JevResponse>(body).map_err(|_| JevError::Network {
                detail: "invalid Jev response (details redacted)".to_string(),
            })
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

            let call = async {
                let mut last_status = STATUS_RATE_LIMITED;
                // One initial attempt plus `retry_limit` retries.
                for attempt in 0..=self.retry_limit {
                    let (status, retry_after, body) = self.send_once(&key, &request).await?;
                    match map_status(status, self.api_key_var) {
                        StatusAction::Parse => return Self::parse_body(&body),
                        StatusAction::Terminal(error) => return Err(error),
                        StatusAction::Retry => {
                            last_status = status;
                            if attempt < self.retry_limit {
                                tokio::time::sleep(self.backoff_for(attempt, retry_after)).await;
                            }
                        }
                    }
                }
                Err(JevError::RateLimitedOrOverloaded {
                    last_status,
                    attempts: self.retry_limit.saturating_add(1),
                })
            };
            tokio::time::timeout(self.timeout, call)
                .await
                .unwrap_or(Err(JevError::Timeout {
                    timeout_ms: self.timeout_ms(),
                }))
        }
    }
}

// Re-export the transport at the module root, so a caller uses `client_http::HttpClient`.
#[cfg(feature = "jev-http")]
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
