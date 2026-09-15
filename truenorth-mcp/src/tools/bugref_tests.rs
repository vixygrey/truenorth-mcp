//! Unit tests for the bug-reference tool.
//!
//! Included from `bugref.rs` via `#[path]`, so `super` is the bugref module. The tool
//! method takes only `Parameters`, so a test drives it directly against a temp repo.
//!
//! Requirements: 8.1, 8.3, 8.6, 8.7.

use std::fs;

use rmcp::handler::server::wrapper::Parameters;
use tempfile::TempDir;

use super::*;

/// A repo with a release plan carrying one task, so a linked_ref can resolve.
fn repo_with_task() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let plan = release_plan_path(repo.path());
    fs::create_dir_all(plan.parent().unwrap()).expect("tasks dir");
    fs::write(
        &plan,
        "tasks:\n- group_id: e80\n  task_name: Wire the gate\n  verify_command: cargo test\n",
    )
    .expect("seed plan");
    repo
}

/// Build a valid RecordBugArgs linked to the seeded task.
fn valid_args() -> RecordBugArgs {
    RecordBugArgs {
        id: "BUG-1".to_string(),
        external_link: "https://tracker.example/issues/1".to_string(),
        status: BugStatus::Open,
        linked_ref: "e80".to_string(),
        tags: vec!["regression".to_string(), "p1".to_string()],
    }
}

fn server(repo: &TempDir) -> TrueNorthServer {
    TrueNorthServer::test_server(repo.path().to_path_buf())
}

fn bugs_path(repo: &TempDir) -> std::path::PathBuf {
    repo.path().join(".agent").join("tasks").join("bugs.yml")
}

#[test]
fn is_absolute_url_accepts_and_rejects() {
    assert!(is_absolute_url("https://tracker/1"));
    assert!(is_absolute_url("http://host/path"));
    // A relative path, a bare word, or a missing authority is rejected.
    assert!(!is_absolute_url("tracker/1"));
    assert!(!is_absolute_url("/issues/1"));
    assert!(!is_absolute_url("https://"));
    assert!(!is_absolute_url("123"));
}

#[tokio::test]
async fn record_bug_writes_a_reference_with_tags() {
    let repo = repo_with_task();
    let srv = server(&repo);

    srv.truenorth_record_bug(Parameters(valid_args()))
        .await
        .expect("record bug");

    let text = fs::read_to_string(bugs_path(&repo)).expect("read bugs");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse bugs");
    let bugs = doc
        .get("bugs")
        .and_then(|v| v.as_sequence())
        .expect("bugs seq");
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].get("id").and_then(|v| v.as_str()), Some("BUG-1"));
    assert_eq!(bugs[0].get("status").and_then(|v| v.as_str()), Some("open"));
    // Tags round-trip as given (Requirement 8.3).
    let tags: Vec<&str> = bugs[0]
        .get("tags")
        .and_then(|v| v.as_sequence())
        .expect("tags")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(tags, vec!["regression", "p1"]);
}

#[tokio::test]
async fn record_bug_appends_and_preserves_existing_references() {
    let repo = repo_with_task();
    let srv = server(&repo);

    srv.truenorth_record_bug(Parameters(valid_args()))
        .await
        .expect("first");
    let mut second = valid_args();
    second.id = "BUG-2".to_string();
    srv.truenorth_record_bug(Parameters(second))
        .await
        .expect("second");

    let text = fs::read_to_string(bugs_path(&repo)).expect("read bugs");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse");
    let bugs = doc.get("bugs").and_then(|v| v.as_sequence()).expect("bugs");
    assert_eq!(bugs.len(), 2);
}

#[tokio::test]
async fn record_bug_rejects_an_empty_id_and_writes_nothing() {
    let repo = repo_with_task();
    let srv = server(&repo);
    let mut args = valid_args();
    args.id = String::new();

    let error = srv
        .truenorth_record_bug(Parameters(args))
        .await
        .expect_err("empty id");
    assert!(error.message.contains("`id`"));
    assert!(
        !bugs_path(&repo).exists(),
        "no bugs file was written on reject"
    );
}

#[tokio::test]
async fn record_bug_rejects_a_relative_link() {
    let repo = repo_with_task();
    let srv = server(&repo);
    let mut args = valid_args();
    args.external_link = "tracker/1".to_string();

    let error = srv
        .truenorth_record_bug(Parameters(args))
        .await
        .expect_err("relative link");
    assert!(error.message.contains("external_link"));
    assert!(!bugs_path(&repo).exists());
}

#[tokio::test]
async fn record_bug_rejects_an_unresolved_linked_ref() {
    let repo = repo_with_task();
    let srv = server(&repo);
    let mut args = valid_args();
    args.linked_ref = "does-not-exist".to_string();

    let error = srv
        .truenorth_record_bug(Parameters(args))
        .await
        .expect_err("unresolved ref");
    assert!(error.message.contains("linked_ref"));
    assert!(!bugs_path(&repo).exists());
}

#[tokio::test]
async fn record_bug_resolves_a_ref_to_a_task_name() {
    let repo = repo_with_task();
    let srv = server(&repo);
    let mut args = valid_args();
    args.linked_ref = "Wire the gate".to_string();

    srv.truenorth_record_bug(Parameters(args))
        .await
        .expect("task_name ref resolves");
    assert!(bugs_path(&repo).is_file());
}

#[tokio::test]
async fn record_bug_resolves_a_ref_to_an_existing_bug_id() {
    let repo = repo_with_task();
    let srv = server(&repo);

    // First bug links to the task.
    srv.truenorth_record_bug(Parameters(valid_args()))
        .await
        .expect("first bug");

    // Second bug links to the first bug's id, which now resolves.
    let mut second = valid_args();
    second.id = "BUG-2".to_string();
    second.linked_ref = "BUG-1".to_string();
    srv.truenorth_record_bug(Parameters(second))
        .await
        .expect("bug-id ref resolves");
}
