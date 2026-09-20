//! Example tests for the pruning aspect (task 8, issue #270).
//!
//! Included from `pruning.rs` via `#[path]`, so `super` is the `pruning` module. These pin
//! the keep-or-drop behavior, the empty-log short circuit, the missing-answer error, and the
//! chunking path that splits an over-budget log across more than one call
//! (jev-integration-eval R6).

use std::collections::BTreeMap;

use super::super::client_fake::FakeClient;
use super::super::{Answer, JevError, JevResponse, Usage};
use super::*;

/// Build a Score answer with the given score and a trivial legend and probability map.
fn score_answer(score: f64) -> Answer {
    Answer::Score {
        score,
        legend: BTreeMap::from([
            ("0".to_string(), "irrelevant".to_string()),
            ("1".to_string(), "relevant".to_string()),
        ]),
        probabilities: BTreeMap::from([("1".to_string(), 1.0)]),
        confidence: 0.9,
    }
}

/// Build a Jev response whose answers hold one Score per line id, from the given scores.
///
/// The ids run `line_0`, `line_1`, and so on, matching the harness ids for the lines.
fn scored_response(scores: &[f64]) -> JevResponse {
    let mut answers = BTreeMap::new();
    for (index, score) in scores.iter().enumerate() {
        answers.insert(format!("line_{index}"), score_answer(*score));
    }
    JevResponse {
        model: "jev-latest".to_string(),
        answers,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    }
}

#[tokio::test]
async fn three_line_log_keeps_relevant_lines_in_order() {
    let fake = FakeClient::new();
    fake.push_response(scored_response(&[0.9, 0.1, 0.8]));
    let log = vec![
        "connect to database".to_string(),
        "random noise line".to_string(),
        "run the migration".to_string(),
    ];

    let outcome = evaluate_pruning(&fake, "fix the migration", &log, 0.5)
        .await
        .expect("a scored log returns Ok");

    assert_eq!(
        outcome.kept,
        vec![
            "connect to database".to_string(),
            "run the migration".to_string()
        ]
    );
    assert_eq!(outcome.kept_count, 2);
    assert_eq!(outcome.dropped_count, 1);
    assert_eq!(outcome.input_count, 3);
    assert_eq!(fake.call_count(), 1, "a small log fits one call");
}

#[tokio::test]
async fn empty_log_returns_empty_outcome_with_no_call() {
    let fake = FakeClient::new();
    let log: Vec<String> = Vec::new();

    let outcome = evaluate_pruning(&fake, "any task", &log, 0.5)
        .await
        .expect("an empty log returns Ok");

    assert_eq!(outcome.kept, Vec::<String>::new());
    assert_eq!(outcome.kept_count, 0);
    assert_eq!(outcome.dropped_count, 0);
    assert_eq!(outcome.input_count, 0);
    assert_eq!(fake.call_count(), 0, "an empty log makes no call");
}

#[tokio::test]
async fn single_line_kept_when_at_or_above_threshold() {
    let fake = FakeClient::new();
    fake.push_response(scored_response(&[0.7]));
    let log = vec!["relevant line".to_string()];

    let outcome = evaluate_pruning(&fake, "task", &log, 0.5)
        .await
        .expect("a single scored line returns Ok");

    assert_eq!(outcome.kept, vec!["relevant line".to_string()]);
    assert_eq!(outcome.kept_count, 1);
    assert_eq!(outcome.dropped_count, 0);
    assert_eq!(outcome.input_count, 1);
}

#[tokio::test]
async fn single_line_dropped_when_below_threshold() {
    let fake = FakeClient::new();
    fake.push_response(scored_response(&[0.2]));
    let log = vec!["irrelevant line".to_string()];

    let outcome = evaluate_pruning(&fake, "task", &log, 0.5)
        .await
        .expect("a single scored line returns Ok");

    assert!(outcome.kept.is_empty());
    assert_eq!(outcome.kept_count, 0);
    assert_eq!(outcome.dropped_count, 1);
    assert_eq!(outcome.input_count, 1);
}

#[tokio::test]
async fn missing_line_score_yields_a_typed_error() {
    // The response answers only line_0, so line_1 has no Score. The aspect rejects it.
    let fake = FakeClient::new();
    fake.push_response(scored_response(&[0.9]));
    let log = vec!["first line".to_string(), "second line".to_string()];

    let result = evaluate_pruning(&fake, "task", &log, 0.5).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "line_1"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn wrong_type_answer_yields_a_typed_error() {
    // The response returns a Noul under line_0, not a Score. The aspect rejects it.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("line_0".to_string(), Answer::Noul { noul: 0.9 })]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    });
    let log = vec!["only line".to_string()];

    let result = evaluate_pruning(&fake, "task", &log, 0.5).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "line_0"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[tokio::test]
async fn a_large_log_forces_more_than_one_chunk() {
    // Build enough long lines that the whole-log request exceeds the token budget. The
    // harness must split the lines across more than one call (R6.5).
    let line = "x".repeat(2_000);
    let count = 400;
    let log: Vec<String> = (0..count).map(|_| line.clone()).collect();

    // Every line scores 1.0, so every queued response keeps its lines. Queue one response
    // per possible chunk; each response answers all line ids, so any chunk reads its ids.
    let all_scores: Vec<f64> = vec![1.0; count];
    let fake = FakeClient::new();
    for _ in 0..count {
        fake.push_response(scored_response(&all_scores));
    }

    let outcome = evaluate_pruning(&fake, "trace the failure", &log, 0.5)
        .await
        .expect("a large log returns Ok");

    assert!(
        fake.call_count() >= 2,
        "an over-budget log splits across more than one call, got {} calls",
        fake.call_count()
    );
    // Conservation holds across the chunks, and every line is kept in order.
    assert_eq!(outcome.input_count, count);
    assert_eq!(
        outcome.kept_count + outcome.dropped_count,
        outcome.input_count
    );
    assert_eq!(
        outcome.kept_count, count,
        "every line scores at the threshold"
    );
    assert_eq!(outcome.kept, log, "the kept lines preserve the input order");
}
