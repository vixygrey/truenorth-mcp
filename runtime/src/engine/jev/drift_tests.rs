//! Example tests for the drift aspect (task 5, issue #267).
//!
//! Included from `drift.rs` via `#[path]`, so `super` is the `drift` module. These pin the
//! two-layer behavior: the model-free literal path match forces out-of-scope with no call,
//! and the model layer applies the boundary in the harness (jev-integration-eval R5).

use std::collections::BTreeMap;

use super::super::client_fake::FakeClient;
use super::super::{Answer, JevError, JevResponse, Usage};
use super::*;

/// Build a Jev response that returns one Noul answer under the drift id.
fn noul_response(noul: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("drift".to_string(), Answer::Noul { noul })]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    }
}

#[tokio::test]
async fn written_path_under_a_protected_dir_forces_out_of_scope_with_no_call() {
    let fake = FakeClient::new();
    let written = vec!["specs/adr/0001.md".to_string()];
    let protected = vec!["specs/".to_string()];

    let outcome = evaluate_drift(&fake, serde_json::json!("plan"), &written, &protected, 0.7)
        .await
        .expect("a forced outcome returns Ok");

    assert!(outcome.out_of_scope);
    assert!(outcome.forced_by_path_match);
    assert_eq!(fake.call_count(), 0, "the model-free layer makes no call");
}

#[tokio::test]
async fn exact_file_match_forces_out_of_scope() {
    let fake = FakeClient::new();
    let written = vec!["LICENSE".to_string()];
    let protected = vec!["LICENSE".to_string()];

    let outcome = evaluate_drift(&fake, serde_json::json!("plan"), &written, &protected, 0.7)
        .await
        .expect("a forced outcome returns Ok");

    assert!(outcome.out_of_scope);
    assert!(outcome.forced_by_path_match);
    assert_eq!(outcome.noul, 1.0);
    assert_eq!(fake.call_count(), 0);
}

#[tokio::test]
async fn clean_plan_below_boundary_is_in_scope_with_one_call() {
    let fake = FakeClient::new();
    fake.push_response(noul_response(0.2));
    let written = vec!["src/main.rs".to_string()];
    let protected = vec!["specs/".to_string(), "LICENSE".to_string()];

    let outcome = evaluate_drift(&fake, serde_json::json!("plan"), &written, &protected, 0.7)
        .await
        .expect("a clean plan returns Ok");

    assert!(!outcome.out_of_scope);
    assert!(!outcome.forced_by_path_match);
    assert_eq!(fake.call_count(), 1, "the model layer makes one call");
}

#[tokio::test]
async fn clean_plan_at_or_above_boundary_is_out_of_scope() {
    let fake = FakeClient::new();
    fake.push_response(noul_response(0.9));
    let written = vec!["src/main.rs".to_string()];
    let protected = vec!["specs/".to_string()];

    let outcome = evaluate_drift(&fake, serde_json::json!("plan"), &written, &protected, 0.7)
        .await
        .expect("a clean plan returns Ok");

    assert!(outcome.out_of_scope);
    assert!(
        !outcome.forced_by_path_match,
        "the model, not a path match, decided"
    );
    assert_eq!(fake.call_count(), 1);
}

#[tokio::test]
async fn missing_or_wrong_type_answer_yields_a_typed_error() {
    // The response returns a Choice under the drift id, not a Noul. The aspect rejects it.
    let fake = FakeClient::new();
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([(
            "drift".to_string(),
            Answer::Choice {
                choice: "revert".to_string(),
                probabilities: BTreeMap::from([("revert".to_string(), 1.0)]),
                confidence: 0.9,
            },
        )]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 0,
        },
    });
    let written = vec!["src/main.rs".to_string()];
    let protected = vec!["specs/".to_string()];

    let result = evaluate_drift(&fake, serde_json::json!("plan"), &written, &protected, 0.7).await;
    match result {
        Err(JevError::MissingAnswer { id }) => assert_eq!(id, "drift"),
        other => panic!("expected MissingAnswer, got {other:?}"),
    }
}

#[test]
fn path_is_protected_matches_dir_prefix_and_exact_file() {
    let protected = vec!["specs/".to_string(), "LICENSE".to_string()];

    // A directory entry matches the directory itself and anything under it.
    assert!(path_is_protected("specs/adr/0001.md", &protected));
    assert!(path_is_protected("specs", &protected));
    // A file entry matches only an exact equal path.
    assert!(path_is_protected("LICENSE", &protected));
    assert!(!path_is_protected("LICENSE.md", &protected));
    // An unrelated path is not protected.
    assert!(!path_is_protected("src/main.rs", &protected));
}

#[test]
fn is_out_of_scope_applies_the_boundary() {
    assert!(is_out_of_scope(0.7, 0.7), "at the boundary is out of scope");
    assert!(is_out_of_scope(0.9, 0.7));
    assert!(!is_out_of_scope(0.69, 0.7));
    assert!(!is_out_of_scope(0.0, 0.7));
}
