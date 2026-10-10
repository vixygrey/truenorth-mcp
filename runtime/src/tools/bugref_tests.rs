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

    srv.truenorth_record_bug(
        Parameters(valid_args()),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
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

    srv.truenorth_record_bug(
        Parameters(valid_args()),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("first");
    let mut second = valid_args();
    second.id = "BUG-2".to_string();
    srv.truenorth_record_bug(
        Parameters(second),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("second");

    let text = fs::read_to_string(bugs_path(&repo)).expect("read bugs");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse");
    let bugs = doc.get("bugs").and_then(|v| v.as_sequence()).expect("bugs");
    assert_eq!(bugs.len(), 2);
}

#[tokio::test]
async fn record_bug_updates_an_existing_reference_without_duplication() {
    let repo = repo_with_task();
    let srv = server(&repo);

    srv.truenorth_record_bug(
        Parameters(valid_args()),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("insert");
    let mut document: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(bugs_path(&repo)).expect("read bugs"))
            .expect("parse bugs");
    document["workspace_note"] = "preserve top level".into();
    document["bugs"][0]["custom_field"] = "preserve entry".into();
    fs::write(
        bugs_path(&repo),
        serde_yaml::to_string(&document).expect("serialize fixture"),
    )
    .expect("write fixture");

    let mut update = valid_args();
    update.status = BugStatus::Resolved;
    update.linked_ref = "Wire the gate".to_string();
    update.tags = vec!["fixed".to_string()];
    srv.truenorth_record_bug(
        Parameters(update),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("update");

    let text = fs::read_to_string(bugs_path(&repo)).expect("read updated bugs");
    let document: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse updated bugs");
    let bugs = document
        .get("bugs")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("bugs");
    assert_eq!(bugs.len(), 1);
    assert_eq!(
        bugs[0].get("status").and_then(serde_yaml::Value::as_str),
        Some("resolved")
    );
    assert_eq!(
        bugs[0]
            .get("linked_ref")
            .and_then(serde_yaml::Value::as_str),
        Some("Wire the gate")
    );
    assert_eq!(
        bugs[0]
            .get("custom_field")
            .and_then(serde_yaml::Value::as_str),
        Some("preserve entry")
    );
    assert_eq!(
        document
            .get("workspace_note")
            .and_then(serde_yaml::Value::as_str),
        Some("preserve top level")
    );
}

#[tokio::test]
async fn record_bug_rejects_an_existing_id_with_a_different_external_link() {
    let repo = repo_with_task();
    let srv = server(&repo);

    srv.truenorth_record_bug(
        Parameters(valid_args()),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("insert");
    let before = fs::read_to_string(bugs_path(&repo)).expect("read before");
    let mut conflicting = valid_args();
    conflicting.external_link = "https://tracker.example/issues/different".to_string();

    let error = srv
        .truenorth_record_bug(
            Parameters(conflicting),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect_err("conflicting external link");

    assert!(error.message.contains("external_link"));
    assert_eq!(
        fs::read_to_string(bugs_path(&repo)).expect("read after"),
        before
    );
}

#[tokio::test]
async fn record_bug_rejects_an_empty_id_and_writes_nothing() {
    let repo = repo_with_task();
    let srv = server(&repo);
    let mut args = valid_args();
    args.id = String::new();

    let error = srv
        .truenorth_record_bug(
            Parameters(args),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
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
        .truenorth_record_bug(
            Parameters(args),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
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
        .truenorth_record_bug(
            Parameters(args),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
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

    srv.truenorth_record_bug(
        Parameters(args),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("task_name ref resolves");
    assert!(bugs_path(&repo).is_file());
}

#[tokio::test]
async fn record_bug_resolves_a_ref_to_an_existing_bug_id() {
    let repo = repo_with_task();
    let srv = server(&repo);

    // First bug links to the task.
    srv.truenorth_record_bug(
        Parameters(valid_args()),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("first bug");

    // Second bug links to the first bug's id, which now resolves.
    let mut second = valid_args();
    second.id = "BUG-2".to_string();
    second.linked_ref = "BUG-1".to_string();
    srv.truenorth_record_bug(
        Parameters(second),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
    .await
    .expect("bug-id ref resolves");
}

#[tokio::test]
async fn record_bug_rejects_malformed_or_non_mapping_storage_without_rewriting_it() {
    for existing in ["bugs: [\n", "- id: BUG-1\n"] {
        let repo = repo_with_task();
        let path = bugs_path(&repo);
        fs::write(&path, existing).expect("seed invalid bugs document");
        let before = fs::read(&path).expect("read fixture");

        let error = server(&repo)
            .truenorth_record_bug(
                Parameters(valid_args()),
                rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(2)),
            )
            .await
            .expect_err("invalid stored document");

        assert!(
            error.message.contains("failed to parse")
                || error.message.contains("must contain a YAML mapping")
        );
        assert_eq!(fs::read(&path).expect("read unchanged fixture"), before);
    }
}

#[tokio::test]
async fn record_bug_rejects_boundary_violations_before_creating_storage() {
    let repo = repo_with_task();
    let srv = server(&repo);

    let mut overlong = valid_args();
    overlong.id = "é".repeat(MAX_BUG_ID + 1);
    let error = srv
        .truenorth_record_bug(
            Parameters(overlong),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(3)),
        )
        .await
        .expect_err("overlong id");
    assert!(error.message.contains("201"));

    let mut invalid_scheme = valid_args();
    invalid_scheme.external_link = "12://tracker.example/1".to_string();
    let error = srv
        .truenorth_record_bug(
            Parameters(invalid_scheme),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(4)),
        )
        .await
        .expect_err("non-alphabetic URL scheme");
    assert!(error.message.contains("external_link"));
    assert!(!bugs_path(&repo).exists());
}

#[tokio::test]
async fn unresolved_ref_from_missing_or_malformed_release_plan_preserves_storage() {
    for plan_body in [None, Some("tasks: [\n"), Some("version: 1\n")] {
        let repo = TempDir::new().expect("temp repo");
        if let Some(body) = plan_body {
            let plan = release_plan_path(repo.path());
            fs::create_dir_all(plan.parent().expect("plan parent")).expect("tasks dir");
            fs::write(plan, body).expect("seed release plan");
        }

        let error = server(&repo)
            .truenorth_record_bug(
                Parameters(valid_args()),
                rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(5)),
            )
            .await
            .expect_err("unresolved ref");
        assert!(error.message.contains("linked_ref"));
        assert!(!bugs_path(&repo).exists());
    }
}

#[test]
fn read_bugs_treats_invalid_or_non_mapping_content_as_empty() {
    for body in ["bugs: [\n", "- id: BUG-1\n"] {
        let repo = TempDir::new().expect("temp repo");
        let path = bugs_path(&repo);
        fs::create_dir_all(path.parent().expect("bugs parent")).expect("tasks dir");
        fs::write(path, body).expect("seed invalid bugs");

        assert_eq!(server(&repo).read_bugs(), empty_bugs_doc());
    }
}

#[test]
fn bug_receipt_error_mapping_is_internal() {
    let error = receipt_error("invalid digest".to_string());
    assert_eq!(error.code.0, -32603);
    assert!(error.message.contains("mutation receipt"));
}

#[tokio::test]
async fn record_bug_normalizes_a_non_sequence_bugs_field() {
    let repo = repo_with_task();
    fs::write(bugs_path(&repo), "bugs: legacy-scalar\nnote: keep\n").expect("seed mapping");

    server(&repo)
        .truenorth_record_bug(
            Parameters(valid_args()),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(6)),
        )
        .await
        .expect("normalize bugs field");

    let text = fs::read_to_string(bugs_path(&repo)).expect("updated bugs");
    let value: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse bugs");
    assert_eq!(value["note"], "keep");
    let bugs = value["bugs"].as_sequence().expect("normalized sequence");
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0]["id"], "BUG-1");
}
