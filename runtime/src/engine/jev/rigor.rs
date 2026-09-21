//! The rigor aspect: parallel rigor scoring over a code state (jev-integration-eval R4).
//!
//! The aspect asks the model four questions about one code state in a single request, then
//! reads each answer by its caller-chosen id. Three questions are Noul judgments, and one is
//! a Score. The single request carries all four questions, so the model scores them together
//! against the same input (R4.1). The harness owns the mapping from a Score index to a 0 to
//! 100 scale, so the report stays deterministic given the model answer.
//!
//! The model is advisory. It returns four answers; the harness reads each by id, maps the
//! complexity Score to the report scale, and records the call latency. A missing answer or a
//! wrong-type answer is a contract failure, not a score.
//!
//! The aspect has no non-test caller until the benchmark bin lands (task 12). Each public
//! item carries a narrow non-test `allow` with this reason, per the repo dead-code policy
//! (main.rs). The task that wires the benchmark removes the attributes. The test build
//! exercises every item through the sibling `rigor_tests.rs`.
//!
//! Requirements: 4.1, 4.6, 4.7, 4.8, 4.9, 4.10. Design: jev-integration-eval, the rigor
//! aspect.

use std::collections::BTreeMap;

use super::{Answer, JevClient, JevError, JevRequest, Question};

/// The question id for the hallucinated-import Noul judgment (R4.6).
const HALLUCINATED_IMPORT_ID: &str = "hallucinated_import";

/// The question id for the convention-violation Noul judgment (R4.6).
const VIOLATES_CONVENTIONS_ID: &str = "violates_conventions";

/// The question id for the complexity Score (R4.6).
const COMPLEXITY_ID: &str = "complexity";

/// The question id for the contains-secrets Noul judgment (R4.6).
const CONTAINS_SECRETS_ID: &str = "contains_secrets";

/// The ordered complexity rubric levels. The Score answer returns an index on this scale.
///
/// The rubric has five levels, so the returned Score index is 0 through 4. The report
/// normalizes the index to 0 to 100 with [`complexity_to_percent`].
const COMPLEXITY_LEVELS: [&str; 5] = ["trivial", "simple", "moderate", "complex", "very complex"];

/// The report of one rigor evaluation (jev-integration-eval R4).
///
/// The three Noul fields carry a value from 0 to 1. The `complexity` field carries a value
/// from 0 to 100, mapped from the Score index by [`complexity_to_percent`]. The `latency_ms`
/// field is the wall-clock duration of the client call in milliseconds (R4.9).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigorReport {
    /// The hallucinated-import judgment from 0 to 1 (Noul).
    pub hallucinated_import: f64,
    /// The convention-violation judgment from 0 to 1 (Noul).
    pub violates_conventions: f64,
    /// The complexity rating from 0 to 100 (Score, mapped from the rubric index).
    pub complexity: f64,
    /// The contains-secrets judgment from 0 to 1 (Noul).
    pub contains_secrets: f64,
    /// The wall-clock duration of the client call in milliseconds (R4.9).
    pub latency_ms: u64,
}

/// Map a complexity Score index to a percentage from 0 to 100 (jev-integration-eval R4).
///
/// A Score answer returns a `score: f64` on the rubric index scale, from 0 to
/// `levels - 1`. The report scale is 0 to 100. The mapping is
/// `score / (levels - 1) * 100`, so index 0 maps to 0.0, the midpoint index maps to 50.0,
/// and the top index maps to 100.0. The rubric has [`COMPLEXITY_LEVELS`] entries, so the
/// divisor is that count minus one.
fn complexity_to_percent(score: f64) -> f64 {
    let top_index = (COMPLEXITY_LEVELS.len() - 1) as f64;
    score / top_index * 100.0
}

/// Reject a slice of question ids that carries a duplicate id (R4.8).
///
/// The harness builds the request from a fixed id set, so a duplicate id would be a harness
/// defect, not a model output. This guard rejects a slice with a repeated id and returns
/// [`JevError::DuplicateAnswer`] naming the offending id.
///
/// The response-side duplicate cannot occur through the [`super::JevResponse`] answer map: it
/// is a `BTreeMap`, so a repeated id collapses to one key after deserialization. This guard
/// therefore checks the request id set the harness builds, not the deserialized response.
///
/// # Errors
///
/// Returns [`JevError::DuplicateAnswer`] naming the first id that appears more than once.
#[cfg_attr(not(test), allow(dead_code))]
pub fn validate_answer_ids(ids: &[&str]) -> Result<(), JevError> {
    let mut seen: Vec<&str> = Vec::with_capacity(ids.len());
    for id in ids {
        if seen.contains(id) {
            return Err(JevError::DuplicateAnswer {
                id: (*id).to_string(),
            });
        }
        seen.push(id);
    }
    Ok(())
}

/// Read one Noul answer by its id, or return a typed error (R4.6, R4.7).
///
/// A present Noul answer returns its value. A missing answer or a non-Noul answer under the
/// id returns [`JevError::MissingAnswer`] naming the id.
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when the id is absent or its answer is not a Noul.
fn read_noul(answers: &BTreeMap<String, Answer>, id: &str) -> Result<f64, JevError> {
    match answers.get(id) {
        Some(Answer::Noul { noul }) => Ok(*noul),
        // A missing answer or a non-Noul answer under the id is a contract failure.
        _ => Err(JevError::MissingAnswer { id: id.to_string() }),
    }
}

/// Read the complexity Score answer and map it to 0 to 100, or return a typed error (R4.6).
///
/// A present Score answer maps through [`complexity_to_percent`]. A missing answer or a
/// non-Score answer under the complexity id returns [`JevError::MissingAnswer`] naming the id.
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when the complexity id is absent or its answer is not
/// a Score.
fn read_complexity(answers: &BTreeMap<String, Answer>) -> Result<f64, JevError> {
    match answers.get(COMPLEXITY_ID) {
        Some(Answer::Score { score, .. }) => Ok(complexity_to_percent(*score)),
        // A missing answer or a non-Score answer under the complexity id is a contract failure.
        _ => Err(JevError::MissingAnswer {
            id: COMPLEXITY_ID.to_string(),
        }),
    }
}

/// Evaluate the rigor of a code state across four parallel questions (jev-integration-eval R4).
///
/// The aspect builds one request that carries exactly four questions against the same code
/// state: three Noul judgments (hallucinated import, convention violation, contains secrets)
/// and one complexity Score over the [`COMPLEXITY_LEVELS`] rubric (R4.1). The harness guards
/// the token budget before the call (R4.10), times the call for `latency_ms` (R4.9), and
/// reads each answer by its caller-chosen id (R4.6). The complexity Score index maps to 0 to
/// 100 through [`complexity_to_percent`].
///
/// # Errors
///
/// Returns [`JevError::DuplicateAnswer`] when the fixed id set carries a duplicate id.
/// Returns [`JevError::BudgetExceeded`] when the request estimates over the token budget,
/// before any call (R4.10). Returns [`JevError::MissingAnswer`] when the response omits an
/// answer or returns a wrong-type answer under an id (R4.7). Propagates any [`JevError`] the
/// client returns.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn evaluate_rigor<C: JevClient>(
    client: &C,
    code_state: serde_json::Value,
) -> Result<RigorReport, JevError> {
    // The harness builds the request from this fixed id set. The duplicate guard proves the
    // set carries no repeated id before the request is built (R4.8).
    let ids = [
        HALLUCINATED_IMPORT_ID,
        VIOLATES_CONVENTIONS_ID,
        COMPLEXITY_ID,
        CONTAINS_SECRETS_ID,
    ];
    validate_answer_ids(&ids)?;

    let mut questions = BTreeMap::new();
    questions.insert(
        HALLUCINATED_IMPORT_ID.to_string(),
        Question::noul(
            "Decide whether the code imports a module that does not exist. Answer 1 when an \
             import is hallucinated and 0 when every import is real.",
            None,
        ),
    );
    questions.insert(
        VIOLATES_CONVENTIONS_ID.to_string(),
        Question::noul(
            "Decide whether the code violates the project conventions. Answer 1 when it \
             violates a convention and 0 when it obeys them.",
            None,
        ),
    );
    questions.insert(
        CONTAINS_SECRETS_ID.to_string(),
        Question::noul(
            "Decide whether the code contains a secret value. Answer 1 when a secret is \
             present and 0 when none is present.",
            None,
        ),
    );
    questions.insert(
        COMPLEXITY_ID.to_string(),
        Question::score(
            "Rate the complexity of the code across the ordered levels.",
            COMPLEXITY_LEVELS
                .iter()
                .map(|s| serde_json::Value::from(*s))
                .collect(),
        ),
    );

    let request = JevRequest::new(code_state, questions);

    // Guard the budget before the call. An over-budget request sends nothing (R4.10).
    super::guard_budget(&request)?;

    // Time the client call for latency_ms (R4.9).
    let started = std::time::Instant::now();
    let response = client.evaluate(request).await?;
    let latency_ms = started.elapsed().as_millis() as u64;

    let hallucinated_import = read_noul(&response.answers, HALLUCINATED_IMPORT_ID)?;
    let violates_conventions = read_noul(&response.answers, VIOLATES_CONVENTIONS_ID)?;
    let contains_secrets = read_noul(&response.answers, CONTAINS_SECRETS_ID)?;
    let complexity = read_complexity(&response.answers)?;

    Ok(RigorReport {
        hallucinated_import,
        violates_conventions,
        complexity,
        contains_secrets,
        latency_ms,
    })
}

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `rigor`.
#[cfg(test)]
#[path = "rigor_tests.rs"]
mod tests;
