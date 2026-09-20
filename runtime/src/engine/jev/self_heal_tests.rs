//! Example tests for the self-healing aspect (task 9).
//!
//! Included from `self_heal.rs` via `#[path]`, so `super` is the `self_heal` module. These
//! pin the self-healing behavior: an inside option maps to its instruction, the two safety
//! overrides win over the mapped option, an outside option is a typed error with no
//! instruction, a missing answer is a typed error, and the trigger predicate classifies a
//! report against the boundaries (jev-integration-eval R7).

use std::collections::BTreeMap;

use super::super::client_fake::FakeClient;
use super::super::rigor::RigorReport;
use super::super::{Answer, JevError, JevResponse, Usage};
use super::*;

/// The low threshold every example uses, matching the config default.
const LOW: f64 = 0.6;

/// The destructive threshold every example uses, matching the config default.
const DESTRUCTIVE: f64 = 0.9;

/// Build a Jev response that returns one Choice answer under the self-heal id.
///
/// The probability map carries the chosen option at probability 1.0, so the fixture is a
/// simple single-option answer.
fn choice_response(choice: &str, confidence: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([(
            "self_heal".to_string(),
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
async fn refactor_imports_at_high_confidence_maps_to_the_instruction() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("REFACTOR_IMPORTS", 0.95));

    let decision = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(decision.instruction, SelfHeal::RefactorImports);
    assert_eq!(decision.confidence, 0.95);
    assert_eq!(fake.call_count(), 1);
}

#[tokio::test]
async fn simplify_logic_at_high_confidence_maps_to_the_instruction() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("SIMPLIFY_LOGIC", 0.95));

    let decision = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(decision.instruction, SelfHeal::SimplifyLogic);
}

#[tokio::test]
async fn ask_human_maps_to_the_instruction() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("ASK_HUMAN", 0.95));

    let decision = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(decision.instruction, SelfHeal::AskHuman);
}

#[tokio::test]
async fn revert_at_high_confidence_maps_to_revert() {
    // A confident revert (>= destructive) is allowed and returns Revert.
    let fake = FakeClient::new();
    fake.push_response(choice_response("REVERT", 0.95));

    let decision = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(decision.instruction, SelfHeal::Revert);
}

#[tokio::test]
async fn revert_below_destructive_threshold_asks_human() {
    // A revert below the destructive threshold, but at or above the low threshold, asks a
    // human rather than revert (the destructive-revert override, R7.5).
    let fake = FakeClient::new();
    fake.push_response(choice_response("REVERT", 0.8));

    let decision = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(decision.instruction, SelfHeal::AskHuman);
}

#[tokio::test]
async fn any_option_below_low_threshold_asks_human() {
    // A confidence below the low threshold asks a human regardless of the option (R7.4).
    let fake = FakeClient::new();
    fake.push_response(choice_response("REFACTOR_IMPORTS", 0.4));

    let decision = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE)
        .await
        .expect("an inside option returns Ok");

    assert_eq!(decision.instruction, SelfHeal::AskHuman);
}

#[tokio::test]
async fn option_outside_the_set_yields_unexpected_option_and_no_instruction() {
    let fake = FakeClient::new();
    fake.push_response(choice_response("DELETE_PROD", 0.95));

    let result = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE).await;
    match result {
        Err(JevError::UnexpectedOption { option, expected }) => {
            assert_eq!(
                option, "DELETE_PROD",
                "the error names the offending option"
            );
            assert!(
                expected.contains("REVERT"),
                "the expected set names the members"
            );
            assert!(
                expected.contains("ASK_HUMAN"),
                "the expected set names every option"
            );
        }
        other => panic!("expected UnexpectedOption, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_answer_yields_a_typed_error() {
    // The response carries no answer under the self-heal id.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::new(),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    });

    let result = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "self_heal"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn wrong_type_answer_yields_missing_answer() {
    // The response returns a Noul under the self-heal id, not a Choice. The aspect rejects it.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("self_heal".to_string(), Answer::Noul { noul: 0.5 })]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    });

    let result = evaluate_self_heal(&fake, serde_json::json!("failure"), LOW, DESTRUCTIVE).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "self_heal"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[test]
fn option_str_and_parse_option_round_trip_every_variant() {
    // Every SelfHeal variant maps to a string and back, so the whole set is exercised.
    for variant in [
        SelfHeal::Revert,
        SelfHeal::RefactorImports,
        SelfHeal::SimplifyLogic,
        SelfHeal::AskHuman,
    ] {
        let text = option_str(variant);
        assert_eq!(parse_option(text), Some(variant));
    }
}

#[test]
fn parse_option_rejects_an_outside_string() {
    assert_eq!(parse_option("DELETE_PROD"), None);
}

/// Build a clean rigor report, all Noul fields low and complexity below any boundary.
fn clean_report() -> RigorReport {
    RigorReport {
        hallucinated_import: 0.1,
        violates_conventions: 0.1,
        complexity: 10.0,
        contains_secrets: 0.1,
        latency_ms: 5,
    }
}

#[test]
fn is_rigor_failure_flags_a_secret_above_the_boundary() {
    let report = RigorReport {
        contains_secrets: 0.9,
        ..clean_report()
    };
    assert!(is_rigor_failure(&report, 0.7, 70.0));
}

#[test]
fn is_rigor_failure_clears_a_clean_report() {
    assert!(!is_rigor_failure(&clean_report(), 0.7, 70.0));
}

#[test]
fn is_rigor_failure_flags_complexity_above_the_boundary() {
    let report = RigorReport {
        complexity: 80.0,
        ..clean_report()
    };
    assert!(is_rigor_failure(&report, 0.7, 70.0));
}
