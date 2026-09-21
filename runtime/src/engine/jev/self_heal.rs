//! The self-healing aspect: pick one recovery action for a rigor failure, or ask a
//! human (jev-integration-eval R7).
//!
//! The aspect asks the model one Choice question over four fixed recovery options, then
//! applies two safety overrides in the harness. The harness owns the option closure and
//! both overrides, so the decision stays deterministic given the model answer. The model
//! is advisory. It returns a chosen option and a confidence; the harness validates the
//! option against the fixed set, then applies the low-confidence override and the
//! destructive-revert override.
//!
//! The trigger predicate [`is_rigor_failure`] classifies a [`RigorReport`] against the
//! configured boundaries, so a caller decides whether a state warrants self-healing before
//! the call. The decision helper [`decide_self_heal`] is pure, so the property tests drive
//! it with no async runtime.
//!
//! The aspect has no non-test caller until the benchmark bin lands (task 12). Each public
//! item carries a narrow non-test `allow` with this reason, per the repo dead-code policy
//! (main.rs). The task that wires the benchmark removes the attributes. The test build
//! exercises every item through the sibling `self_heal_tests.rs` and
//! `self_heal_prop_tests.rs`.
//!
//! Requirements: 7.1, 7.2, 7.3, 7.4, 7.5. Design: jev-integration-eval, the self-healing
//! aspect, Property 29 (option closure), Property 30 (low-confidence REVERT safety).

use std::collections::BTreeMap;

use super::rigor::RigorReport;
use super::{Answer, JevClient, JevError, JevRequest, Question};

/// The question id the self-healing aspect uses for its one Choice question.
const SELF_HEAL_QUESTION_ID: &str = "self_heal";

/// The option string for the revert action.
const REVERT: &str = "REVERT";

/// The option string for the import-refactor action.
const REFACTOR_IMPORTS: &str = "REFACTOR_IMPORTS";

/// The option string for the logic-simplification action.
const SIMPLIFY_LOGIC: &str = "SIMPLIFY_LOGIC";

/// The option string for the ask-a-human action.
const ASK_HUMAN: &str = "ASK_HUMAN";

/// One recovery action the harness can decide (R7.2).
///
/// The set is fixed. The Choice question offers exactly these four options, and the harness
/// maps each option string to one variant with [`parse_option`].
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfHeal {
    /// Revert the change.
    Revert,
    /// Refactor the imports.
    RefactorImports,
    /// Simplify the logic.
    SimplifyLogic,
    /// Ask a human to decide.
    AskHuman,
}

/// The outcome of one self-healing evaluation (R7).
///
/// The `instruction` is the decided recovery action after the harness overrides. The
/// `confidence` is the model's certainty from 0 to 1, carried through unchanged.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelfHealDecision {
    /// The decided recovery action after the harness applies its overrides.
    pub instruction: SelfHeal,
    /// The model's confidence from 0 to 1, carried through unchanged.
    pub confidence: f64,
}

/// Map a [`SelfHeal`] variant to its fixed option string.
///
/// The mapping is the inverse of [`parse_option`], so a round trip through both is the
/// identity for every variant.
#[cfg_attr(not(test), allow(dead_code))]
pub fn option_str(s: SelfHeal) -> &'static str {
    match s {
        SelfHeal::Revert => REVERT,
        SelfHeal::RefactorImports => REFACTOR_IMPORTS,
        SelfHeal::SimplifyLogic => SIMPLIFY_LOGIC,
        SelfHeal::AskHuman => ASK_HUMAN,
    }
}

/// Map an option string to a [`SelfHeal`] variant, or `None` when it is outside the set.
///
/// A string inside the fixed set maps to its variant. A string outside the set returns
/// `None`, so [`decide_self_heal`] rejects it as an unexpected option.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_option(s: &str) -> Option<SelfHeal> {
    match s {
        REVERT => Some(SelfHeal::Revert),
        REFACTOR_IMPORTS => Some(SelfHeal::RefactorImports),
        SIMPLIFY_LOGIC => Some(SelfHeal::SimplifyLogic),
        ASK_HUMAN => Some(SelfHeal::AskHuman),
        _ => None,
    }
}

/// The fixed option set, comma-joined, for an error message and the Choice criteria.
///
/// The order matches the [`SelfHeal`] variant order, so the rendered set reads the same
/// across every call.
fn expected_set() -> String {
    [REVERT, REFACTOR_IMPORTS, SIMPLIFY_LOGIC, ASK_HUMAN].join(", ")
}

/// Report whether a rigor report warrants self-healing (R7.1).
///
/// The report warrants self-healing when any Noul judgment (hallucinated import,
/// convention violation, contains secrets) is at or above `rigor_failure_boundary`, OR the
/// complexity is at or above `complexity_boundary`. The `latency_ms` field carries no
/// signal for this predicate.
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_rigor_failure(
    report: &RigorReport,
    rigor_failure_boundary: f64,
    complexity_boundary: f64,
) -> bool {
    report.hallucinated_import >= rigor_failure_boundary
        || report.violates_conventions >= rigor_failure_boundary
        || report.contains_secrets >= rigor_failure_boundary
        || report.complexity >= complexity_boundary
}

/// Decide the recovery action from a chosen option and a confidence (R7.2 to R7.5).
///
/// This pure step carries the option closure and both safety overrides. The overrides apply
/// in a fixed order:
///
/// 1. A `choice` outside the fixed set returns [`JevError::UnexpectedOption`], so the caller
///    records no instruction (R7.3).
/// 2. A `confidence` less than `low_threshold` returns [`SelfHeal::AskHuman`]. The
///    low-confidence override wins over the mapped option (R7.4).
/// 3. A parsed [`SelfHeal::Revert`] with `confidence` less than `destructive_threshold`
///    returns [`SelfHeal::AskHuman`]. The destructive-revert override blocks a low-confidence
///    revert (R7.5).
/// 4. Otherwise the mapped option returns unchanged (R7.2).
///
/// The step is pure: the same arguments always return the same result, so the property tests
/// drive it directly with no async runtime (Property 29, Property 30).
///
/// # Errors
///
/// Returns [`JevError::UnexpectedOption`] naming the option and the expected set when `choice`
/// is outside the fixed set.
#[cfg_attr(not(test), allow(dead_code))]
pub fn decide_self_heal(
    choice: &str,
    confidence: f64,
    low_threshold: f64,
    destructive_threshold: f64,
) -> Result<SelfHealDecision, JevError> {
    let parsed = match parse_option(choice) {
        Some(option) => option,
        None => {
            return Err(JevError::UnexpectedOption {
                option: choice.to_string(),
                expected: expected_set(),
            });
        }
    };

    // Override 1: the low-confidence override wins over the mapped option (R7.4).
    if confidence < low_threshold {
        return Ok(SelfHealDecision {
            instruction: SelfHeal::AskHuman,
            confidence,
        });
    }

    // Override 2: a low-confidence revert asks a human rather than revert (R7.5).
    if parsed == SelfHeal::Revert && confidence < destructive_threshold {
        return Ok(SelfHealDecision {
            instruction: SelfHeal::AskHuman,
            confidence,
        });
    }

    Ok(SelfHealDecision {
        instruction: parsed,
        confidence,
    })
}

/// Evaluate the self-healing action for a rigor failure, or ask a human (R7).
///
/// The aspect builds one Choice question over the four fixed options, calls the client, and
/// reads the Choice answer under the self-heal question id. It applies the option closure and
/// both overrides through [`decide_self_heal`]. A revert at high confidence returns
/// [`SelfHeal::Revert`]; a revert below the destructive threshold, or any option below the low
/// threshold, returns [`SelfHeal::AskHuman`].
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when the response omits the self-heal answer or returns a
/// non-Choice answer under the self-heal id. Returns [`JevError::UnexpectedOption`] when the
/// chosen option is outside the fixed set. Propagates any [`JevError`] the client returns.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn evaluate_self_heal<C: JevClient>(
    client: &C,
    failure_state: serde_json::Value,
    low_threshold: f64,
    destructive_threshold: f64,
) -> Result<SelfHealDecision, JevError> {
    // The criteria map has one entry per fixed option, each with a short description.
    let mut criteria: BTreeMap<String, Option<serde_json::Value>> = BTreeMap::new();
    criteria.insert(REVERT.to_string(), Some("Revert the change.".into()));
    criteria.insert(
        REFACTOR_IMPORTS.to_string(),
        Some("Refactor the imports.".into()),
    );
    criteria.insert(
        SIMPLIFY_LOGIC.to_string(),
        Some("Simplify the logic.".into()),
    );
    criteria.insert(ASK_HUMAN.to_string(), Some("Ask a human to decide.".into()));

    let mut questions = BTreeMap::new();
    questions.insert(
        SELF_HEAL_QUESTION_ID.to_string(),
        Question::choice(
            "Pick the one recovery action for the rigor failure. Pick ASK_HUMAN when no \
             automatic action is safe.",
            criteria,
        ),
    );

    let request = JevRequest::new(failure_state, questions);
    let response = client.evaluate(request).await?;

    let (choice, confidence) = match response.answers.get(SELF_HEAL_QUESTION_ID) {
        Some(Answer::Choice {
            choice, confidence, ..
        }) => (choice.clone(), *confidence),
        // A missing answer or a non-Choice answer under the self-heal id is a contract failure.
        _ => {
            return Err(JevError::MissingAnswer {
                id: SELF_HEAL_QUESTION_ID.to_string(),
            });
        }
    };

    decide_self_heal(&choice, confidence, low_threshold, destructive_threshold)
}

// Example and property tests live in sibling files to hold this module under the size guidance.
// The `#[path]` include keeps them child modules of `self_heal`.
#[cfg(test)]
#[path = "self_heal_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "self_heal_prop_tests.rs"]
mod prop_tests;
