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
        task: None,
    }
}

#[tokio::test]
async fn a_deterministic_block_short_circuits_before_any_jev_call() {
    let repo = TempDir::new().expect("temp repo");
    let fake = FakeClient::new();
    let change = ProposedChange {
        paths: vec!["specs/plan.md".to_string()],
        content: "clean".to_string(),
        task: None,
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

#[tokio::test]
async fn each_aspect_receives_its_own_secret_filtered_state() {
    // The guard builds a state per aspect rather than one shared blob (issue #304). This
    // pins that the rigor call and the drift call each carry their own state, each assembled
    // from the change paths through the secret filter. The file on disk holds a secret line;
    // both states must drop it, proving the filter runs on each independently.
    let repo = TempDir::new().expect("temp repo");
    write_file(
        &repo,
        "src/main.rs",
        "fn main() {}\nAWS_SECRET_ACCESS_KEY=leak-me\n",
    );
    let fake = FakeClient::new();
    // Rigor call first (all low), then drift call (in scope).
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));

    let change = ProposedChange {
        paths: vec!["src/main.rs".to_string()],
        content: "fn main() {}".to_string(),
        task: None,
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

    assert_eq!(decision, GuardDecision::Allow { notes: Vec::new() });

    // Two separate calls, each with its own state.
    let calls = fake.calls();
    assert_eq!(
        calls.len(),
        2,
        "one rigor call and one drift call, each with its own state"
    );

    for request in &calls {
        // Each state is assembled from the change path.
        let serialized = serde_json::to_string(&request.state).expect("serialize the state");
        assert!(
            serialized.contains("src/main.rs"),
            "each aspect state carries the change path"
        );
        // Each state ran through the secret filter, so no denylisted line survives (P40).
        assert!(
            !serialized.contains("leak-me"),
            "each aspect state drops the secret line independently"
        );
    }
}

#[test]
fn resolve_scope_task_prefers_the_caller_task_then_the_cockpit() {
    // The caller task wins outright (#331).
    let repo = TempDir::new().expect("temp repo");
    let change = ProposedChange {
        paths: vec!["src/main.rs".to_string()],
        content: "fn main() {}".to_string(),
        task: Some("add input validation".to_string()),
    };
    assert_eq!(
        resolve_scope_task(&change, repo.path()).as_deref(),
        Some("add input validation"),
        "the caller task wins"
    );
}

#[test]
fn resolve_scope_task_falls_back_to_the_cockpit_active_task() {
    // With no caller task, the cockpit active_task fills in (#331).
    let repo = TempDir::new().expect("temp repo");
    let state_dir = repo.path().join(".agent").join("tasks");
    std::fs::create_dir_all(&state_dir).expect("mkdir");
    std::fs::write(
        state_dir.join("state.yml"),
        "active_task: refactor the cache layer\nphase: null\n",
    )
    .expect("write state");

    let change = ProposedChange {
        paths: vec!["src/main.rs".to_string()],
        content: "fn main() {}".to_string(),
        task: None,
    };
    assert_eq!(
        resolve_scope_task(&change, repo.path()).as_deref(),
        Some("refactor the cache layer"),
        "the cockpit active task is the fallback"
    );
}

#[test]
fn resolve_scope_task_is_none_when_neither_is_present() {
    // No caller task and no cockpit file resolves to None, so drift degrades gracefully.
    let repo = TempDir::new().expect("temp repo");
    let change = ProposedChange {
        paths: vec!["src/main.rs".to_string()],
        content: "fn main() {}".to_string(),
        task: None,
    };
    assert_eq!(resolve_scope_task(&change, repo.path()), None);
}
