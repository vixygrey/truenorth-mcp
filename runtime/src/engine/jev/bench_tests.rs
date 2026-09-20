//! Unit tests for the benchmark data layer and runner (task 12.3).
//!
//! Included from `bench.rs` via `#[path]`, so `super` is the `bench` module and
//! `super::super` is the `jev` module. These pin the fixture load errors, the pure
//! accounting (agreement and cost), the runner behavior against the named fake, and the
//! guarded report write. Every test runs offline against the fake, so no network call
//! happens (R11.2, R11.7).

use std::collections::BTreeMap;
use std::path::Path;

use tempfile::tempdir;

use super::super::client_fake::FakeClient;
use super::super::config::{JevConfig, JevPrice};
use super::super::{Answer, JevError, JevResponse, Usage};
use super::*;

/// The routing question id the routing aspect reads.
const ROUTING_ID: &str = "route";
/// The drift question id the drift aspect reads.
const DRIFT_ID: &str = "drift";
/// The self-heal question id the self-heal aspect reads.
const SELF_HEAL_ID: &str = "self_heal";

/// Build a Choice response under `question_id` with the given choice and confidence.
fn choice_response(question_id: &str, choice: &str, confidence: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([(
            question_id.to_string(),
            Answer::Choice {
                choice: choice.to_string(),
                probabilities: BTreeMap::from([(choice.to_string(), 1.0)]),
                confidence,
            },
        )]),
        usage: Usage {
            input_tokens: 100,
            output_tokens: 20,
        },
    }
}

/// Build a Noul response under the drift id with the given noul value.
fn noul_response(noul: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([(DRIFT_ID.to_string(), Answer::Noul { noul })]),
        usage: Usage {
            input_tokens: 50,
            output_tokens: 10,
        },
    }
}

// load_fixtures --------------------------------------------------------------------------

#[test]
fn load_fixtures_missing_path_returns_invalid_fixture_set() {
    let error =
        load_fixtures(Path::new("/no/such/fixtures.yml")).expect_err("a missing path is an error");
    assert!(matches!(error, JevError::InvalidFixtureSet { .. }));
}

#[test]
fn load_fixtures_malformed_yaml_returns_invalid_fixture_set() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("bad.yml");
    std::fs::write(&path, "this: [is not: a valid fixture list").expect("write bad yaml");

    let error = load_fixtures(&path).expect_err("malformed yaml is an error");
    assert!(matches!(error, JevError::InvalidFixtureSet { .. }));
}

#[test]
fn load_fixtures_valid_yaml_returns_the_parsed_fixtures() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("fixtures.yml");
    let yaml = "\
- case_id: case-1
  state: \"a command\"
  expected:
    routing: get_skill
- case_id: case-2
  state: \"another command\"
  expected:
    drift_out_of_scope: true
    self_heal: REVERT
";
    std::fs::write(&path, yaml).expect("write fixtures");

    let fixtures = load_fixtures(&path).expect("valid yaml parses");
    assert_eq!(fixtures.len(), 2);
    assert_eq!(fixtures[0].case_id, "case-1");
    assert_eq!(fixtures[0].expected.routing.as_deref(), Some("get_skill"));
    assert_eq!(fixtures[1].expected.drift_out_of_scope, Some(true));
    assert_eq!(fixtures[1].expected.self_heal.as_deref(), Some("REVERT"));
}

// agreement ------------------------------------------------------------------------------

#[test]
fn agreement_three_of_four_is_point_seven_five() {
    assert_eq!(agreement(3, 4), 0.75);
}

#[test]
fn agreement_zero_total_is_zero() {
    assert_eq!(agreement(0, 0), 0.0);
}

// cost_estimate --------------------------------------------------------------------------

#[test]
fn cost_estimate_none_price_is_none() {
    assert!(cost_estimate(1_000_000, 500_000, None).is_none());
}

#[test]
fn cost_estimate_input_rate_only_yields_the_input_amount() {
    // One million input tokens at 2.0 per million is 2.0. A zero output rate contributes 0.
    let price = JevPrice {
        input_per_million: 2.0,
        output_per_million: 0.0,
    };
    let estimate = cost_estimate(1_000_000, 1_000_000, Some(price)).expect("a price yields Some");
    assert_eq!(estimate.amount, 2.0);
    assert!(estimate.price_unverified);
}

#[test]
fn cost_estimate_zero_output_rate_contributes_zero() {
    // The output tokens are non-zero, but the output rate is zero, so only the input rate
    // contributes (R12.2).
    let price = JevPrice {
        input_per_million: 3.0,
        output_per_million: 0.0,
    };
    let estimate = cost_estimate(2_000_000, 5_000_000, Some(price)).expect("Some");
    assert_eq!(estimate.amount, 6.0);
}

#[test]
fn cost_estimate_accounts_input_and_output_separately() {
    let price = JevPrice {
        input_per_million: 2.0,
        output_per_million: 4.0,
    };
    // 1M input at 2.0 is 2.0; 500k output at 4.0 is 2.0; the sum is 4.0.
    let estimate = cost_estimate(1_000_000, 500_000, Some(price)).expect("Some");
    assert_eq!(estimate.amount, 4.0);
}

// run_benchmark --------------------------------------------------------------------------

/// A config with a known price for the runner tests.
fn test_config() -> JevConfig {
    JevConfig {
        price: Some(JevPrice {
            input_per_million: 1.0,
            output_per_million: 1.0,
        }),
        ..JevConfig::default()
    }
}

#[tokio::test]
async fn run_benchmark_reports_latency_agreement_and_no_network_call() {
    // One fixture labels all three aspects. The runner evaluates routing, then drift, then
    // self-heal, so the fake serves three responses in that order.
    let fixtures = vec![Fixture {
        case_id: "case-1".to_string(),
        state: serde_json::json!("a command"),
        expected: ExpectedAnswers {
            routing: Some("get_skill".to_string()),
            drift_out_of_scope: Some(true),
            self_heal: Some("ASK_HUMAN".to_string()),
        },
    }];

    let fake = FakeClient::new();
    // Routing: the chosen target matches the expected answer.
    fake.push_response(choice_response(ROUTING_ID, "get_skill", 0.9));
    // Drift: a noul of 1.0 is at or over the default 0.70 boundary, so out_of_scope is true.
    fake.push_response(noul_response(1.0));
    // Self-heal: ASK_HUMAN at high confidence stays ASK_HUMAN and matches the expected answer.
    fake.push_response(choice_response(SELF_HEAL_ID, "ASK_HUMAN", 0.95));

    let config = test_config();
    let metrics = run_benchmark(&fake, &fixtures, &config).await;

    assert_eq!(
        metrics.len(),
        3,
        "three labeled aspects yield three metrics"
    );
    assert_eq!(
        fake.call_count(),
        3,
        "one call per labeled aspect, and no more"
    );

    for aspect in &metrics {
        assert_eq!(aspect.latency_ms.len(), 1, "one latency per evaluated case");
        assert_eq!(
            aspect.agreement, 1.0,
            "every outcome matched the expected answer"
        );
        assert!(
            aspect.estimated_cost.is_some(),
            "a configured price yields a cost"
        );
    }

    // Routing and self-heal carry a confidence; drift carries none.
    let routing = metrics
        .iter()
        .find(|m| m.aspect == "routing")
        .expect("routing");
    assert_eq!(routing.confidence, vec![0.9]);
    let drift = metrics.iter().find(|m| m.aspect == "drift").expect("drift");
    assert!(drift.confidence.is_empty(), "drift returns no confidence");
    // The routing usage is the one response's usage (100 input, 20 output).
    assert_eq!(routing.input_tokens, 100);
    assert_eq!(routing.output_tokens, 20);
}

#[tokio::test]
async fn run_benchmark_no_price_reports_counts_and_no_cost() {
    let fixtures = vec![Fixture {
        case_id: "case-1".to_string(),
        state: serde_json::json!("a command"),
        expected: ExpectedAnswers {
            routing: Some("get_skill".to_string()),
            ..ExpectedAnswers::default()
        },
    }];

    let fake = FakeClient::new();
    fake.push_response(choice_response(ROUTING_ID, "get_skill", 0.9));

    // The default config carries no price.
    let config = JevConfig::default();
    let metrics = run_benchmark(&fake, &fixtures, &config).await;

    assert_eq!(metrics.len(), 1, "only routing is labeled");
    let routing = &metrics[0];
    assert_eq!(routing.input_tokens, 100, "token counts are still reported");
    assert!(
        routing.estimated_cost.is_none(),
        "no price means no cost (R12.6)"
    );
}

// write_report ---------------------------------------------------------------------------

#[test]
fn write_report_writes_under_telemetry_and_carries_no_key() {
    let dir = tempdir().expect("temp dir");
    let repo_root = dir.path();
    // The write guard creates `.agent/telemetry/` as needed, but the `.agent/` root must
    // exist for the containment check to anchor on-disk.
    std::fs::create_dir_all(repo_root.join(".agent")).expect("agent dir");

    let metrics = vec![AspectMetrics {
        aspect: "routing".to_string(),
        latency_ms: vec![5],
        confidence: vec![0.9],
        agreement: 1.0,
        input_tokens: 100,
        output_tokens: 20,
        estimated_cost: None,
    }];

    let rel_path = write_report(repo_root, &metrics).expect("the report writes");
    assert_eq!(rel_path, "telemetry/jev-benchmark.yml");

    let written = repo_root.join(".agent/telemetry/jev-benchmark.yml");
    assert!(
        written.exists(),
        "the report exists under .agent/telemetry/"
    );

    let contents = std::fs::read_to_string(&written).expect("read the report");
    assert!(
        !contents.contains("TRUENORTH_JEV_API_KEY"),
        "the report carries no api key variable"
    );
    assert!(
        contents.contains("routing"),
        "the report carries the aspect name"
    );
}
