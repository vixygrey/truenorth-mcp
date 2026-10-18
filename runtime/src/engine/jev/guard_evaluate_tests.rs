//! Production guard regressions: deterministic blocks, advisory findings and filtered
//! proposal-aware context are exercised with an injected client and no network.

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
    .await
    .expect("safe paths");

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
    .await
    .expect("safe paths");

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
    .await
    .expect("safe paths");

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
    .await
    .expect("safe paths");

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
    .await
    .expect("safe paths");

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
    .await
    .expect("safe paths");

    assert!(
        matches!(decision, GuardDecision::Allow { notes } if notes.iter().any(|n| n.contains("CONVENTIONS.md unavailable")))
    );
    assert_eq!(fake.call_count(), 2, "one rigor call and one drift call");
}

#[tokio::test]
async fn a_strong_rigor_finding_is_advisory() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.95, 0.0, 0.0));
    fake.push_response(drift_response(0.1));
    let (decision, observed) = evaluate_guard_observed(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect("safe paths");
    assert!(
        matches!(decision, GuardDecision::Annotate { notes } if notes.iter().any(|n| n.starts_with("rigor:")))
    );
    assert_eq!(observed.rigor.unwrap().violates_conventions, 0.95);
    assert!(observed.unavailable.is_none());
}

#[tokio::test]
async fn a_finding_above_its_own_boundary_is_advisory() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let fake = FakeClient::new();
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
    .await
    .expect("safe paths");

    match decision {
        GuardDecision::Annotate { notes } => {
            assert!(notes.iter().any(|n| n.starts_with("rigor:")));
        }
        other => panic!("expected advisory finding, got {other:?}"),
    }
}

#[tokio::test]
async fn secret_boundary_produces_advice_at_its_own_detection_boundary() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/main.rs", "fn main() {}");
    let config = JevConfig::default();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.69, 0.69, config.secret_boundary, 2.0));
    fake.push_response(drift_response(0.1));

    let decision = evaluate_guard(
        &clean_change(),
        &protected(),
        &config,
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect("safe paths");
    assert!(matches!(decision, GuardDecision::Annotate { .. }));
    assert_eq!(fake.call_count(), 2);
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
    .await
    .expect("safe paths");

    assert!(matches!(decision, GuardDecision::Allow { .. }));

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
        state_dir.join("state.json"),
        r#"{"active_task": "refactor the cache layer", "phase": null}"#,
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

#[tokio::test]
async fn mixed_unsafe_paths_reject_before_any_client_call_even_offline() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/good.rs", "safe source");
    let fake = FakeClient::new();
    for bad in [
        "/etc/passwd",
        "../outside.rs",
        "src/../../outside.rs",
        "src\\outside.rs",
        ".env",
        "config/.env",
        "config/credentials/key.txt",
        ".agent/telemetry/jev-report.yml",
    ] {
        let change = ProposedChange {
            paths: vec!["src/good.rs".to_string(), bad.to_string()],
            content: "clean".to_string(),
            task: None,
        };
        for enabled in [false, true] {
            let result = evaluate_guard(
                &change,
                &protected(),
                &JevConfig::default(),
                enabled,
                true,
                Some(&fake),
                repo.path(),
            )
            .await;
            assert!(
                matches!(
                    &result,
                    Err(super::super::super::JevError::InvalidGuardPath { .. })
                ),
                "must reject {bad:?} before any client call: {result:?}"
            );
            assert_eq!(fake.call_count(), 0);
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_file_and_directory_reject_before_any_client_call() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("temp repo");
    let outside = TempDir::new().expect("outside");
    write_file(&repo, "src/good.rs", "safe source");
    std::fs::write(outside.path().join("private.rs"), "private").expect("outside file");
    symlink(
        outside.path().join("private.rs"),
        repo.path().join("shortcut.rs"),
    )
    .expect("file symlink");
    symlink(outside.path(), repo.path().join("linked")).expect("directory symlink");
    let fake = FakeClient::new();

    for bad in ["shortcut.rs", "linked/private.rs"] {
        let change = ProposedChange {
            paths: vec!["src/good.rs".to_string(), bad.to_string()],
            content: "clean".to_string(),
            task: None,
        };
        let result = evaluate_guard(
            &change,
            &protected(),
            &JevConfig::default(),
            true,
            true,
            Some(&fake),
            repo.path(),
        )
        .await;
        assert!(
            matches!(
                &result,
                Err(super::super::super::JevError::InvalidGuardPath { .. })
            ),
            "symlink {bad} must fail before a call: {result:?}"
        );
        assert_eq!(fake.call_count(), 0);
    }
}

#[tokio::test]
async fn valid_file_and_missing_proposed_file_make_correct_state() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "src/good.rs", "safe source");
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));
    let change = ProposedChange {
        paths: vec!["src/good.rs".to_string(), "src/new.rs".to_string()],
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
    .await
    .expect("missing proposed file is legitimate");
    assert!(matches!(decision, GuardDecision::Allow { .. }));
    let calls = fake.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].state["proposal"]["paths"],
        serde_json::json!(["src/good.rs", "src/new.rs"])
    );
    assert_eq!(calls[0].state["proposal"]["content"], "clean");
    assert_eq!(
        calls[0].state["existing_context"],
        serde_json::json!([{"path": "src/good.rs", "content": "safe source"}])
    );
    assert_eq!(
        calls[1].state["changed_paths"],
        serde_json::json!(["src/good.rs", "src/new.rs"])
    );
}

#[tokio::test]
async fn violating_proposal_replaces_clean_disk_and_clean_proposal_does_not_inherit_disk() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "CONVENTIONS.md", "No unwrap in production code.");
    write_file(&repo, "Cargo.toml", "[dependencies]\nserde = \"1\"\n");
    write_file(
        &repo,
        "src/main.rs",
        "fn disk() { panic!(\"old behavior\") }",
    );
    let fake = FakeClient::new();
    for _ in 0..2 {
        fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
        fake.push_response(drift_response(0.1));
    }
    let mut change = clean_change();
    change.content = "fn proposed() { unwrap() }".to_string();
    let result = evaluate_guard(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();
    assert!(matches!(result, GuardDecision::Allow { .. }));
    change.content = "fn proposed() {}".to_string();
    evaluate_guard(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();
    let requests = fake.calls();
    assert_eq!(
        requests[0].state["proposal"]["content"],
        "fn proposed() { unwrap() }"
    );
    assert_eq!(requests[2].state["proposal"]["content"], "fn proposed() {}");
    assert_eq!(
        requests[0].state["existing_context"][0]["content"],
        "fn disk() { panic!(\"old behavior\") }"
    );
    assert_eq!(
        requests[2].state["project_context"][0]["content"],
        "No unwrap in production code."
    );
    assert_eq!(
        requests[2].state["project_context"][1]["content"],
        "[dependencies]\nserde = \"1\""
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("src/main.rs")).unwrap(),
        "fn disk() { panic!(\"old behavior\") }"
    );
}

#[tokio::test]
async fn missing_parent_and_multiple_paths_send_one_logical_proposal_without_writing() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));
    let change = ProposedChange {
        paths: vec!["fresh/a.rs".to_string(), "fresh/b.rs".to_string()],
        content: "fn proposed() {}".to_string(),
        task: Some("Add the new module".to_string()),
    };
    evaluate_guard(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();
    let requests = fake.calls();
    assert_eq!(requests[0].state["proposal"]["content"], "fn proposed() {}");
    assert_eq!(requests[0].state["existing_context"], serde_json::json!([]));
    assert_eq!(requests[1].state["task"], "Add the new module");
    assert!(requests[1].state.get("proposed_content").is_none());
    assert!(!repo.path().join("fresh").exists());
}

#[tokio::test]
async fn strong_drift_and_rigor_findings_remain_separate_visible_advice() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.99, 0.95, 0.5, 4.0));
    fake.push_response(drift_response(0.9));
    let (decision, observed) = evaluate_guard_observed(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();
    assert!(
        matches!(decision, GuardDecision::Annotate { notes } if notes.iter().any(|n| n.starts_with("rigor:")) && notes.iter().any(|n| n.starts_with("drift:")))
    );
    assert_eq!(observed.rigor.unwrap().complexity, 100.0);
    assert_eq!(observed.drift.unwrap().noul, 0.9);
}

#[tokio::test]
async fn drift_failure_keeps_prior_rigor_finding_and_reports_unavailability() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.9, 0.0, 0.0));
    fake.push_error(FakeError::Timeout { timeout_ms: 30_000 });
    let (decision, observation) = evaluate_guard_observed(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();
    assert!(
        matches!(decision, GuardDecision::Annotate { notes } if notes.iter().any(|n| n.starts_with("rigor:")) && notes.iter().any(|n| n.contains("drift scoring did not complete")))
    );
    assert_eq!(observation.rigor.unwrap().violates_conventions, 0.9);
    assert!(observation.drift.is_none());
    assert!(
        observation
            .unavailable
            .unwrap()
            .contains("drift scoring did not complete")
    );
    assert_eq!(
        fake.call_count(),
        2,
        "rigor completed before drift timed out"
    );
    let requests = fake.calls();
    assert_eq!(requests[0].state["proposal"]["content"], "fn main() {}");
    assert!(requests[1].state.get("proposal").is_none());
    assert!(requests[1].state.get("existing_context").is_none());
}

#[tokio::test]
async fn malformed_rigor_response_is_visible_unavailability() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 4.5));
    let (decision, observation) = evaluate_guard_observed(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();
    assert!(
        matches!(decision, GuardDecision::Allow { notes } if notes.iter().any(|note| note.contains("invalid value for question id `complexity`")))
    );
    assert!(observation.rigor.is_none());
    assert!(observation.unavailable.unwrap().contains("complexity"));
    assert_eq!(fake.call_count(), 1);
}

#[tokio::test]
async fn non_utf8_and_oversized_disk_context_are_advisory_not_replacement_proposals() {
    let repo = TempDir::new().unwrap();
    write_file(&repo, "src/large.rs", &"DISK_CANARY".repeat(4_000));
    write_file(&repo, "src/main.rs", "placeholder");
    std::fs::write(repo.path().join("src/main.rs"), b"binary\xffDISK_CANARY").unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));
    let change = ProposedChange {
        paths: vec!["src/main.rs".to_string(), "src/large.rs".to_string()],
        content: "fn proposed() {}".to_string(),
        task: Some("Update parsing".to_string()),
    };

    let (decision, observation) = evaluate_guard_observed(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect("unavailable existing context is advisory");

    assert!(matches!(decision, GuardDecision::Allow { notes } if
        notes.iter().any(|note| note.contains("src/main.rs") && note.contains("non-UTF-8")) &&
        notes.iter().any(|note| note.contains("src/large.rs") && note.contains("exceeds"))));
    assert!(observation.rigor.is_some());
    assert!(observation.drift.is_some());
    assert_eq!(
        observation.unavailable_context.len(),
        4,
        "both targets and missing project context are reported"
    );
    let requests = fake.calls();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].state["proposal"]["content"], "fn proposed() {}");
    assert_eq!(requests[0].state["existing_context"], serde_json::json!([]));
    for request in requests {
        assert!(!request.state.to_string().contains("DISK_CANARY"));
        assert!(!request.state.to_string().contains("binary"));
    }
    assert_eq!(
        std::fs::read(repo.path().join("src/main.rs"))
            .unwrap()
            .as_slice(),
        b"binary\xffDISK_CANARY"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn unsafe_optional_context_is_reported_without_transmitting_an_outside_canary() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::fs::write(outside.path().join("project.md"), "EXTERNAL_RULE_CANARY").unwrap();
    std::fs::write(outside.path().join("package.json"), "EXTERNAL_DEP_CANARY").unwrap();
    symlink(
        outside.path().join("project.md"),
        repo.path().join("CONVENTIONS.md"),
    )
    .unwrap();
    symlink(
        outside.path().join("package.json"),
        repo.path().join("package.json"),
    )
    .unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));
    let change = ProposedChange {
        paths: vec!["src/new.ts".to_string()],
        content: "export const item = 1;".to_string(),
        task: Some("Add item".to_string()),
    };

    let (decision, observation) = evaluate_guard_observed(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect("optional symlinks should not fail the proposal");

    assert!(matches!(decision, GuardDecision::Allow { notes } if
        notes.iter().any(|note| note.contains("CONVENTIONS.md unavailable: unsafe")) &&
        notes.iter().any(|note| note.contains("package.json unavailable: unsafe"))));
    assert!(observation.unavailable.is_none());
    let requests = fake.calls();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].state["project_context"], serde_json::json!([]));
    for request in requests {
        assert!(!request.state.to_string().contains("EXTERNAL_RULE_CANARY"));
        assert!(!request.state.to_string().contains("EXTERNAL_DEP_CANARY"));
    }
}

#[tokio::test]
async fn absent_task_is_explicitly_reported_but_drift_still_evaluates() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));

    let (decision, observation) = evaluate_guard_observed(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();

    let missing_task = "scope task unavailable: judging paths against protected paths only";
    assert!(matches!(decision, GuardDecision::Allow { notes } if
        notes.iter().any(|note| note == missing_task)));
    assert!(
        observation
            .unavailable_context
            .iter()
            .any(|note| note == missing_task)
    );
    assert!(observation.drift.is_some());
    let requests = fake.calls();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].state.get("task").is_none());
    assert!(
        requests[1].state["scope_note"]
            .as_str()
            .unwrap()
            .contains("do not infer a scope violation")
    );
    assert!(requests[1].state.get("proposal").is_none());
}

#[tokio::test]
async fn residual_task_rejects_drift_transmission_but_retains_rigor_finding() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.95, 0.0, 0.0));
    let change = ProposedChange {
        paths: vec!["src/new.rs".to_string()],
        content: "fn proposed() {}".to_string(),
        task: Some("Handle secret-scope-canary".to_string()),
    };

    let (decision, observation) = evaluate_guard_observed(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();

    assert!(matches!(decision, GuardDecision::Annotate { notes } if
        notes.iter().any(|note| note.starts_with("rigor:")) &&
        notes.iter().any(|note| note.contains("probabilistic layer did not run")) &&
        notes.iter().all(|note| !note.contains("secret-scope-canary"))));
    assert_eq!(observation.rigor.unwrap().violates_conventions, 0.95);
    assert!(observation.drift.is_none());
    assert!(observation.unavailable.unwrap().contains("secret denylist"));
    assert_eq!(
        fake.call_count(),
        1,
        "residual drift state must not reach the client"
    );
    assert!(
        !fake.calls()[0]
            .state
            .to_string()
            .contains("secret-scope-canary")
    );
}

#[tokio::test]
async fn no_proposed_paths_is_an_input_error_even_when_the_jev_feature_is_off() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    let change = ProposedChange {
        paths: vec![],
        content: "fn proposed() {}".to_string(),
        task: None,
    };

    let error = evaluate_guard(
        &change,
        &protected(),
        &JevConfig::default(),
        false,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect_err("a proposal with no target cannot be evaluated");

    assert!(
        matches!(error, super::super::super::JevError::InvalidGuardPath { path, .. } if path == "paths")
    );
    assert_eq!(fake.call_count(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn permission_denied_optional_context_is_advisory_and_never_transmitted() {
    use std::os::unix::fs::PermissionsExt;

    // Root can bypass ordinary mode bits, so this permission boundary needs a non-root runner.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let repo = TempDir::new().unwrap();
    write_file(&repo, "CONVENTIONS.md", "UNREADABLE_RULE_CANARY");
    std::fs::set_permissions(
        repo.path().join("CONVENTIONS.md"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));
    let change = ProposedChange {
        paths: vec!["src/new.rs".to_string()],
        content: "fn proposed() {}".to_string(),
        task: Some("Add a source file".to_string()),
    };

    let (decision, observation) = evaluate_guard_observed(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect("an optional context permission error is advisory");

    assert!(matches!(decision, GuardDecision::Allow { notes } if
        notes.iter().any(|note| note.contains("CONVENTIONS.md unavailable: unsafe or unreadable"))));
    assert!(
        observation
            .unavailable_context
            .iter()
            .any(|note| note.contains("CONVENTIONS.md unavailable: unsafe or unreadable"))
    );
    let calls = fake.calls();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|call| !call.state.to_string().contains("UNREADABLE_RULE_CANARY"))
    );
}

#[tokio::test]
async fn inaccessible_repository_root_rejects_a_change_before_any_provider_call() {
    let missing_root = TempDir::new().unwrap();
    let root = missing_root.path().join("not-a-repository");
    let fake = FakeClient::new();
    let result = evaluate_guard(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        &root,
    )
    .await;

    assert!(matches!(
        result,
        Err(super::super::super::JevError::InvalidGuardPath { path, .. })
            if path == "repository root"
    ));
    assert_eq!(fake.call_count(), 0);
}

#[tokio::test]
async fn oversized_manifest_is_advisory_and_keeps_available_project_rules() {
    let repo = TempDir::new().unwrap();
    write_file(&repo, "CONVENTIONS.md", "Use explicit errors.");
    write_file(&repo, "Cargo.toml", &"OVERSIZED_DEP_CANARY ".repeat(2_000));
    let fake = FakeClient::new();
    fake.push_response(rigor_response(0.0, 0.0, 0.0, 0.0));
    fake.push_response(drift_response(0.1));

    let (decision, observation) = evaluate_guard_observed(
        &clean_change(),
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .expect("oversized project context is advisory");

    assert!(matches!(decision, GuardDecision::Allow { notes } if
        notes.iter().any(|note| note.contains("Cargo.toml unavailable: exceeds")) &&
        notes.iter().any(|note| note == "dependency manifest unavailable")));
    assert!(observation.unavailable.is_none());
    let calls = fake.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].state["project_context"],
        serde_json::json!([{"path": "CONVENTIONS.md", "content": "Use explicit errors."}])
    );
    assert!(
        calls
            .iter()
            .all(|call| !call.state.to_string().contains("OVERSIZED_DEP_CANARY"))
    );
}

#[tokio::test]
async fn secret_marked_proposal_blocks_without_ever_transmitting_its_value() {
    let repo = TempDir::new().unwrap();
    let fake = FakeClient::new();
    let change = ProposedChange {
        paths: vec!["src/new.rs".to_string()],
        content: "const note = \"secret-proposal-canary\";".to_string(),
        task: None,
    };

    let (decision, observation) = evaluate_guard_observed(
        &change,
        &protected(),
        &JevConfig::default(),
        true,
        true,
        Some(&fake),
        repo.path(),
    )
    .await
    .unwrap();

    match decision {
        GuardDecision::Block(packet) => {
            assert_eq!(packet.violated_check, "secret");
            assert!(!format!("{packet:?}").contains("secret-proposal-canary"));
        }
        other => panic!("expected deterministic secret block, got {other:?}"),
    }
    assert!(observation.rigor.is_none());
    assert!(observation.drift.is_none());
    assert_eq!(fake.call_count(), 0);
}
