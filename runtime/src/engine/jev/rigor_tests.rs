//! Example tests for the rigor aspect (task 7, issue #269).
//!
//! Included from `rigor.rs` via `#[path]`, so `super` is the `rigor` module. These pin the
//! parallel rigor behavior: one request carries four questions, each answer reads by id, the
//! complexity Score maps to 0 to 100, the budget guard blocks an over-budget request before
//! any call, and a missing or wrong-type answer is a typed error (jev-integration-eval R4).

use std::collections::BTreeMap;

use super::super::client_fake::FakeClient;
use super::super::{Answer, JevError, JevResponse, Usage};
use super::*;

/// Build a Score answer at the given rubric index, with a filled legend and probabilities.
///
/// The legend and probability maps carry the top index at probability 1.0, so the fixture is
/// a simple single-level answer. The aspect reads only the `score`, so the maps are shape
/// only.
fn score_answer(score: f64) -> Answer {
    let key = format!("{score}");
    Answer::Score {
        score,
        legend: BTreeMap::from([(key.clone(), "moderate".to_string())]),
        probabilities: BTreeMap::from([(key, 1.0)]),
        confidence: 0.9,
    }
}

/// Build a full rigor response: three Noul answers and one complexity Score.
fn full_response(
    hallucinated: f64,
    violates: f64,
    secrets: f64,
    complexity_index: f64,
) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([
            (
                "hallucinated_import".to_string(),
                Answer::Noul { noul: hallucinated },
            ),
            (
                "violates_conventions".to_string(),
                Answer::Noul { noul: violates },
            ),
            (
                "contains_secrets".to_string(),
                Answer::Noul { noul: secrets },
            ),
            ("complexity".to_string(), score_answer(complexity_index)),
        ]),
        usage: Usage {
            input_tokens: 20,
            output_tokens: 4,
        },
    }
}

#[tokio::test]
async fn full_response_yields_the_report_and_one_four_question_request() {
    let fake = FakeClient::new();
    // Complexity index 3.0 of a 5-level rubric maps to 3 / 4 * 100 = 75.0.
    fake.push_response(full_response(0.1, 0.2, 0.0, 3.0));

    let report = evaluate_rigor(&fake, serde_json::json!("code"))
        .await
        .expect("a full response returns Ok");

    assert_eq!(report.hallucinated_import, 0.1);
    assert_eq!(report.violates_conventions, 0.2);
    assert_eq!(report.contains_secrets, 0.0);
    assert_eq!(report.complexity, 75.0);

    // The report records a latency. The value is non-negative by type, so assert the call ran.
    assert_eq!(fake.call_count(), 1);
    let calls = fake.calls();
    assert_eq!(
        calls[0].questions.len(),
        4,
        "the single request carries exactly four questions"
    );
}

#[tokio::test]
async fn missing_hallucinated_import_answer_yields_a_typed_error() {
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([
            (
                "violates_conventions".to_string(),
                Answer::Noul { noul: 0.0 },
            ),
            ("contains_secrets".to_string(), Answer::Noul { noul: 0.0 }),
            ("complexity".to_string(), score_answer(1.0)),
        ]),
        usage: Usage {
            input_tokens: 20,
            output_tokens: 4,
        },
    });

    let result = evaluate_rigor(&fake, serde_json::json!("code")).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "hallucinated_import"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_complexity_answer_yields_a_typed_error() {
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([
            (
                "hallucinated_import".to_string(),
                Answer::Noul { noul: 0.0 },
            ),
            (
                "violates_conventions".to_string(),
                Answer::Noul { noul: 0.0 },
            ),
            ("contains_secrets".to_string(), Answer::Noul { noul: 0.0 }),
        ]),
        usage: Usage {
            input_tokens: 20,
            output_tokens: 4,
        },
    });

    let result = evaluate_rigor(&fake, serde_json::json!("code")).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "complexity"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn wrong_type_complexity_answer_yields_missing_answer() {
    // The response returns a Noul under the complexity id, not a Score. The aspect rejects it.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([
            (
                "hallucinated_import".to_string(),
                Answer::Noul { noul: 0.0 },
            ),
            (
                "violates_conventions".to_string(),
                Answer::Noul { noul: 0.0 },
            ),
            ("contains_secrets".to_string(), Answer::Noul { noul: 0.0 }),
            ("complexity".to_string(), Answer::Noul { noul: 0.5 }),
        ]),
        usage: Usage {
            input_tokens: 20,
            output_tokens: 4,
        },
    });

    let result = evaluate_rigor(&fake, serde_json::json!("code")).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "complexity"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn over_budget_request_is_rejected_before_any_call() {
    // A huge state string estimates over the token budget, so the guard blocks the call.
    let fake = FakeClient::new();
    let huge = "x".repeat(super::super::TOKEN_BUDGET * super::super::BYTES_PER_TOKEN + 1_000);

    let result = evaluate_rigor(&fake, serde_json::json!(huge)).await;
    match result {
        Err(JevError::BudgetExceeded { estimate, budget }) => {
            assert!(estimate > budget, "the estimate exceeds the budget");
            assert_eq!(budget, super::super::TOKEN_BUDGET);
        }
        other => panic!("expected BudgetExceeded, got {other:?}"),
    }
    assert_eq!(
        fake.call_count(),
        0,
        "the budget guard sends no call (R4.10)"
    );
}

#[test]
fn validate_answer_ids_rejects_a_duplicate_and_accepts_distinct() {
    // A duplicate id in the slice returns DuplicateAnswer naming the repeated id.
    match validate_answer_ids(&["a", "b", "a"]) {
        Err(JevError::DuplicateAnswer { id }) => assert_eq!(id, "a"),
        other => panic!("expected DuplicateAnswer, got {other:?}"),
    }
    // A distinct slice returns Ok.
    validate_answer_ids(&["a", "b", "c", "d"]).expect("a distinct slice is valid");
}

#[test]
fn complexity_mapping_spans_zero_to_one_hundred() {
    // Index 0 maps to 0, the top index maps to 100, and the midpoint maps to 50.
    assert_eq!(complexity_to_percent(0.0), 0.0);
    assert_eq!(complexity_to_percent(4.0), 100.0);
    assert_eq!(complexity_to_percent(2.0), 50.0);
}
