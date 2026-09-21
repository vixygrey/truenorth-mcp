//! The drift aspect: a two-layer scope guardrail over a plan (jev-integration-eval R5).
//!
//! The aspect decides whether a plan drifts out of scope by touching a protected path. It
//! runs two layers. The first layer is a deterministic, model-free literal path match
//! (R5.8): a plan that writes a protected path is out of scope with no Jev call. The second
//! layer runs only when the first layer finds no match. It asks the model one Noul question
//! about scope, then applies the drift boundary in the harness (R5.4 to R5.6). The harness
//! owns the threshold step, so the decision is deterministic given the model value.
//!
//! Both layers keep the model advisory. The literal match cannot be overridden by the model,
//! and the boundary comparison, not the model, makes the final call. The model returns a
//! value; the harness decides.
//!
//! The aspect has no non-test caller until the benchmark bin lands (task 12). Each public
//! item carries a narrow non-test `allow` with this reason, per the repo dead-code policy
//! (main.rs). The task that wires the benchmark removes the attributes. The test build
//! exercises every item through the sibling `drift_tests.rs` and `drift_prop_tests.rs`.
//!
//! Requirements: 5.4, 5.5, 5.6, 5.8. Design: jev-integration-eval, ADR-J3, the drift aspect,
//! Property 25 (drift boundary determinism).

use std::collections::BTreeMap;

use super::{Answer, JevClient, JevError, JevRequest, Question};

/// The question id the drift aspect uses for its one Noul question.
const DRIFT_QUESTION_ID: &str = "drift";

/// The outcome of one drift evaluation (jev-integration-eval R5).
///
/// The `noul` value is the model's out-of-scope judgment from 0 to 1, or 1.0 when the
/// model-free layer forced the decision. The `out_of_scope` flag is the harness decision.
/// The `forced_by_path_match` flag is true when the literal path match decided the outcome
/// with no Jev call.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftOutcome {
    /// The out-of-scope value from 0 to 1. It is 1.0 when the path match forced the outcome.
    pub noul: f64,
    /// The harness decision: true when the plan is out of scope.
    pub out_of_scope: bool,
    /// True when the model-free literal path match decided the outcome with no Jev call.
    pub forced_by_path_match: bool,
}

/// Report whether a written path is protected by any protected entry (R5.8).
///
/// A protected entry that ends in `/` is a directory prefix: it matches any written path
/// that starts with it, for example `specs/` matches `specs/adr/0001.md`. A protected entry
/// that does not end in `/` is a file: it matches a written path that equals it exactly, for
/// example `LICENSE` matches only `LICENSE`. The match is deterministic and needs no client.
#[cfg_attr(not(test), allow(dead_code))]
pub fn path_is_protected(written: &str, protected: &[String]) -> bool {
    protected.iter().any(|entry| {
        if let Some(prefix) = entry.strip_suffix('/') {
            // A directory entry matches the directory itself and anything under it.
            written == prefix || written.starts_with(entry)
        } else {
            written == entry
        }
    })
}

/// Report whether a Noul value crosses the drift boundary (R5.4 to R5.6).
///
/// The comparison is `noul >= boundary`. This pure step carries the determinism: the same
/// value and boundary always return the same decision. The harness, not the model, owns this
/// step.
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_out_of_scope(noul: f64, boundary: f64) -> bool {
    noul >= boundary
}

/// Evaluate whether a plan drifts out of scope (jev-integration-eval R5, ADR-J3).
///
/// The evaluation runs two layers. First, the model-free literal path match: when any entry
/// in `written_paths` is protected, the plan is out of scope with no Jev call, and the
/// outcome carries `forced_by_path_match: true` and `noul: 1.0` (R5.8). Otherwise, the
/// aspect asks the model one Noul question about scope, reads the answer under the drift
/// question id, and applies the boundary in the harness (R5.4 to R5.6).
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when the response omits the drift answer or returns a
/// non-Noul answer under the drift id. Propagates any [`JevError`] the client returns.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn evaluate_drift<C: JevClient>(
    client: &C,
    plan_state: serde_json::Value,
    written_paths: &[String],
    protected_paths: &[String],
    drift_boundary: f64,
) -> Result<DriftOutcome, JevError> {
    // Layer one: the deterministic, model-free literal path match. No Jev call.
    let forced = written_paths
        .iter()
        .any(|written| path_is_protected(written, protected_paths));
    if forced {
        return Ok(DriftOutcome {
            noul: 1.0,
            out_of_scope: true,
            forced_by_path_match: true,
        });
    }

    // Layer two: one Noul question about scope, then the harness applies the boundary.
    let mut questions = BTreeMap::new();
    questions.insert(
        DRIFT_QUESTION_ID.to_string(),
        Question::noul(
            "Decide whether the plan is out of scope. The plan must not touch the protected \
             paths. Answer 1 when the plan is out of scope and 0 when it is in scope.",
            None,
        ),
    );

    let request = JevRequest::new(plan_state, questions);
    let response = client.evaluate(request).await?;

    let noul = match response.answers.get(DRIFT_QUESTION_ID) {
        Some(Answer::Noul { noul }) => *noul,
        // A missing answer or a non-Noul answer under the drift id is a contract failure.
        _ => {
            return Err(JevError::MissingAnswer {
                id: DRIFT_QUESTION_ID.to_string(),
            });
        }
    };

    Ok(DriftOutcome {
        noul,
        out_of_scope: is_out_of_scope(noul, drift_boundary),
        forced_by_path_match: false,
    })
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `drift`.
#[cfg(test)]
#[path = "drift_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "drift_prop_tests.rs"]
mod prop_tests;
