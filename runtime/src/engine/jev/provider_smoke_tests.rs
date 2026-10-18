use std::collections::BTreeMap;
use std::time::Duration;

use super::*;
use crate::engine::jev::Usage;
use crate::engine::jev::client_fake::{FakeClient, FakeError};

#[derive(Clone)]
struct FaultClient(JevError);

impl JevClient for FaultClient {
    async fn evaluate(&self, _: JevRequest) -> Result<JevResponse, JevError> {
        Err(self.0.clone())
    }
}

fn policy() -> SmokePolicy {
    SmokePolicy {
        schema_version: 1,
        approval: "approved".to_string(),
        requested_model: JEV_MODEL.to_string(),
        concrete_model: "jev-1.13.0".to_string(),
        pricing: SmokePricing {
            version: "typesafe-models-2026-10-18".to_string(),
            source: "https://docs.typesafe.ai/models".to_string(),
            currency: "USD".to_string(),
            input_per_million: 0.042,
            output_per_million: 0.0,
        },
    }
}

fn rigor_response() -> JevResponse {
    JevResponse {
        model: "jev-1.13.0".to_string(),
        answers: BTreeMap::from([
            ("hallucinated_import".into(), Answer::Noul { noul: 0.7 }),
            ("violates_conventions".into(), Answer::Noul { noul: 0.2 }),
            ("contains_secrets".into(), Answer::Noul { noul: 0.1 }),
            (
                "complexity".into(),
                Answer::Score {
                    score: 1.4,
                    legend: BTreeMap::from([
                        ("0".into(), "trivial".into()),
                        ("1".into(), "simple".into()),
                        ("2".into(), "moderate".into()),
                        ("3".into(), "complex".into()),
                        ("4".into(), "very complex".into()),
                    ]),
                    probabilities: BTreeMap::from([
                        ("0".into(), 0.0),
                        ("1".into(), 0.6),
                        ("2".into(), 0.4),
                        ("3".into(), 0.0),
                        ("4".into(), 0.0),
                    ]),
                    confidence: 0.5,
                },
            ),
        ]),
        usage: Usage {
            input_tokens: 100,
            output_tokens: 30,
        },
    }
}

fn drift_response() -> JevResponse {
    JevResponse {
        model: "jev-1.13.0".into(),
        answers: BTreeMap::from([("drift".into(), Answer::Noul { noul: 0.2 })]),
        usage: Usage {
            input_tokens: 80,
            output_tokens: 10,
        },
    }
}

#[test]
fn rejects_model_drift_bad_answers_and_unknown_usage() {
    let policy = policy();
    policy.validate().expect("valid provenance");
    let mut result = rigor_response();
    result.model = "jev-1.14.0".into();
    assert_eq!(
        inspect(&result, &policy, Aspect::Rigor),
        Err("unexpected concrete responder model")
    );
    result.model = policy.concrete_model.clone();
    result.answers.remove("complexity");
    assert_eq!(
        inspect(&result, &policy, Aspect::Rigor),
        Err("missing required answers")
    );
    result = rigor_response();
    result.usage.input_tokens = 0;
    assert_eq!(
        inspect(&result, &policy, Aspect::Rigor),
        Err("missing token usage")
    );
    result = rigor_response();
    result.answers.insert(
        "hallucinated_import".into(),
        Answer::Noul { noul: f64::NAN },
    );
    assert_eq!(
        inspect(&result, &policy, Aspect::Rigor),
        Err("invalid Noul answer")
    );
    result = drift_response();
    result
        .answers
        .insert("drift".into(), Answer::Noul { noul: 1.5 });
    assert_eq!(
        inspect(&result, &policy, Aspect::Drift),
        Err("invalid Noul answer")
    );
}

#[test]
fn rejects_invalid_policy_before_network() {
    let mut candidate = policy();
    candidate.concrete_model = "jev-latest".into();
    assert!(candidate.validate().is_err());
    candidate = policy();
    candidate.pricing.input_per_million = f64::NAN;
    assert!(candidate.validate().is_err());
    candidate = policy();
    candidate.pricing.source = "https://evil.example/secret?key=secret".into();
    assert_eq!(candidate.validate(), Err("invalid pricing provenance"));
}

#[tokio::test]
async fn fake_proves_wire_shape_and_accounting_without_vendor_claim() {
    let fake = FakeClient::new();
    fake.push_response(rigor_response());
    fake.push_response(drift_response());
    let counts = compatibility(&fake, &policy(), &JevConfig::default(), 1.0)
        .await
        .expect("typed production contract");
    assert_eq!(counts.input_tokens, 180);
    assert_eq!(counts.output_tokens, 40);
    assert!((counts.estimated_cost_usd - 0.00000756).abs() < f64::EPSILON);
    let calls = fake.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].model, JEV_MODEL);
    let rigor_wire = serde_json::to_value(&calls[0]).expect("rigor wire request");
    assert_eq!(
        rigor_wire["questions"]["hallucinated_import"]["type"],
        "noul"
    );
    assert_eq!(rigor_wire["questions"]["complexity"]["type"], "score");
    let drift_wire = serde_json::to_value(&calls[1]).expect("drift wire request");
    assert_eq!(drift_wire["questions"]["drift"]["type"], "noul");
}

#[tokio::test]
async fn spending_ceiling_stops_before_second_production_request() {
    let fake = FakeClient::new();
    fake.push_response(rigor_response());
    assert_eq!(
        compatibility(&fake, &policy(), &JevConfig::default(), 0.000001).await,
        Err("reviewed spending ceiling reached")
    );
    assert_eq!(fake.call_count(), 1);
}

#[tokio::test]
async fn concrete_model_must_match_on_both_production_calls() {
    let fake = FakeClient::new();
    fake.push_response(rigor_response());
    let mut drift = drift_response();
    drift.model = "jev-1.14.0".into();
    fake.push_response(drift);
    assert_eq!(
        compatibility(&fake, &policy(), &JevConfig::default(), 1.0).await,
        Err("production drift evaluation failed")
    );
    assert_eq!(fake.call_count(), 2);
}

#[tokio::test]
async fn errors_never_include_provider_payload() {
    let fake = FakeClient::new();
    fake.push_error(FakeError::Network {
        detail: "URL, key, and raw provider payload".into(),
    });
    let error = compatibility(&fake, &policy(), &JevConfig::default(), 1.0)
        .await
        .expect_err("failed provider call");
    assert_eq!(error, "production rigor evaluation failed");
    fake.push_error(FakeError::Timeout { timeout_ms: 5 });
    assert!(timeout(&fake).await.is_ok());
    fake.push_response(drift_response());
    assert_eq!(
        retry(&fake).await,
        Err("provider retriable status not demonstrated")
    );
}

#[test]
fn controlled_origin_must_match_approved_provider() {
    assert!(same_origin(
        "https://api.typesafe.ai/v1/systemone",
        "https://api.typesafe.ai/approved-timeout-control",
    ));
    for unsafe_url in [
        "https://api.typesafe.ai.evil.example/retry",
        "https://sub.typesafe.ai/retry",
        "https://api.typesafe.ai:8443/retry",
        "http://api.typesafe.ai/retry",
        "https://test:secret@api.typesafe.ai/retry",
    ] {
        assert!(!same_origin(
            "https://api.typesafe.ai/v1/systemone",
            unsafe_url
        ));
    }
}

#[test]
fn pending_or_missing_live_evidence_never_passes() {
    let mut policy = policy();
    let passing = [Outcome::Passed; 4];
    assert!(accepted(&policy, passing, Duration::from_secs(10)));
    policy.approval = "pending".into();
    assert!(!accepted(&policy, passing, Duration::from_secs(10)));
    policy.approval = "approved".into();
    assert!(!accepted(
        &policy,
        [
            Outcome::Passed,
            Outcome::Passed,
            Outcome::NotProven,
            Outcome::Passed
        ],
        Duration::from_secs(10)
    ));
    assert!(!accepted(&policy, passing, MAX_ELAPSED));
}

#[tokio::test]
async fn typed_fault_scenarios_require_exact_provider_status_and_attempts() {
    assert!(
        unauthorized(&FaultClient(JevError::Unauthorized {
            var: "TRUENORTH_JEV_API_KEY",
        }))
        .await
        .is_ok()
    );
    for status in [429, 529] {
        assert_eq!(
            retry(&FaultClient(JevError::RateLimitedOrOverloaded {
                last_status: status,
                attempts: 2,
            }))
            .await,
            Ok(status)
        );
        assert!(
            retry(&FaultClient(JevError::RateLimitedOrOverloaded {
                last_status: status,
                attempts: 1,
            }))
            .await
            .is_err()
        );
    }
    assert!(
        unauthorized(&FaultClient(JevError::Network {
            detail: "provider payload never logged".into(),
        }))
        .await
        .is_err()
    );
}
