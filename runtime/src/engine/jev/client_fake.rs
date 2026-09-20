//! The named fake Jev client: a test-support type that satisfies [`super::JevClient`]
//! with no network call.
//!
//! The fake returns a queued response and records every request, so a test can drive an
//! aspect module offline and assert what the harness sent. The property tests use the
//! recorded call count to prove the flag-off silence property: while the Jev feature is
//! off, the harness makes no call, so the fake records none (Property 22).
//!
//! The fake is a named type, not an inline stub, per the styleguide. It has no non-test
//! caller until the benchmark bin lands (a later issue), so each public item carries a
//! narrow non-test `allow` with this reason. The task that wires the benchmark removes the
//! attributes. The test build exercises the fake through the sibling tests and the aspect
//! tests.
//!
//! Requirements: 2.2, 2.4, 1.4, 1.5. Design: jev-integration-eval, ADR-J2, the named fake,
//! Property 22 (flag-off silence).

use std::sync::Mutex;

use super::{JevClient, JevError, JevRequest, JevResponse};

/// A named fake [`super::JevClient`] for offline tests and the benchmark.
///
/// The fake holds a queue of responses. Each call to [`JevClient::evaluate`] pops the next
/// queued response and records the request. When the queue is empty, the call returns a
/// configured fallback error, so a test that under-queues fails loudly rather than hangs.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Default)]
pub struct FakeClient {
    /// The queued responses, popped front to back on each call.
    queue: Mutex<std::collections::VecDeque<Result<JevResponse, FakeError>>>,
    /// The requests the fake received, in call order.
    calls: Mutex<Vec<JevRequest>>,
}

/// A cloneable stand-in for the errors the fake can replay.
///
/// [`JevError`] is not `Clone`, and the fake must replay a queued outcome, so the fake
/// stores this small cloneable enum and converts it to a [`JevError`] on return. The set
/// covers the outcomes an aspect test needs to drive.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub enum FakeError {
    /// Replay a residual-secret rejection.
    SecretResidual,
    /// Replay a timeout with the given millisecond bound.
    Timeout {
        /// The timeout in milliseconds.
        timeout_ms: u64,
    },
    /// Replay a network failure with the given detail.
    Network {
        /// The failure detail.
        detail: String,
    },
}

impl From<FakeError> for JevError {
    fn from(error: FakeError) -> Self {
        match error {
            FakeError::SecretResidual => JevError::SecretResidual,
            FakeError::Timeout { timeout_ms } => JevError::Timeout { timeout_ms },
            FakeError::Network { detail } => JevError::Network { detail },
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl FakeClient {
    /// Build an empty fake with no queued response.
    ///
    /// A call against an empty queue returns a network error naming the empty queue, so an
    /// under-queued test fails with a clear message rather than a hang.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue one successful response, returned by the next call.
    pub fn push_response(&self, response: JevResponse) {
        lock(&self.queue).push_back(Ok(response));
    }

    /// Queue one error outcome, returned by the next call.
    pub fn push_error(&self, error: FakeError) {
        lock(&self.queue).push_back(Err(error));
    }

    /// The number of calls the fake has received.
    ///
    /// The flag-off silence property asserts this is zero while the Jev feature is off
    /// (Property 22).
    pub fn call_count(&self) -> usize {
        lock(&self.calls).len()
    }

    /// The requests the fake received, in call order, cloned for inspection.
    pub fn calls(&self) -> Vec<JevRequest> {
        lock(&self.calls).clone()
    }
}

/// Lock a mutex, recovering the guard from a poisoned lock rather than panicking.
///
/// A poisoned lock means a prior holder panicked. The fake holds only plain data, so the
/// data is still consistent, and recovering the guard keeps the fake panic-free in library
/// code (styleguide: no `unwrap`, `expect`, or `panic` outside tests).
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl JevClient for FakeClient {
    async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError> {
        lock(&self.calls).push(request);
        match lock(&self.queue).pop_front() {
            Some(Ok(response)) => Ok(response),
            Some(Err(error)) => Err(error.into()),
            None => Err(JevError::Network {
                detail: "the fake client queue is empty; queue a response before the call"
                    .to_string(),
            }),
        }
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `client_fake`.
#[cfg(test)]
#[path = "client_fake_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "client_fake_prop_tests.rs"]
mod prop_tests;
