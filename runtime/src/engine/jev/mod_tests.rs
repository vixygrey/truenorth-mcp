//! Unit tests for the Jev wire types (task 2, issue #264).
//!
//! Included from `mod.rs` via `#[path]`, so `super` is the `jev` module. These pin the
//! serde `type`-tag mapping for each `Question` and `Answer` variant, the Score legend
//! and probability map, and the `Usage` shape, so a later client can rely on the wire
//! contract (Requirement 2.1, 2.10).

use std::collections::BTreeMap;

use super::*;

#[test]
fn request_new_sets_the_model_id() {
    // Every built request names `jev-latest` (Requirement 2.10).
    let request = JevRequest::new(serde_json::json!("state"), BTreeMap::new());
    assert_eq!(request.model, JEV_MODEL);
    assert_eq!(request.model, "jev-latest");
}

#[test]
fn noul_question_serializes_with_the_type_tag() {
    let question = Question::Noul {
        instructions: "Is this out of scope?".to_string(),
        criteria: None,
    };
    let value = serde_json::to_value(&question).expect("serialize noul");
    assert_eq!(value["type"], "noul");
    assert_eq!(value["instructions"], "Is this out of scope?");
    // An absent criteria is skipped, not serialized as null.
    assert!(value.get("criteria").is_none(), "None criteria is skipped");
}

#[test]
fn noul_question_serializes_present_criteria() {
    let question = Question::Noul {
        instructions: "Is this urgent?".to_string(),
        criteria: Some("yes means it blocks release".to_string()),
    };
    let value = serde_json::to_value(&question).expect("serialize noul with criteria");
    assert_eq!(value["criteria"], "yes means it blocks release");
}

#[test]
fn choice_question_serializes_with_the_type_tag_and_option_map() {
    let mut criteria = BTreeMap::new();
    criteria.insert("revert".to_string(), Some("undo the change".to_string()));
    criteria.insert("ask_human".to_string(), None);
    let question = Question::Choice {
        instructions: "Pick a fix.".to_string(),
        criteria,
    };
    let value = serde_json::to_value(&question).expect("serialize choice");
    assert_eq!(value["type"], "choice");
    assert_eq!(value["criteria"]["revert"], "undo the change");
    assert!(
        value["criteria"]["ask_human"].is_null(),
        "a null option maps to null"
    );
}

#[test]
fn score_question_serializes_with_ordered_levels() {
    let question = Question::Score {
        instructions: "Rate complexity.".to_string(),
        criteria: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
    };
    let value = serde_json::to_value(&question).expect("serialize score");
    assert_eq!(value["type"], "score");
    assert_eq!(value["criteria"][0], "low");
    assert_eq!(value["criteria"][2], "high");
}

#[test]
fn noul_answer_deserializes_from_the_type_tag() {
    let json = serde_json::json!({ "type": "noul", "noul": 0.82 });
    let answer: Answer = serde_json::from_value(json).expect("deserialize noul answer");
    match answer {
        Answer::Noul { noul } => assert!((noul - 0.82).abs() < f64::EPSILON),
        other => panic!("expected a Noul answer, got {other:?}"),
    }
}

#[test]
fn choice_answer_deserializes_with_probabilities_and_confidence() {
    let json = serde_json::json!({
        "type": "choice",
        "choice": "get_skill",
        "probabilities": { "get_skill": 0.7, "index_skills": 0.3 },
        "confidence": 0.7
    });
    let answer: Answer = serde_json::from_value(json).expect("deserialize choice answer");
    match answer {
        Answer::Choice {
            choice,
            probabilities,
            confidence,
        } => {
            assert_eq!(choice, "get_skill");
            assert_eq!(probabilities.len(), 2);
            assert!((confidence - 0.7).abs() < f64::EPSILON);
        }
        other => panic!("expected a Choice answer, got {other:?}"),
    }
}

#[test]
fn score_answer_deserializes_with_legend_and_probabilities() {
    let json = serde_json::json!({
        "type": "score",
        "score": 1.4,
        "legend": { "0": "low", "1": "medium", "2": "high" },
        "probabilities": { "0": 0.1, "1": 0.4, "2": 0.5 },
        "confidence": 0.5
    });
    let answer: Answer = serde_json::from_value(json).expect("deserialize score answer");
    match answer {
        Answer::Score {
            score,
            legend,
            probabilities,
            confidence,
        } => {
            assert!((score - 1.4).abs() < f64::EPSILON);
            assert_eq!(legend.get("1").map(String::as_str), Some("medium"));
            assert_eq!(probabilities.len(), 3);
            assert!((confidence - 0.5).abs() < f64::EPSILON);
        }
        other => panic!("expected a Score answer, got {other:?}"),
    }
}

#[test]
fn response_deserializes_with_answers_and_usage() {
    let json = serde_json::json!({
        "model": "jev-latest",
        "answers": { "drift": { "type": "noul", "noul": 0.1 } },
        "usage": { "input_tokens": 1200, "output_tokens": 0 }
    });
    let response: JevResponse = serde_json::from_value(json).expect("deserialize response");
    assert_eq!(response.model, "jev-latest");
    assert_eq!(response.answers.len(), 1);
    assert_eq!(response.usage.input_tokens, 1200);
    assert_eq!(response.usage.output_tokens, 0);
}

#[test]
fn constants_match_the_wire_contract() {
    assert_eq!(JEV_MODEL, "jev-latest");
    assert_eq!(TOKEN_BUDGET, 32_000);
    assert_eq!(BYTES_PER_TOKEN, 4);
}

#[test]
fn every_jev_error_variant_renders_a_nonempty_message() {
    // Touch every variant, so each `Display` message is proven well-formed and no variant
    // is dead code. The list is the full set from the design's error handling. Each message
    // names its offending value, expected shape, or remediation (Requirement 10).
    let all = [
        JevError::MissingApiKey {
            var: "TRUENORTH_JEV_API_KEY",
        },
        JevError::SecretResidual,
        JevError::Unauthorized {
            var: "TRUENORTH_JEV_API_KEY",
        },
        JevError::Validation {
            field: "questions".to_string(),
        },
        JevError::RateLimitedOrOverloaded {
            last_status: 529,
            attempts: 3,
        },
        JevError::UnexpectedStatus { status: 500 },
        JevError::Network {
            detail: "connection reset".to_string(),
        },
        JevError::Timeout { timeout_ms: 30_000 },
        JevError::BudgetExceeded {
            estimate: 40_000,
            budget: TOKEN_BUDGET,
        },
        JevError::MissingAnswer {
            id: "drift".to_string(),
        },
        JevError::DuplicateAnswer {
            id: "drift".to_string(),
        },
        JevError::UnexpectedOption {
            option: "PANIC".to_string(),
            expected: "REVERT, ASK_HUMAN".to_string(),
        },
        JevError::InvalidThresholds {
            low: 0.9,
            high: 0.5,
        },
        JevError::InvalidPrice { value: -1.0 },
        JevError::InvalidFixtureSet {
            path: "fixtures.yml".to_string(),
            detail: "not found".to_string(),
        },
        JevError::Config {
            path: ".agent/config/rules.yml".to_string(),
            detail: "parse error".to_string(),
        },
    ];
    for error in &all {
        assert!(!error.to_string().is_empty(), "{error:?} renders a message");
    }

    // Spot-check that key messages name the offending value and the secret variant leaks
    // nothing (Requirement 9.6, 10.1, 10.5).
    assert!(
        JevError::MissingApiKey {
            var: "TRUENORTH_JEV_API_KEY"
        }
        .to_string()
        .contains("TRUENORTH_JEV_API_KEY")
    );
    let residual = JevError::SecretResidual.to_string();
    assert!(residual.contains("secret denylist"));
    assert!(!residual.contains("password"));
}

/// A minimal in-test double that satisfies [`JevClient`], proving the trait is
/// implementable and returning a fixed answer with no network call.
struct StubClient;

impl JevClient for StubClient {
    async fn evaluate(&self, _request: JevRequest) -> Result<JevResponse, JevError> {
        Ok(JevResponse {
            model: JEV_MODEL.to_string(),
            answers: BTreeMap::from([("drift".to_string(), Answer::Noul { noul: 0.2 })]),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 0,
            },
        })
    }
}

#[tokio::test]
async fn jev_client_trait_is_implementable_and_returns_a_typed_response() {
    let client = StubClient;
    let request = JevRequest::new(serde_json::json!("plan"), BTreeMap::new());
    let response = client
        .evaluate(request)
        .await
        .expect("stub returns a response");
    assert_eq!(response.model, "jev-latest");
    match response.answers.get("drift") {
        Some(Answer::Noul { noul }) => assert!((noul - 0.2).abs() < f64::EPSILON),
        other => panic!("expected a Noul answer, got {other:?}"),
    }
}
