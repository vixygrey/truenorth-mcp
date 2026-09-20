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
