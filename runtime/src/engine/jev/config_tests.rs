//! Unit tests for the Jev harness config resolution.
//!
//! Included from `config.rs` via `#[path]`, so `super` is the jev config module. These
//! cover the absent-file default, the read and parse errors, a valid round-trip including
//! the millisecond durations, the threshold and price rejections, and the accepted boundary
//! values (Requirement 1.6, 1.7, 8.7, 12.5).

use std::fs;
use std::time::Duration;

use tempfile::TempDir;

use super::*;

/// Seed `.agent/config/rules.yml` with the given body under a fresh temp repo root.
fn seed_rules(body: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let config = repo.path().join(AGENT_DIR).join("config");
    fs::create_dir_all(&config).expect("create .agent/config");
    fs::write(config.join("rules.yml"), body).expect("write rules.yml");
    repo
}

/// A valid `jev` block body, with a price and non-default durations.
const VALID_BLOCK: &str = "\
jev:
  confidence_high: 0.80
  confidence_low: 0.40
  drift_boundary: 0.65
  rigor_failure_boundary: 0.55
  complexity_boundary: 60
  pruning_keep_threshold: 0.45
  destructive_threshold: 0.95
  retry_limit: 5
  max_backoff_ms: 4000
  timeout_ms: 15000
  price:
    input_per_million: 0.042
    output_per_million: 0.0
";

#[test]
fn absent_file_resolves_to_default() {
    let repo = TempDir::new().expect("temp repo");
    let resolved = resolve(repo.path()).expect("absent file resolves");
    assert_eq!(resolved.confidence_high, 0.85);
    assert_eq!(resolved.confidence_low, 0.60);
    assert_eq!(resolved.retry_limit, 3);
    assert_eq!(resolved.max_backoff, Duration::from_secs(8));
    assert_eq!(resolved.timeout, Duration::from_secs(30));
    assert!(resolved.price.is_none());
}

#[test]
fn absent_jev_block_resolves_to_default() {
    let repo = seed_rules("features:\n  ontology: true\n");
    let resolved = resolve(repo.path()).expect("absent jev block resolves");
    assert_eq!(resolved.confidence_high, 0.85);
    assert!(resolved.price.is_none());
}

#[test]
fn unreadable_config_returns_config_error() {
    // A directory at the rules.yml path makes read_to_string fail with a non-NotFound kind.
    let repo = TempDir::new().expect("temp repo");
    let config = repo.path().join(AGENT_DIR).join("config");
    fs::create_dir_all(&config).expect("create .agent/config");
    fs::create_dir_all(config.join("rules.yml")).expect("create dir at rules.yml path");

    let error = resolve(repo.path()).expect_err("unreadable config rejected");
    match error {
        super::super::JevError::Config { path, .. } => {
            assert!(path.contains("rules.yml"), "message names the config path");
        }
        other => panic!("expected Config, got {other:?}"),
    }
}

#[test]
fn malformed_config_returns_config_error() {
    // A scalar where a mapping is expected fails to deserialize into RulesJevView.
    let repo = seed_rules("just a bare string\n");
    let error = resolve(repo.path()).expect_err("malformed config rejected");
    match error {
        super::super::JevError::Config { path, .. } => {
            assert!(path.contains("rules.yml"), "message names the config path");
        }
        other => panic!("expected Config, got {other:?}"),
    }
}

#[test]
fn valid_block_round_trips_values_including_durations() {
    let repo = seed_rules(VALID_BLOCK);
    let resolved = resolve(repo.path()).expect("valid block resolves");
    assert_eq!(resolved.confidence_high, 0.80);
    assert_eq!(resolved.confidence_low, 0.40);
    assert_eq!(resolved.drift_boundary, 0.65);
    assert_eq!(resolved.rigor_failure_boundary, 0.55);
    assert_eq!(resolved.complexity_boundary, 60.0);
    assert_eq!(resolved.pruning_keep_threshold, 0.45);
    assert_eq!(resolved.destructive_threshold, 0.95);
    assert_eq!(resolved.retry_limit, 5);
    assert_eq!(resolved.max_backoff, Duration::from_millis(4000));
    assert_eq!(resolved.timeout, Duration::from_millis(15000));
    let price = resolved.price.expect("price present");
    assert_eq!(price.input_per_million, 0.042);
    assert_eq!(price.output_per_million, 0.0);
}

#[test]
fn unrelated_key_is_tolerated() {
    let body = "token_caps:\n  max: 1000\njev:\n  confidence_high: 0.70\n  confidence_low: 0.30\n";
    let repo = seed_rules(body);
    let resolved = resolve(repo.path()).expect("unrelated key tolerated");
    assert_eq!(resolved.confidence_high, 0.70);
    assert_eq!(resolved.confidence_low, 0.30);
}

#[test]
fn low_greater_than_high_is_rejected() {
    let body =
        "jev:\n  confidence_high: 0.50\n  confidence_low: 0.90\n  destructive_threshold: 0.95\n";
    let repo = seed_rules(body);
    match resolve(repo.path()).expect_err("low>high rejected") {
        super::super::JevError::InvalidThresholds { low, high } => {
            assert_eq!(low, 0.90);
            assert_eq!(high, 0.50);
        }
        other => panic!("expected InvalidThresholds, got {other:?}"),
    }
}

#[test]
fn destructive_below_high_is_rejected() {
    let body =
        "jev:\n  confidence_high: 0.85\n  confidence_low: 0.60\n  destructive_threshold: 0.70\n";
    let repo = seed_rules(body);
    match resolve(repo.path()).expect_err("destructive<high rejected") {
        super::super::JevError::InvalidThresholds { .. } => {}
        other => panic!("expected InvalidThresholds, got {other:?}"),
    }
}

#[test]
fn out_of_range_unit_boundary_is_rejected() {
    let body = "jev:\n  drift_boundary: 1.50\n";
    let repo = seed_rules(body);
    match resolve(repo.path()).expect_err("out-of-range boundary rejected") {
        super::super::JevError::Config { detail, .. } => {
            assert!(detail.contains("drift_boundary"), "detail names the field");
            assert!(detail.contains("0 to 1"), "detail names the range");
        }
        other => panic!("expected Config, got {other:?}"),
    }
}

#[test]
fn out_of_range_complexity_boundary_is_rejected() {
    let body = "jev:\n  complexity_boundary: 150\n";
    let repo = seed_rules(body);
    match resolve(repo.path()).expect_err("out-of-range complexity rejected") {
        super::super::JevError::Config { detail, .. } => {
            assert!(
                detail.contains("complexity_boundary"),
                "detail names the field"
            );
            assert!(detail.contains("0 to 100"), "detail names the range");
        }
        other => panic!("expected Config, got {other:?}"),
    }
}

#[test]
fn negative_price_is_rejected() {
    let body = "jev:\n  price:\n    input_per_million: -1.0\n    output_per_million: 0.0\n";
    let repo = seed_rules(body);
    match resolve(repo.path()).expect_err("negative price rejected") {
        super::super::JevError::InvalidPrice { value } => assert_eq!(value, -1.0),
        other => panic!("expected InvalidPrice, got {other:?}"),
    }
}

#[test]
fn boundary_values_at_zero_one_and_equal_thresholds_are_accepted() {
    // confidence_low == confidence_high, thresholds at exact 0 and 1 edges, complexity at
    // both edges. destructive_threshold at 1.0 satisfies the >= high rule.
    let body = "\
jev:
  confidence_high: 1.0
  confidence_low: 1.0
  drift_boundary: 0.0
  rigor_failure_boundary: 1.0
  complexity_boundary: 0
  pruning_keep_threshold: 1.0
  destructive_threshold: 1.0
";
    let repo = seed_rules(body);
    let resolved = resolve(repo.path()).expect("edge values accepted");
    assert_eq!(resolved.confidence_high, 1.0);
    assert_eq!(resolved.confidence_low, 1.0);
    assert_eq!(resolved.drift_boundary, 0.0);
    assert_eq!(resolved.complexity_boundary, 0.0);
}

#[test]
fn complexity_at_hundred_is_accepted() {
    let body = "jev:\n  complexity_boundary: 100\n";
    let repo = seed_rules(body);
    let resolved = resolve(repo.path()).expect("complexity 100 accepted");
    assert_eq!(resolved.complexity_boundary, 100.0);
}
