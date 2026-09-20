//! The Jev evaluation harness: a project-owned client trait, wire types, aspect
//! modules, a named fake, and a benchmark runner for the TypeSafe Jev model.
//!
//! The harness measures whether Jev fits five product aspects: tool routing, parallel
//! rigor scoring, drift guardrails, context pruning, and self-healing decisions. It is
//! engine infrastructure, not an MCP tool. It registers nothing into the tool router, no
//! gate, and no resource (jev-integration-eval design, Overview).
//!
//! The harness sits behind two independent opt-in layers (ADR-J1). The runtime `jev`
//! flag in `.agent/config/rules.yml` gates behavior; it is off by default, so a project
//! that does not opt in builds no state and makes no call. The Cargo `jev-http` feature
//! gates the networking dependency and the real HTTP client; the default build and the
//! offline test suite compile no HTTP client, so `cargo test` runs the full suite against
//! the named fake with no network call.
//!
//! This module holds the trait layer: the [`JevClient`] trait, the wire types
//! ([`JevRequest`], [`Question`], [`JevResponse`], [`Answer`], [`Usage`]), the shared
//! constants, and the [`JevError`] enum. The whole trait layer compiles with no
//! networking feature, so the aspect modules and the named fake build offline. The
//! client layer and the aspect modules land in later tasks (issues #265 onward).
//!
//! The design prose names the trait `Jev_Client`. Rust API Guidelines require a
//! `CamelCase` trait name, and the styleguide mandates those guidelines, so the trait is
//! `JevClient` in code. The name is the only deviation from the design's spelling.
//!
//! Requirements: 2.1, 2.6, 2.8, 2.10, 4.10, 6.5, 9.4, 9.6, 10.1, 10.2, 10.5.
//! Design: jev-integration-eval, ADR-J1, ADR-J2, the trait and wire types, error handling.
//!
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod client_fake;
pub mod client_http;
pub mod confidence;
pub mod config;
pub mod drift;
pub mod pruning;
pub mod rigor;
pub mod routing;
pub mod secret_filter;
pub mod self_heal;

// The trait layer below has no non-test consumer until the client and aspect modules land
// (issues #266 onward). Each public item carries a narrow non-test `allow` with this
// reason, per the repo dead-code policy (main.rs). The task that adds each item's first
// caller removes its attribute. The test build exercises every item through `mod_tests.rs`.

/// The model id every request carries (Requirement 2.10).
#[cfg_attr(not(test), allow(dead_code))]
pub const JEV_MODEL: &str = "jev-latest";

/// The shared request token budget, in tokens (Requirement 4.10, 6.5).
///
/// The `state` and the `questions` share this budget. A built request that estimates over
/// it is rejected before the call (task 5, Property 33).
#[cfg_attr(not(test), allow(dead_code))]
pub const TOKEN_BUDGET: usize = 32_000;

/// The average bytes per token for the conservative token estimate (Requirement 4.10).
///
/// The exact Jev tokenizer is closed, so the harness estimates tokens from serialized
/// bytes. This divisor is an approximation, not the vendor count. The estimate rounds up,
/// so a borderline request is rejected rather than sent over budget (task 5).
#[cfg_attr(not(test), allow(dead_code))]
pub const BYTES_PER_TOKEN: usize = 4;

/// Estimate the token count of a request from its serialized byte length (Requirement 4.10).
///
/// The exact Jev tokenizer is closed, so the estimate divides the serialized JSON byte
/// length by [`BYTES_PER_TOKEN`] and rounds up. The round-up is conservative: a borderline
/// request estimates high, so [`guard_budget`] rejects it rather than send it over budget.
///
/// Serialization of a [`JevRequest`] cannot fail in practice, because the type holds only
/// serializable data. A serialize error maps to [`usize::MAX`], so [`guard_budget`] rejects
/// the request rather than panic.
///
/// The first callers are the request builders and the rigor aspect (task 7). Until one
/// lands, the function carries a narrow non-test `allow` with this reason, per the repo
/// dead-code policy (main.rs). The test build exercises it through `mod_tests.rs`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn estimate_tokens(request: &JevRequest) -> usize {
    match serde_json::to_string(request) {
        Ok(json) => json.len().div_ceil(BYTES_PER_TOKEN),
        Err(_) => usize::MAX,
    }
}

/// Reject a request that estimates over the token budget, before any call (Requirement 4.10).
///
/// This is a pre-call guard. It sends nothing. A request whose [`estimate_tokens`] exceeds
/// [`TOKEN_BUDGET`] returns [`JevError::BudgetExceeded`] naming the estimate and the budget.
/// A request at or under the budget returns `Ok`.
///
/// The first callers are the request builders and the rigor aspect (task 7). Until one
/// lands, the function carries a narrow non-test `allow` with this reason, per the repo
/// dead-code policy (main.rs). The test build exercises it through `mod_tests.rs`.
///
/// # Errors
///
/// Returns [`JevError::BudgetExceeded`] when the estimate is more than [`TOKEN_BUDGET`].
#[cfg_attr(not(test), allow(dead_code))]
pub fn guard_budget(request: &JevRequest) -> Result<(), JevError> {
    let estimate = estimate_tokens(request);
    if estimate > TOKEN_BUDGET {
        return Err(JevError::BudgetExceeded {
            estimate,
            budget: TOKEN_BUDGET,
        });
    }
    Ok(())
}

/// The single narrow interface for one Jev evaluation (Requirement 2.1, ADR-J2).
///
/// Both the named fake and the real HTTP client satisfy this trait. The aspect modules
/// depend on the trait, never on a concrete client, so the fake drives every test offline
/// (Requirement 2.4).
///
/// The method is async. The runtime is tokio-based through `rmcp`, so an async method
/// composes with the existing runtime, and the fake satisfies it with a ready future
/// (ADR-J2).
///
/// The first implementor is the named fake (issue #266); the aspect modules (issues #267
/// onward) are the first callers. Until the fake lands, the trait has no non-test user, so
/// it carries a narrow non-test `allow` with a reason. The task that adds the fake removes
/// this attribute.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(async_fn_in_trait)]
pub trait JevClient {
    /// Evaluate one request. Returns the typed response or a typed error.
    ///
    /// # Errors
    ///
    /// Returns a [`JevError`] on a missing key, a residual secret, a non-success status,
    /// a network failure, a timeout, or an invalid response shape.
    async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError>;
}

/// One request body sent to the Jev endpoint (Requirement 2, 4.1).
///
/// The `state` is the evaluation input: a string, object, or array per the wire contract.
/// The `questions` map is keyed by a caller-chosen id, and the matching answer returns
/// under the same id.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize)]
pub struct JevRequest {
    /// The evaluation input. A string, object, or array per the wire contract.
    pub state: serde_json::Value,
    /// The model id, always [`JEV_MODEL`] (Requirement 2.10).
    pub model: String,
    /// The caller-chosen question map.
    pub questions: BTreeMap<String, Question>,
}

impl JevRequest {
    /// Build a request against `state` with the given questions and the fixed model id.
    ///
    /// The `model` field is set to [`JEV_MODEL`], so every built request names
    /// `jev-latest` (Requirement 2.10).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(state: serde_json::Value, questions: BTreeMap<String, Question>) -> Self {
        Self {
            state,
            model: JEV_MODEL.to_string(),
            questions,
        }
    }
}

/// One typed question in a request (Requirement 3, 4, 5, 6, 7).
///
/// The serde `type` tag selects the variant, matching the wire contract. A `Noul` is a
/// yes-or-no judgment, a `Choice` picks one option from a fixed set, and a `Score` rates
/// the state along ordered levels.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// A yes-or-no question. The answer carries a `noul` value, no confidence.
    Noul {
        /// The yes-or-no question to evaluate.
        instructions: String,
        /// An optional clarification of what yes and no mean.
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<String>,
    },
    /// A question over a fixed option set. Criteria maps each option to a description.
    Choice {
        /// What the model should decide.
        instructions: String,
        /// The option set, each mapped to a description or `null` when none is needed.
        criteria: BTreeMap<String, Option<String>>,
    },
    /// A question over ordered levels. Criteria is an ordered list of two or more levels.
    Score {
        /// What the model should rate.
        instructions: String,
        /// The ordered level descriptions. Two or more entries.
        criteria: Vec<String>,
    },
}

/// One request response (Requirement 2).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Deserialize)]
pub struct JevResponse {
    /// The responding model id.
    pub model: String,
    /// The answer map, keyed by the caller-chosen question id.
    pub answers: BTreeMap<String, Answer>,
    /// The token usage for the request.
    pub usage: Usage,
}

/// One typed answer, matched to its question type (Requirement 3, 4, 5, 6, 7).
///
/// The serde `type` tag selects the variant. A `Noul` answer carries a value from 0 to 1
/// and no confidence (Requirement 8.5). A `Choice` and a `Score` answer each carry a
/// probability map and a confidence from 0 to 1.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    /// A Noul answer. A value from 0 to 1. No confidence (Requirement 8.5).
    Noul {
        /// The yes-or-no answer on a scale from 0 (no) to 1 (yes).
        noul: f64,
    },
    /// A Choice answer. The chosen option, a probability map, and a confidence.
    Choice {
        /// The highest-probability option.
        choice: String,
        /// Every option mapped to its probability. The values sum to 1.
        probabilities: BTreeMap<String, f64>,
        /// How certain the model is, derived from the probabilities.
        confidence: f64,
    },
    /// A Score answer. A weighted score, a legend, a probability map, and a confidence.
    Score {
        /// The probability-weighted answer across the levels. Can land between levels.
        score: f64,
        /// Each level number mapped back to its description.
        legend: BTreeMap<String, String>,
        /// Each level mapped to its probability. The values sum to 1.
        probabilities: BTreeMap<String, f64>,
        /// How certain the model is, derived from the probabilities.
        confidence: f64,
    },
}

/// The token usage the response reports (Requirement 12.1).
///
/// The input and output counts are separate, so a configured price can rate them
/// independently, including a zero output rate (Requirement 12.1, 12.2).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Usage {
    /// The input token count.
    pub input_tokens: u64,
    /// The output token count.
    pub output_tokens: u64,
}

/// An error from the Jev harness (Requirement 10, design error handling).
///
/// Every variant message names the offending value, the expected shape, and a
/// remediation hint where one applies. No variant carries a secret value: the
/// [`JevError::SecretResidual`] and the key-related variants name nothing sensitive
/// (Requirement 9.6). Library code returns one of these rather than a panic, an `unwrap`,
/// or an `expect` (Requirement 10.6, 10.7).
// `Clone` and `PartialEq` let the status mapper carry a `JevError` in `StatusAction`
// (client_http.rs) and let a test compare a mapped decision by value. Every variant holds
// only plain data (strings, numbers, and a `&'static str`), so both derive cleanly.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Error)]
pub enum JevError {
    /// The API-key environment variable is absent (Requirement 2.6, 9.4).
    #[error(
        "the Jev API key is not set. Set the `{var}` environment variable to your Jev \
         API key, then re-run. No network call was made."
    )]
    MissingApiKey {
        /// The environment variable name, for example `TRUENORTH_JEV_API_KEY`.
        var: &'static str,
    },

    /// The assembled state still matched the secret denylist after exclusion (Requirement
    /// 9.7). The message names no content, so it leaks no secret.
    #[error(
        "the request state still matched the secret denylist after the exclusion step. \
         No network call was made. Review the denylist and the input paths."
    )]
    SecretResidual,

    /// The endpoint returned 401 (Requirement 10.1).
    #[error(
        "the Jev endpoint rejected the API key (401). Check the `{var}` environment \
         variable holds a valid key."
    )]
    Unauthorized {
        /// The environment variable name that holds the key.
        var: &'static str,
    },

    /// The endpoint returned 422 and named a field (Requirement 10.2).
    #[error("the Jev endpoint rejected the request (422): the field `{field}` is invalid.")]
    Validation {
        /// The offending field the response body named.
        field: String,
    },

    /// The retry limit was reached for 429 or 529 (Requirement 10.5).
    #[error(
        "the Jev endpoint stayed unavailable (last status {last_status}) after \
         {attempts} attempts. Retry later or raise the retry limit."
    )]
    RateLimitedOrOverloaded {
        /// The last HTTP status observed, 429 or 529.
        last_status: u16,
        /// The number of attempts made.
        attempts: u32,
    },

    /// The endpoint returned a status outside the mapped set (Requirement 10, unexpected
    /// path).
    #[error("the Jev endpoint returned an unexpected status {status}.")]
    UnexpectedStatus {
        /// The unmapped HTTP status.
        status: u16,
    },

    /// The network call failed before a response arrived (Requirement 10, network path).
    #[error("the Jev call failed before a response: {detail}. Check network reach.")]
    Network {
        /// The transport failure detail.
        detail: String,
    },

    /// The call exceeded the configured timeout (Requirement 2.5, 2.8).
    #[error("the Jev call exceeded the {timeout_ms} ms timeout. No result was kept.")]
    Timeout {
        /// The configured timeout in milliseconds.
        timeout_ms: u64,
    },

    /// The built request exceeded the token budget (Requirement 4.10).
    #[error(
        "the request estimate {estimate} tokens exceeds the {budget}-token budget. No \
         request was sent. Reduce the state or split the input."
    )]
    BudgetExceeded {
        /// The estimated token count.
        estimate: usize,
        /// The token budget, [`TOKEN_BUDGET`].
        budget: usize,
    },

    /// An expected question id was absent from the response (Requirement 4.7).
    #[error("the response is missing the answer for question id `{id}`.")]
    MissingAnswer {
        /// The absent question id.
        id: String,
    },

    /// A question id appeared more than once in the response (Requirement 4.8).
    #[error("the response carries the answer id `{id}` more than once.")]
    DuplicateAnswer {
        /// The duplicated question id.
        id: String,
    },

    /// A Choice answer chose an option outside the expected set (Requirement 3.4, 7.3).
    #[error("the response chose `{option}`, which is outside the expected set [{expected}].")]
    UnexpectedOption {
        /// The option the response chose.
        option: String,
        /// The expected option set, rendered for the message.
        expected: String,
    },

    /// The configured confidence threshold pair is invalid (Requirement 8.7).
    #[error(
        "invalid confidence thresholds: low {low}, high {high}. Low must be less than or \
         equal to high, and both must be in 0 to 1."
    )]
    InvalidThresholds {
        /// The low threshold.
        low: f64,
        /// The high threshold.
        high: f64,
    },

    /// A configured price rate is negative or not a finite number (Requirement 12.5).
    #[error("invalid price value {value}. A price rate must be a finite value of 0 or more.")]
    InvalidPrice {
        /// The invalid price value.
        value: f64,
    },

    /// The labeled fixture set is missing or unparsable (Requirement 11.9).
    #[error("could not load the fixture set `{path}`: {detail}. No aspect ran.")]
    InvalidFixtureSet {
        /// The fixture-set path.
        path: String,
        /// The load or parse failure detail.
        detail: String,
    },

    /// The config or flag read failed (Requirement 1.6, 1.7).
    #[error("could not read the Jev config `{path}`: {detail}.")]
    Config {
        /// The config path.
        path: String,
        /// The read or parse failure detail.
        detail: String,
    },
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `jev`.
#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "budget_prop_tests.rs"]
mod budget_prop_tests;
