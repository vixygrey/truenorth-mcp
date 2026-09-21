//! Example tests for the routing aspect (task 6, issue #268).
//!
//! Included from `routing.rs` via `#[path]`, so `super` is the `routing` module. These pin the
//! routing behavior: an inside option maps to a member and a band, the decline option is a valid
//! target, an outside option is a typed error with no route, and a missing answer is a typed
//! error (jev-integration-eval R3).

use std::collections::BTreeMap;

use super::super::client_fake::FakeClient;
use super::super::confidence::ConfidenceBand;
use super::super::{Answer, JevError, JevResponse, Usage};
use super::*;

/// Build a Jev response that returns one Choice answer under the routing id.
///
/// The probability map carries the chosen option at probability 1.0, so the fixture is a simple
/// single-option answer.
fn choice_response(choice: &str, confidence: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([(
            "route".to_string(),
            Answer::Choice {
                choice: choice.to_string(),
                probabilities: BTreeMap::from([(choice.to_string(), 1.0)]),
                confidence,
            },
        )]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    }
}

#[tokio::test]
async fn choice_inside_the_set_yields_the_target_and_a_high_band() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("get_skill", 0.9));

    let outcome = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(outcome.target, "get_skill");
    assert_eq!(outcome.confidence, 0.9);
    assert_eq!(outcome.band, ConfidenceBand::High);
    assert_eq!(fake.call_count(), 1);
}

#[tokio::test]
async fn confidence_in_the_middle_band_yields_medium() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("index_skills", 0.7));

    let outcome = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(outcome.target, "index_skills");
    assert_eq!(outcome.band, ConfidenceBand::Medium);
}

#[tokio::test]
async fn confidence_below_the_low_threshold_yields_low() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("validate_skill", 0.4));

    let outcome = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(outcome.band, ConfidenceBand::Low);
}

#[tokio::test]
async fn decline_yields_the_none_target_and_is_not_an_error() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("NONE", 0.5));

    let outcome = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85)
        .await
        .expect("the decline option is a valid target");

    assert_eq!(outcome.target, "NONE");
    assert_eq!(fake.call_count(), 1);
}

#[tokio::test]
async fn option_outside_the_set_yields_unexpected_option_and_no_route() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("rm_rf", 0.9));

    let result = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85).await;
    match result {
        Err(JevError::UnexpectedOption { option, expected }) => {
            assert_eq!(option, "rm_rf", "the error names the offending option");
            assert!(
                expected.contains("get_skill"),
                "the expected set names the members"
            );
            assert!(
                expected.contains("NONE"),
                "the expected set names the decline"
            );
        }
        other => panic!("expected UnexpectedOption, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_answer_yields_a_typed_error() {
    // The response carries no answer under the routing id.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::new(),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    });

    let result = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "route"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn wrong_type_answer_yields_missing_answer() {
    // The response returns a Noul under the routing id, not a Choice. The aspect rejects it.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("route".to_string(), Answer::Noul { noul: 0.5 })]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    });

    let result = evaluate_routing(&fake, serde_json::json!("command"), 0.6, 0.85).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "route"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[test]
fn classify_choice_maps_every_target_to_itself() {
    // Every ROUTING_TARGETS member classifies to its own name, so the whole set is exercised.
    for target in ROUTING_TARGETS {
        let outcome = classify_choice(target, 0.9, 0.6, 0.85).expect("a member is valid");
        assert_eq!(outcome.target, target);
        assert_eq!(outcome.band, ConfidenceBand::High);
    }
}

#[test]
fn structured_criteria_describes_every_target_and_leaves_decline_null() {
    // Every routing tool carries a `what`, so no tool is a bare option that a described
    // neighbor can pull cases toward (issue #327). The decline option stays null.
    let criteria = structured_criteria();

    // Every routing target plus the decline option is present, so the option closure is
    // unchanged by the structured descriptions.
    assert_eq!(criteria.len(), ROUTING_TARGETS.len() + 1);

    // Every target carries a `what`. This catches a new target added without a description.
    for target in ROUTING_TARGETS {
        let described = criteria
            .get(target)
            .unwrap_or_else(|| panic!("target {target} is present"))
            .as_ref()
            .unwrap_or_else(|| panic!("target {target} carries a description"));
        assert!(
            described["what"].is_string(),
            "target {target} has a what description"
        );
    }

    // The decline option is a fixed sentinel, not a tool, so it stays null.
    assert!(
        criteria.get(DECLINE).expect("present").is_none(),
        "the decline option keeps a null description"
    );
}

#[test]
fn structured_criteria_serializes_the_structured_form() {
    // The built criteria serializes so every described option is an object and the decline
    // option is JSON null, matching the wire the endpoint accepts (issue #305, #327).
    let criteria = structured_criteria();
    let value = serde_json::to_value(&criteria).expect("serialize criteria");
    assert!(
        value["get_skill"].is_object(),
        "a described option is an object"
    );
    assert_eq!(
        value["get_skill"]["what"],
        "Read one skill's rendered content by name."
    );
    // A tool with a confusable neighbor also carries a not_for.
    assert!(
        value["get_skill"]["not_for"].is_string(),
        "get_skill has a not_for"
    );
    assert!(value["NONE"].is_null(), "the decline option is JSON null");
}
