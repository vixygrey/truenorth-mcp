//! The routing aspect: pick one TrueNorth tool for a command, or decline (R3).
//!
//! The aspect asks the model one Choice question over the fixed tool surface plus a decline
//! option, then bands the model's confidence in the harness. The harness owns the option
//! closure and the banding step, so the decision stays deterministic given the model answer.
//!
//! The model is advisory. It returns a chosen option and a confidence; the harness validates
//! the option against the fixed set and maps the confidence to a band. A chosen option outside
//! the set is a contract failure, not a route.
//!
//! The aspect has no non-test caller until the benchmark bin lands (task 12). Each public item
//! carries a narrow non-test `allow` with this reason, per the repo dead-code policy (main.rs).
//! The task that wires the benchmark removes the attributes. The test build exercises every item
//! through the sibling `routing_tests.rs` and `routing_prop_tests.rs`.
//!
//! Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8. Design: jev-integration-eval, the
//! routing aspect, Property 28 (routing option closure).

use std::collections::BTreeMap;

use super::confidence::{ConfidenceBand, confidence_band};
use super::{Answer, JevClient, JevError, JevRequest, Question};

/// The question id the routing aspect uses for its one Choice question.
const ROUTING_QUESTION_ID: &str = "route";

/// The decline option: the model picks this when no tool fits (R3.2).
const DECLINE: &str = "NONE";

/// The TrueNorth tool surface the router chooses from (R3.2).
///
/// Each entry is a live tool name. The model picks exactly one of these or [`DECLINE`]. The
/// names are self-describing, so the Choice criteria map their descriptions to `None`.
#[cfg_attr(not(test), allow(dead_code))]
pub const ROUTING_TARGETS: [&str; 15] = [
    "truenorth_scaffold_project",
    "truenorth_advance_phase",
    "truenorth_record_task",
    "truenorth_verify_gate",
    "truenorth_tdd_cycle",
    "truenorth_record_bug",
    "truenorth_generate_ontology",
    "truenorth_verify_ontology",
    "index_skills",
    "get_skill",
    "read_skill",
    "search_skills",
    "get_dependencies",
    "get_git_context",
    "validate_skill",
];

/// The outcome of one routing evaluation (R3).
///
/// The `target` is the chosen tool name, or [`DECLINE`] when the model declined. The
/// `confidence` is the model's certainty from 0 to 1. The `band` is the harness classification
/// of that confidence against the configured thresholds.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingOutcome {
    /// The chosen tool name, or [`DECLINE`] when the model declined.
    pub target: String,
    /// The model's confidence from 0 to 1.
    pub confidence: f64,
    /// The harness confidence band for the confidence value.
    pub band: ConfidenceBand,
}

/// Report whether an option is a valid routing choice (R3.3, R3.4).
///
/// A valid option is a [`ROUTING_TARGETS`] member or [`DECLINE`]. The check is deterministic and
/// needs no client.
fn is_valid_target(option: &str) -> bool {
    option == DECLINE || ROUTING_TARGETS.contains(&option)
}

/// Render the valid option set for an error message.
///
/// The message lists every [`ROUTING_TARGETS`] member and [`DECLINE`], comma-joined, so an
/// [`JevError::UnexpectedOption`] names the shape the model must obey.
fn expected_set() -> String {
    let mut options: Vec<&str> = ROUTING_TARGETS.to_vec();
    options.push(DECLINE);
    options.join(", ")
}

/// Validate a chosen option and band its confidence (R3.3 to R3.7).
///
/// This pure step carries the option closure and the banding. A `choice` inside the valid set
/// returns a [`RoutingOutcome`] whose `target` equals the choice. A `choice` outside the set
/// returns [`JevError::UnexpectedOption`], so the caller records no route. The banding uses
/// [`confidence_band`], so an invalid threshold pair surfaces as
/// [`JevError::InvalidThresholds`].
///
/// The step is pure: the same arguments always return the same result, so the property test
/// drives it directly with no async runtime (Property 28).
///
/// # Errors
///
/// Returns [`JevError::UnexpectedOption`] naming the option and the expected set when `choice`
/// is outside the valid set. Returns [`JevError::InvalidThresholds`] when the threshold pair is
/// invalid.
#[cfg_attr(not(test), allow(dead_code))]
pub fn classify_choice(
    choice: &str,
    confidence: f64,
    low: f64,
    high: f64,
) -> Result<RoutingOutcome, JevError> {
    if !is_valid_target(choice) {
        return Err(JevError::UnexpectedOption {
            option: choice.to_string(),
            expected: expected_set(),
        });
    }
    let band = confidence_band(confidence, low, high)?;
    Ok(RoutingOutcome {
        target: choice.to_string(),
        confidence,
        band,
    })
}

/// Evaluate which TrueNorth tool a command routes to, or decline (R3, ADR-J routing aspect).
///
/// The aspect builds one Choice question over the [`ROUTING_TARGETS`] set plus [`DECLINE`], calls
/// the client, and reads the Choice answer under the routing question id. It validates the chosen
/// option against the valid set and bands the confidence in the harness. The [`DECLINE`] option is
/// a valid member, not an error: it maps to a [`RoutingOutcome`] with target [`DECLINE`].
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when the response omits the routing answer or returns a
/// non-Choice answer under the routing id. Returns [`JevError::UnexpectedOption`] when the chosen
/// option is outside the valid set. Returns [`JevError::InvalidThresholds`] when the threshold
/// pair is invalid. Propagates any [`JevError`] the client returns.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn evaluate_routing<C: JevClient>(
    client: &C,
    command_state: serde_json::Value,
    low: f64,
    high: f64,
) -> Result<RoutingOutcome, JevError> {
    // The criteria map has one entry per valid option. The tool names are self-describing, so
    // each description is `None`.
    let mut criteria: BTreeMap<String, Option<String>> = BTreeMap::new();
    for target in ROUTING_TARGETS {
        criteria.insert(target.to_string(), None);
    }
    criteria.insert(DECLINE.to_string(), None);

    let mut questions = BTreeMap::new();
    questions.insert(
        ROUTING_QUESTION_ID.to_string(),
        Question::Choice {
            instructions: "Pick the one TrueNorth tool that serves the command. Pick NONE when \
                           no tool fits."
                .to_string(),
            criteria,
        },
    );

    let request = JevRequest::new(command_state, questions);
    let response = client.evaluate(request).await?;

    let (choice, confidence) = match response.answers.get(ROUTING_QUESTION_ID) {
        Some(Answer::Choice {
            choice, confidence, ..
        }) => (choice.clone(), *confidence),
        // A missing answer or a non-Choice answer under the routing id is a contract failure.
        _ => {
            return Err(JevError::MissingAnswer {
                id: ROUTING_QUESTION_ID.to_string(),
            });
        }
    };

    classify_choice(&choice, confidence, low, high)
}

// Example and property tests live in sibling files to hold this module under the size guidance.
// The `#[path]` include keeps them child modules of `routing`.
#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "routing_prop_tests.rs"]
mod prop_tests;
