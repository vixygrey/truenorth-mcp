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
    // The plain string form through the constructor. It must serialize byte-identically to
    // the earlier `String` field: a bare JSON string (issue #305 backward compatibility).
    let question = Question::noul("Is this out of scope?", None);
    let value = serde_json::to_value(&question).expect("serialize noul");
    assert_eq!(value["type"], "noul");
    assert_eq!(value["instructions"], "Is this out of scope?");
    assert!(
        value["instructions"].is_string(),
        "the plain form stays a bare string"
    );
    // An absent criteria is skipped, not serialized as null.
    assert!(value.get("criteria").is_none(), "None criteria is skipped");
}

#[test]
fn noul_question_serializes_present_criteria() {
    let question = Question::noul(
        "Is this urgent?",
        Some("yes means it blocks release".into()),
    );
    let value = serde_json::to_value(&question).expect("serialize noul with criteria");
    assert_eq!(value["criteria"], "yes means it blocks release");
}

#[test]
fn choice_question_serializes_with_the_type_tag_and_option_map() {
    let mut criteria = BTreeMap::new();
    criteria.insert("revert".to_string(), Some("undo the change".into()));
    criteria.insert("ask_human".to_string(), None);
    let question = Question::choice("Pick a fix.", criteria);
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
    let question = Question::score(
        "Rate complexity.",
        vec!["low".into(), "medium".into(), "high".into()],
    );
    let value = serde_json::to_value(&question).expect("serialize score");
    assert_eq!(value["type"], "score");
    assert_eq!(value["criteria"][0], "low");
    assert_eq!(value["criteria"][2], "high");
}

#[test]
fn noul_question_serializes_a_structured_object_instruction() {
    // The enriched form: an object instruction with caller-chosen field names. The endpoint
    // sends the keys and values to the model as data, so the wire carries the object verbatim
    // (issue #305, docs.typesafe.ai "Structured instructions and criteria").
    let question = Question::noul(
        serde_json::json!({
            "question": "Is the change out of scope?",
            "focus": "Judge scope, not quality.",
        }),
        None,
    );
    let value = serde_json::to_value(&question).expect("serialize noul with object instruction");
    assert_eq!(value["type"], "noul");
    assert!(
        value["instructions"].is_object(),
        "the object instruction round-trips"
    );
    assert_eq!(value["instructions"]["focus"], "Judge scope, not quality.");
}

#[test]
fn choice_question_serializes_a_structured_criteria_value() {
    // An option described by an object with `what`, `not_for`, and `examples` fields. The
    // field names are caller-chosen and unreserved; the wire carries them verbatim.
    let mut criteria = BTreeMap::new();
    criteria.insert(
        "return_status".to_string(),
        Some(serde_json::json!({
            "what": "Progress of a return already sent",
            "not_for": "Whether an item can be returned",
            "examples": ["Has my return arrived yet?"],
        })),
    );
    let question = Question::choice("Which returns topic?", criteria);
    let value = serde_json::to_value(&question).expect("serialize choice with object criteria");
    let option = &value["criteria"]["return_status"];
    assert!(option.is_object(), "the object criteria round-trips");
    assert_eq!(option["what"], "Progress of a return already sent");
    assert_eq!(option["examples"][0], "Has my return arrived yet?");
}

#[test]
fn score_question_serializes_structured_levels() {
    // A Score level described by an object. The ordered array of levels carries objects
    // verbatim, alongside the plain-string level form.
    let question = Question::score(
        "Rate urgency.",
        vec![
            "can wait a week".into(),
            serde_json::json!({ "level": "blocked", "meaning": "someone is blocked right now" }),
        ],
    );
    let value = serde_json::to_value(&question).expect("serialize score with object level");
    assert_eq!(value["criteria"][0], "can wait a week");
    assert!(
        value["criteria"][1].is_object(),
        "an object level round-trips"
    );
    assert_eq!(
        value["criteria"][1]["meaning"],
        "someone is blocked right now"
    );
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

#[test]
fn small_request_is_under_budget() {
    let request = JevRequest::new(serde_json::json!("state"), BTreeMap::new());
    assert!(estimate_tokens(&request) <= TOKEN_BUDGET);
    assert!(guard_budget(&request).is_ok());
}

#[test]
fn huge_state_is_over_budget() {
    // A state string well over four times the budget guarantees an over-budget estimate.
    let huge = "x".repeat(TOKEN_BUDGET * BYTES_PER_TOKEN * 2);
    let request = JevRequest::new(serde_json::json!(huge), BTreeMap::new());
    match guard_budget(&request) {
        Err(JevError::BudgetExceeded { estimate, budget }) => {
            assert_eq!(estimate, estimate_tokens(&request));
            assert_eq!(budget, TOKEN_BUDGET);
            assert!(estimate > TOKEN_BUDGET);
        }
        other => panic!("expected BudgetExceeded, got {other:?}"),
    }
}

#[test]
fn estimate_tokens_rounds_up() {
    // A JSON string value of 7 characters serializes to 9 bytes: the seven characters plus
    // the two surrounding quotes. The estimate is ceil(9 / 4) = 3.
    let request = JevRequest::new(serde_json::json!("1234567"), BTreeMap::new());
    let json = serde_json::to_string(&request).expect("serialize request");
    let request_bytes = json.len();
    // The estimate is the serialized request byte length divided by four, rounded up.
    assert_eq!(
        estimate_tokens(&request),
        request_bytes.div_ceil(BYTES_PER_TOKEN)
    );

    // Prove the round-up directly on a known 9-byte value.
    let nine = serde_json::json!("1234567");
    assert_eq!(serde_json::to_string(&nine).expect("serialize").len(), 9);
    assert_eq!(9usize.div_ceil(BYTES_PER_TOKEN), 3);
}

#[test]
fn guard_budget_boundary_at_and_over_the_budget() {
    // Grow the state one byte at a time until the estimate reaches the budget. Assert Ok at
    // the last state under or equal to the budget, and Err at the first state over it.
    let mut size = TOKEN_BUDGET * BYTES_PER_TOKEN - 200;
    let mut last_ok: Option<usize> = None;
    let mut first_over: Option<usize> = None;
    while size < TOKEN_BUDGET * BYTES_PER_TOKEN + 200 {
        let state = "y".repeat(size);
        let request = JevRequest::new(serde_json::json!(state), BTreeMap::new());
        let estimate = estimate_tokens(&request);
        if estimate <= TOKEN_BUDGET {
            assert!(guard_budget(&request).is_ok(), "at or under budget is Ok");
            last_ok = Some(estimate);
        } else {
            assert!(
                matches!(guard_budget(&request), Err(JevError::BudgetExceeded { .. })),
                "over budget is BudgetExceeded"
            );
            if first_over.is_none() {
                first_over = Some(estimate);
            }
        }
        size += 1;
    }
    // The scan crossed the boundary: it saw at least one Ok and at least one over.
    assert!(
        last_ok.is_some(),
        "the scan saw an at-or-under-budget request"
    );
    assert!(first_over.is_some(), "the scan saw an over-budget request");
}
