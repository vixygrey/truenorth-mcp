//! Golden wire-shape tests, pinned from live `POST /v1/systemone` captures.
//!
//! These goldens are the exact request and response bytes observed against the live TypeSafe
//! endpoint (model alias `jev-latest`, responder `jev-1.13.0`) on 2026-09-20. They confirm
//! the wire contract the harness targets, so a later change to the [`Question`] or [`Answer`]
//! types (issues #304 and #305) cannot silently drift from the shape the endpoint accepts and
//! returns.
//!
//! The bodies are sanitized: no API key, and each `state` is synthetic (no repository
//! content). The captures came from three calls that exercised every variant: a Choice
//! (routing), a Noul (drift), and a Score (rigor complexity). Every request used the plain
//! criteria and plain-string instruction form, and every call returned 200. This proves the
//! current plain form is a valid subset of the documented `string | object | array` contract,
//! so the #305 enrichment stays additive and backward-compatible.
//!
//! Included from `mod.rs` via `#[path]`, so `super` is the `jev` module.

use super::{Answer, JevResponse, Question};

// A live Choice response body (routing aspect). Responder `jev-1.13.0`, confidence and a
// probability distribution over the option set, summing to 1.
const GOLDEN_CHOICE_RESPONSE: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "route": {
      "type": "choice",
      "choice": "search_skills",
      "confidence": 0.45,
      "probabilities": {
        "search_skills": 0.5,
        "NONE": 0.26,
        "index_skills": 0.11,
        "get_skill": 0.04,
        "get_git_context": 0.04,
        "truenorth_advance_phase": 0.01,
        "truenorth_record_task": 0.01,
        "truenorth_verify_gate": 0.01,
        "truenorth_tdd_cycle": 0.01,
        "read_skill": 0.01,
        "truenorth_verify_ontology": 0,
        "truenorth_generate_ontology": 0,
        "truenorth_record_bug": 0,
        "truenorth_scaffold_project": 0,
        "validate_skill": 0,
        "get_dependencies": 0
      }
    }
  },
  "usage": { "input_tokens": 445, "output_tokens": 181 }
}"#;

// A live Noul response body (drift aspect). A `noul` value from 0 to 1 and no confidence.
const GOLDEN_NOUL_RESPONSE: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "drift": { "type": "noul", "noul": 0.28 }
  },
  "usage": { "input_tokens": 322, "output_tokens": 21 }
}"#;

// A live Score response body (rigor complexity). A probability-weighted `score`, a `legend`
// mapping each level index to its description, a probability map summing to 1, and a
// confidence.
const GOLDEN_SCORE_RESPONSE: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "complexity": {
      "type": "score",
      "score": 0.64,
      "confidence": 0.7,
      "legend": {
        "0": "trivial",
        "1": "simple",
        "2": "moderate",
        "3": "complex",
        "4": "very complex"
      },
      "probabilities": { "0": 0.36, "1": 0.64, "2": 0, "3": 0, "4": 0 }
    }
  },
  "usage": { "input_tokens": 497, "output_tokens": 76 }
}"#;

#[test]
fn golden_choice_response_deserializes_to_a_choice_answer() {
    let response: JevResponse =
        serde_json::from_str(GOLDEN_CHOICE_RESPONSE).expect("the live Choice body deserializes");
    // The responder returns a concrete version, not the `jev-latest` alias the request sent.
    // The code reads `model` as a free string, so this round-trips as data.
    assert_eq!(response.model, "jev-1.13.0");
    match response
        .answers
        .get("route")
        .expect("the route answer is present")
    {
        Answer::Choice {
            choice,
            probabilities,
            confidence,
        } => {
            assert_eq!(choice, "search_skills");
            assert_eq!(probabilities.len(), 16);
            assert!((confidence - 0.45).abs() < f64::EPSILON);
            let sum: f64 = probabilities.values().sum();
            assert!((sum - 1.0).abs() < 1e-9, "the probabilities sum to 1");
        }
        other => panic!("expected a Choice answer, got {other:?}"),
    }
}

#[test]
fn golden_noul_response_deserializes_to_a_noul_answer() {
    let response: JevResponse =
        serde_json::from_str(GOLDEN_NOUL_RESPONSE).expect("the live Noul body deserializes");
    match response
        .answers
        .get("drift")
        .expect("the drift answer is present")
    {
        // A Noul answer carries a value and no confidence field.
        Answer::Noul { noul } => assert!((noul - 0.28).abs() < f64::EPSILON),
        other => panic!("expected a Noul answer, got {other:?}"),
    }
    assert_eq!(response.usage.output_tokens, 21);
}

#[test]
fn golden_score_response_deserializes_to_a_score_answer() {
    let response: JevResponse =
        serde_json::from_str(GOLDEN_SCORE_RESPONSE).expect("the live Score body deserializes");
    match response
        .answers
        .get("complexity")
        .expect("the complexity answer is present")
    {
        Answer::Score {
            score,
            legend,
            probabilities,
            confidence,
        } => {
            assert!((score - 0.64).abs() < f64::EPSILON);
            assert_eq!(legend.len(), 5);
            assert_eq!(legend.get("4").map(String::as_str), Some("very complex"));
            assert_eq!(probabilities.len(), 5);
            assert!((confidence - 0.7).abs() < f64::EPSILON);
            let sum: f64 = probabilities.values().sum();
            assert!((sum - 1.0).abs() < 1e-9, "the probabilities sum to 1");
        }
        other => panic!("expected a Score answer, got {other:?}"),
    }
}

#[test]
fn golden_score_request_matches_the_live_score_question_shape() {
    // The rigor aspect built this Score question, and the live endpoint accepted it (200).
    // The golden pins the plain shape: a string instruction and an ordered array of level
    // strings. A change that alters this serialization breaks the confirmed contract.
    let question = Question::score(
        "Rate the complexity of the code across the ordered levels.",
        vec![
            "trivial".into(),
            "simple".into(),
            "moderate".into(),
            "complex".into(),
            "very complex".into(),
        ],
    );
    let value = serde_json::to_value(&question).expect("serialize the Score question");
    assert_eq!(value["type"], "score");
    assert!(
        value["instructions"].is_string(),
        "instructions is a plain string"
    );
    assert_eq!(value["criteria"][0], "trivial");
    assert_eq!(value["criteria"][4], "very complex");
    assert_eq!(
        value["criteria"].as_array().map(Vec::len),
        Some(5),
        "the ordered level array has five entries"
    );
}

#[test]
fn golden_choice_request_matches_the_live_choice_question_shape() {
    // The routing aspect built a Choice question with an option-to-null map, and the endpoint
    // accepted it (200). The golden pins the plain criteria form (option keyed to null).
    let mut criteria = std::collections::BTreeMap::new();
    criteria.insert("NONE".to_string(), None);
    criteria.insert("search_skills".to_string(), None);
    let question = Question::choice(
        "Pick the one TrueNorth tool that serves the command. Pick NONE when no tool fits.",
        criteria,
    );
    let value = serde_json::to_value(&question).expect("serialize the Choice question");
    assert_eq!(value["type"], "choice");
    assert!(
        value["instructions"].is_string(),
        "instructions is a plain string"
    );
    assert!(
        value["criteria"]["NONE"].is_null(),
        "a null option maps to null"
    );
    assert!(value["criteria"]["search_skills"].is_null());
}
