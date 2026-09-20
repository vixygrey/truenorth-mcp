//! The `evaluate_guard` entry point and the probabilistic fail-open layer (task 3, #295).
//!
//! Included from `guard.rs` via `#[path]`, so `super` is the `guard` module. This module ties
//! the deterministic layer (`guard.rs`) and the pure combine step (`guard_combine.rs`) into one
//! decision function. It runs the deterministic layer first, then the probabilistic layer.
//!
//! The probabilistic layer always fails open (ADR-G3). It returns `Allow` with a note, never a
//! `Block`, when the flag is off, the client is absent, the API key is absent, the secret
//! filter rejects a residual secret, or a Jev call times out, fails on the network, or returns
//! a non-success status (R4). It never turns a Jev outage into a block (R4.5, P37).
//!
//! The layer sends no secret to Jev: it builds the state through the secret filter, so a
//! denylisted path or content is excluded before the call, and a residual secret fails closed
//! (R3.2, R3.3, P40). No neutralization packet and no note carries a secret value or the API
//! key (P40).
//!
//! The entry point has no non-test caller until the tool lands (issue #296). Each staged public
//! item carries a narrow non-test `allow` with this reason, per the repo dead-code policy
//! (main.rs). The test build exercises every path through the sibling `guard_evaluate_tests.rs`
//! and `guard_evaluate_prop_tests.rs`.
//!
//! Requirements: 1.3, 1.4, 3.1, 3.2, 3.3, 3.5, 4.1, 4.2, 4.3, 4.5.
//! Design: jev-active-guardrail, the evaluate_guard function, P36, P37, P40.

use std::path::PathBuf;

use super::super::JevClient;
use super::super::config::JevConfig;
use super::super::drift::{DriftOutcome, evaluate_drift};
use super::super::rigor::{RigorReport, evaluate_rigor};
use super::super::secret_filter::build_state_with_secret_filter;
use super::combine::{CandidateSignal, combine};
use super::{GuardDecision, ProposedChange, deterministic_layer};

/// The note a flag-off evaluation returns (R4.1).
const NOTE_FLAG_OFF: &str = "probabilistic layer did not run: the jev feature is off";

/// The note an absent-client evaluation returns (R4.1).
const NOTE_NO_CLIENT: &str = "probabilistic layer did not run: no jev client is configured";

/// The note an absent-key evaluation returns (R4.2).
const NOTE_NO_KEY: &str = "probabilistic layer did not run: no jev API key is set";

/// Evaluate a proposed change and return one decision (R1.3, R1.4).
///
/// The function runs the deterministic layer first, with no Jev call. A deterministic block
/// returns immediately (R1.3). When the deterministic layer passes, the function runs the
/// probabilistic layer, which always fails open (ADR-G3).
///
/// The probabilistic layer returns `Allow` with a note, and makes no Jev call, when:
///
/// - the flag is off (R4.1),
/// - the client is absent (R4.1),
/// - the API key is absent (R4.2).
///
/// Otherwise it builds the Jev state through the secret filter. A residual secret surfaces the
/// harness `SecretResidual` as an `Allow` with a note, because the probabilistic layer never
/// hard-blocks the developer; the deterministic secret scan already caught a plain secret
/// (R3.2, R3.3). It then scores rigor and drift. A timeout, a network error, or a non-success
/// status returns `Allow` with a note naming the cause, never a block (R4.3, R4.5, P37).
///
/// When both aspects succeed, the function gates the signals through the pure combine step and
/// returns its decision (R1.4).
///
/// The `api_key_present` flag is supplied by the caller, which reads the key environment
/// variable once. Passing it keeps this function free of a process-global environment read, so
/// the tests are deterministic and parallel-safe. The caller (the tool, issue #296) reads
/// `JEV_API_KEY_VAR` and passes the result.
pub async fn evaluate_guard<C: JevClient>(
    change: &ProposedChange,
    protected_paths: &[String],
    config: &JevConfig,
    jev_enabled: bool,
    api_key_present: bool,
    client: Option<&C>,
    repo_root: &std::path::Path,
) -> GuardDecision {
    // The deterministic layer runs first, with no Jev call (R1.3). A block returns now.
    if let Some(decision) = deterministic_layer(change, protected_paths) {
        return decision;
    }

    // The probabilistic layer fails open on every unavailability (ADR-G3, R4).
    if !jev_enabled {
        return allow_with_note(NOTE_FLAG_OFF);
    }
    let Some(client) = client else {
        return allow_with_note(NOTE_NO_CLIENT);
    };
    if !api_key_present {
        return allow_with_note(NOTE_NO_KEY);
    }

    // Build the Jev state through the secret filter, so no secret reaches Jev (R3.2, P40).
    let paths: Vec<PathBuf> = change.paths.iter().map(PathBuf::from).collect();
    let state = match build_state_with_secret_filter(repo_root, &paths) {
        Ok(state) => state,
        // A residual secret makes no call. The probabilistic layer fails open with a note; the
        // deterministic secret scan already blocks a plain secret (R3.3).
        Err(cause) => return allow_with_note(&format!("probabilistic layer did not run: {cause}")),
    };

    // Score rigor. A Jev unavailability fails open with a note naming the cause (R4.3, P37).
    let rigor = match evaluate_rigor(client, state.clone()).await {
        Ok(report) => report,
        Err(cause) => return allow_with_note(&format!("rigor scoring did not complete: {cause}")),
    };

    // Score drift. The written paths and the protected set drive the model-free path layer.
    let drift = match evaluate_drift(
        client,
        state,
        &change.paths,
        protected_paths,
        config.drift_boundary,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(cause) => return allow_with_note(&format!("drift scoring did not complete: {cause}")),
    };

    // Both aspects succeeded. Gate the signals through the pure combine step (R1.4).
    let rigor_signal = rigor_candidate(&rigor, config);
    let drift_signal = drift_candidate(&drift);
    combine(&rigor, rigor_signal, &drift, drift_signal, None, config)
}

/// Build an `Allow` decision carrying one advisory note.
fn allow_with_note(note: &str) -> GuardDecision {
    GuardDecision::Allow {
        notes: vec![note.to_string()],
    }
}

/// Build the rigor candidate signal from a rigor report (R3.4).
///
/// The rigor Noul answers carry no separate confidence, so the signal strength is the strongest
/// fired signal: the maximum of the three Noul judgments and the complexity mapped into the unit
/// range. This is the honest, only-available confidence for the rigor candidate. The guard's
/// rigor block is an advisory stop, not a destructive action, so the candidate is
/// non-destructive; a destructive action is a self-heal revert, decided elsewhere.
fn rigor_candidate(rigor: &RigorReport, config: &JevConfig) -> CandidateSignal {
    let complexity_unit = if config.complexity_boundary > 0.0 {
        (rigor.complexity / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let confidence = rigor
        .hallucinated_import
        .max(rigor.violates_conventions)
        .max(rigor.contains_secrets)
        .max(complexity_unit);
    CandidateSignal {
        confidence,
        destructive: false,
    }
}

/// Build the drift candidate signal from a drift outcome (R3.4).
///
/// The drift Noul carries no separate confidence, so the signal strength is the Noul value. A
/// drift forced by the model-free path match carries `noul = 1.0`, so a forced outcome always
/// clears the gate, which matches the deterministic decision the path match already made. The
/// drift block is an advisory stop, so the candidate is non-destructive.
fn drift_candidate(drift: &DriftOutcome) -> CandidateSignal {
    CandidateSignal {
        confidence: drift.noul,
        destructive: false,
    }
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `evaluate`.
#[cfg(test)]
#[path = "guard_evaluate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guard_evaluate_prop_tests.rs"]
mod prop_tests;
