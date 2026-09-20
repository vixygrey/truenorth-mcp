//! Example tests for `evaluate_guard` and the fail-open layer (task 3, issue #295).
//!
//! Included from `guard_evaluate.rs` via `#[path]`, so `super` is the `evaluate` module. These
//! pin the decision boundaries: the deterministic block short-circuit, the flag-off and
//! absent-key notes, the fail-open on a Jev unavailability, and a confident probabilistic block
//! (jev-active-guardrail R1.3, R3, R4, R5).

use std::collections::BTreeMap;

use tempfile::TempDir;

use super::super::super::client_fake::{FakeClient, FakeError};
use super::super::super::config::JevConfig;
use super::super::super::{Answer, JevResponse, Usage};
use super::super::{GuardDecision, NeutralizationPacket, ProposedChange};
use super::*;

/// A protected set for the example tests.
fn protected() -> Vec<String> {
    vec!["specs/".to_string(), "LICENSE".to_string()]
}

/// Write a file under the repo root, creating parent directories.
fn write_file(repo: &TempDir, rel: &str, content: &str) {
    let path = repo.path().join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(&path, content).expect("write file");
}

/// Build a rigor response: the four answers the rigor aspect reads by id.
fn rigor_response(
    hallucinated: f64,
    violates: f64,
    contains_secrets: f64,
    complexity_score: f64,
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
                Answer::Noul {
                    noul: contains_secrets,
                },
            ),
            (
                "complexity".to_string(),
                Answer::Score {
                    score: complexity_score,
                    legend: BTreeMap::new(),
                    probabilities: BTreeMap::new(),
                    confidence: 0.9,
                },
            ),
        ]),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 4,
        },
    }
}

/// Build a drift response: one Noul answer under the drift id.
fn drift_response(noul: f64) -> JevResponse {
    JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("drift".to_string(), Answer::Noul { noul })]),
        usage: Usage {
            input_tokens: 8,
            output_tokens: 1,
        },
    }
}

/// A clean proposed change over one non-protected path.
fn clean_change() -> ProposedChange {
    ProposedChange {
        paths: vec!["src/main.rs".to_string()],
        content: "fn main() {}".to_string(),
    }
}

#[tokio::test]
async fn a_deterministic_block_short_circuits_before_any_jev_call() {
    let repo = TempDir::new().expect("temp repo");
    let fake = FakeClient::new();
    let change = ProposedChange {
        paths: vec!["specs/plan.md".to_string()],
        content: "clean".to_string(),
    };

    let decision = evaluate_guard(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Block(NeutralizationPacket { violated_check, .. }) => {
            assert_eq!(violated_check, "protected-path");
        }
        other => panic!("expected a protected-path Block, got {other:?}"),
    }
    assert_eq!(
        fake.call_count(),
        0,
        "a deterministic block makes no Jev call"
    );
}

#[tokio::test]
async fn the_flag_off_returns_allow_with_a_note_and_no_call() {
    let repo = TempDir::new().expect("temp repo");
    let fake = FakeClient::new();

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        false,
        true,
        Some(&fake),
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Allow { notes } => {
            assert!(notes.iter().any(|n| n.contains("jev feature is off")));
        }
        other => panic!("expected Allow with a note, got {other:?}"),
    }
    assert_eq!(fake.call_count(), 0);
}

#[tokio::test]
async fn an_absent_key_returns_allow_with_a_note_and_no_call() {
    let repo = TempDir::new().expect("temp repo");
    let fake = FakeClient::new();

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        false,
        Some(&fake),
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Allow { notes } => {
            assert!(notes.iter().any(|n| n.contains("no jev API key")));
        }
        other => panic!("expected Allow with a note, got {other:?}"),
    }
    assert_eq!(fake.call_count(), 0);
}

#[tokio::test]
async fn an_absent_client_returns_allow_with_a_note() {
    let repo = TempDir::new().expect("temp repo");

    let decision = evaluate_guard::<FakeClient>(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        None,
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Allow { notes } => {
            assert!(notes.iter().any(|n| n.contains("no jev client")));
        }
        other => panic!("expected Allow with a note, got {other:?}"),
    }
}

#[tokio::test]
async fn a_rigor_timeout_fails_open_with_a_note() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let fake = FakeClient::new();
    // The first call is the rigor call. Queue a timeout, so the layer fails open.
    fake.push_error(FakeError::Timeout { timeout_ms: 30_000 });

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Allow { notes } => {
            assert!(
                notes
                    .iter()
                    .any(|n| n.contains("rigor scoring did not complete"))
            );
        }
        other => panic!("expected a fail-open Allow, got {other:?}"),
    }
}

#[tokio::test]
async fn a_clean_change_with_low_signals_allows() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let fake = FakeClient::new();
    // Rigor call first (all low), then drift call (in scope).
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await;

    assert_eq!(decision, GuardDecision::Allow { notes: Vec::new() });
    assert_eq!(fake.call_count(), 2, "one rigor call and one drift call");
}

#[tokio::test]
async fn a_confident_rigor_failure_blocks() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let fake = FakeClient::new();
    // Rigor: a convention violation at 0.95, above the 0.70 failure boundary and the 0.85
    // confidence high threshold. Drift: in scope.
    fake.push_response(rigor_response(0.0, 0.95, 0.0, 0.0));
    fake.push_response(drift_response(0.1));

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Block(packet) => assert_eq!(packet.violated_check, "rigor"),
        other => panic!("expected a rigor Block, got {other:?}"),
    }
}

#[tokio::test]
async fn an_unconfident_rigor_failure_annotates() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let fake = FakeClient::new();
    // Rigor: a convention violation at 0.75, above the 0.70 failure boundary but below the
    // 0.85 confidence high threshold, so it annotates rather than blocks (R5.2).
    fake.push_response(rigor_response(0.0, 0.75, 0.0, 0.0));
    fake.push_response(drift_response(0.1));

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await;

    match decision {
        GuardDecision::Annotate { notes } => {
            assert!(notes.iter().any(|n| n.starts_with("rigor:")));
        }
        other => panic!("expected an Annotate, got {other:?}"),
    }
}
