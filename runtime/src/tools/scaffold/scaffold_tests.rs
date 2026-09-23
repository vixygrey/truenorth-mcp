//! Unit tests for the greenfield scaffold core emission.
//!
//! Included from `scaffold.rs` via `#[path]`, so `super` is the scaffold module. The tool
//! method takes only `Parameters`, so a test drives it directly against a temp repo.
//!
//! Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.12.

use std::fs;
use std::path::Path;

use rmcp::handler::server::wrapper::Parameters;
use tempfile::TempDir;

use super::*;

fn server(repo: &TempDir) -> TrueNorthServer {
    TrueNorthServer::test_server(repo.path().to_path_buf())
}

async fn scaffold(srv: &TrueNorthServer, profile: Option<&str>) -> Result<(), ErrorData> {
    srv.truenorth_scaffold_project(Parameters(ScaffoldArgs {
        profile: profile.map(str::to_string),
    }))
    .await
    .map(|_| ())
}

#[test]
fn resolve_scaffold_profile_defaults_and_rejects() {
    // Absent uses issue-per-task (Requirement 5.1).
    assert_eq!(
        resolve_scaffold_profile(None).expect("default").name,
        "issue-per-task"
    );
    // A known name resolves.
    assert_eq!(
        resolve_scaffold_profile(Some("kanban"))
            .expect("known")
            .name,
        "kanban"
    );
    // An unknown name errors (Requirement 5.2).
    assert!(resolve_scaffold_profile(Some("waterfall")).is_err());
}

#[tokio::test]
async fn scaffold_emits_the_agent_tree_and_root_docs() {
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("scaffold");

    let root = repo.path();
    // The .agent/ contract, profile, and required area files exist.
    assert!(root.join(".agent/layout.yml").is_file());
    assert!(root.join(".agent/profile.yml").is_file());
    assert!(root.join(".agent/tasks/state.yml").is_file());
    assert!(root.join(".agent/memories/glossary.md").is_file());
    assert!(root.join(".agent/telemetry/runs.yml").is_file());
    // The default profile's starter file (issue-per-task uses backlog).
    assert!(root.join(".agent/tasks/backlog.yml").is_file());
    assert_eq!(
        fs::read_to_string(root.join(".agent/tasks/execution-status.yml")).expect("read status"),
        "stories: {}\ndevelopment_status: {}\n"
    );
    // Root docs wired to .agent/.
    assert!(root.join("AGENTS.md").is_file());
    assert!(root.join("CONVENTIONS.md").is_file());
    assert!(
        fs::read_to_string(root.join("AGENTS.md"))
            .unwrap()
            .contains(".agent/")
    );
}

#[test]
fn checked_in_issue_per_task_cockpit_matches_scaffold_seeds() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root");
    let tasks = repo_root.join(".agent/tasks");

    assert_eq!(
        fs::read_to_string(repo_root.join(".agent/profile.yml")).expect("read profile"),
        "profile: issue-per-task\n"
    );
    // The backlog is mixed human/runtime state, so its checked-in content is not a scaffold
    // fixture. The scaffold test above still proves that it creates the empty starter file.
    assert_eq!(
        fs::read_to_string(tasks.join("execution-status.yml")).expect("read execution status"),
        "stories: {}\ndevelopment_status: {}\n"
    );
}

#[tokio::test]
async fn scaffold_writes_the_declared_profile_name() {
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, Some("epic-based")).await.expect("scaffold");

    let profile = fs::read_to_string(repo.path().join(".agent/profile.yml")).expect("read");
    assert!(profile.contains("profile: epic-based"));
    // The epic-based starter set includes the release plan.
    assert_eq!(
        fs::read_to_string(repo.path().join(".agent/tasks/execution-status.yml"))
            .expect("read status"),
        "stories: {}\ndevelopment_status: {}\n"
    );
    assert!(repo.path().join(".agent/tasks/release-plan.yml").is_file());
}

#[tokio::test]
async fn scaffold_is_language_agnostic() {
    // Requirement 5.4: no Cargo.toml, no package.json, no source tree.
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("scaffold");

    assert!(!repo.path().join("Cargo.toml").exists());
    assert!(!repo.path().join("package.json").exists());
    assert!(!repo.path().join("src").exists());
}

#[tokio::test]
async fn scaffold_is_non_destructive_on_a_second_run() {
    // Requirement 5.12: an existing path is left unchanged and reported as skipped.
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("first");

    // Tamper with one seeded file, then re-run.
    let state = repo.path().join(".agent/tasks/state.yml");
    fs::write(&state, "phase: integrate\n").expect("tamper");
    scaffold(&srv, None).await.expect("second");

    // The tampered content survives, since the second run skipped the existing file.
    assert_eq!(
        fs::read_to_string(&state).expect("read state"),
        "phase: integrate\n"
    );
}

#[tokio::test]
async fn scaffold_emits_hooks_and_github_templates() {
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("scaffold");

    let root = repo.path();
    assert!(root.join(".githooks/commit-msg").is_file());
    assert!(root.join(".githooks/post-merge").is_file());
    assert!(root.join(".github/commit-template.md").is_file());
    assert!(root.join(".github/pull-request-template.md").is_file());
    assert!(root.join(".github/ISSUE_TEMPLATE/bug.md").is_file());
    assert!(root.join(".github/ISSUE_TEMPLATE/feature.md").is_file());
    assert!(root.join(".github/ISSUE_TEMPLATE/config.yml").is_file());

    // The emitted commit-msg hook is the profile-templated one.
    let hook = fs::read_to_string(root.join(".githooks/commit-msg")).expect("read hook");
    assert!(hook.starts_with("#!/bin/sh\n"));
}

#[tokio::test]
async fn scaffold_prints_the_hooks_path_command_without_running_it() {
    // Requirement 5.7: the command is reported, not run. So core.hooksPath is not set.
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    let result = srv
        .truenorth_scaffold_project(Parameters(ScaffoldArgs { profile: None }))
        .await
        .expect("scaffold");

    // The result carries the command text.
    let text = result.content[0]
        .as_text()
        .expect("text content")
        .text
        .clone();
    assert!(text.contains("git config core.hooksPath .githooks"));
}

#[test]
fn issue_forms_include_grouping_only_for_epic_and_milestone() {
    // Requirement 5.11: a grouping field appears for epic and milestone vocabularies.
    assert!(bug_form(profile::EPIC_BASED).contains("## epic id"));
    assert!(bug_form(profile::MILESTONE_BASED).contains("## milestone id"));
    // Ticket and none vocabularies carry no grouping field.
    assert!(!bug_form(profile::ISSUE_PER_TASK).contains("id\n\n## "));
    assert!(!bug_form(profile::KANBAN).contains("## epic id"));
    assert!(!bug_form(profile::GENERIC).contains("## milestone id"));
}

#[test]
fn issue_forms_include_the_id_field_only_when_the_profile_requires_it() {
    // Requirement 5.11: the issue-id field is omitted for kanban and generic.
    assert!(bug_form(profile::ISSUE_PER_TASK).contains("## Issue or ticket id"));
    assert!(bug_form(profile::EPIC_BASED).contains("## Issue or ticket id"));
    assert!(!bug_form(profile::KANBAN).contains("## Issue or ticket id"));
    assert!(!bug_form(profile::GENERIC).contains("## Issue or ticket id"));
}

#[tokio::test]
async fn scaffold_github_templates_carry_no_hardcoded_project_urls() {
    // Requirement 5.10: generic structure only, no copied domain content. The only URL is
    // the placeholder contact link.
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("scaffold");

    let config = fs::read_to_string(repo.path().join(".github/ISSUE_TEMPLATE/config.yml"))
        .expect("read config");
    assert!(
        config.contains("example.invalid"),
        "uses a placeholder host"
    );
}

#[tokio::test]
async fn scaffold_unknown_profile_makes_no_file_change() {
    // Requirement 5.2: an unknown profile makes no file change.
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);

    let error = scaffold(&srv, Some("waterfall"))
        .await
        .expect_err("unknown");
    assert!(error.message.contains("waterfall"));
    // Nothing was written.
    assert!(!repo.path().join(".agent").exists());
    assert!(!repo.path().join("AGENTS.md").exists());
}

#[tokio::test]
async fn scaffold_emits_the_jev_guard_pretooluse_hook() {
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("scaffold");

    let hook_path = repo.path().join(".kiro/hooks/jev-guard.json");
    assert!(hook_path.is_file(), "the guard hook file is emitted");

    let body = fs::read_to_string(&hook_path).expect("read the hook file");
    // The hook is valid JSON and parses.
    let json: serde_json::Value = serde_json::from_str(&body).expect("the hook is valid JSON");

    // It is a PreToolUse hook matching the common write tools and running the guard CLI.
    let hook = &json["hooks"][0];
    assert_eq!(hook["trigger"], "PreToolUse");
    assert_eq!(hook["matcher"], "write_to_file|fs_write|str_replace");
    assert_eq!(hook["action"]["type"], "command");
    assert_eq!(hook["action"]["command"], "truenorth-mcp guard");
}

#[tokio::test]
async fn the_emitted_hook_documents_the_client_honor_boundary_and_the_fallback() {
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    scaffold(&srv, None).await.expect("scaffold");

    let body =
        fs::read_to_string(repo.path().join(".kiro/hooks/jev-guard.json")).expect("read the hook");
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    let description = json["hooks"][0]["description"]
        .as_str()
        .expect("the hook carries a description");

    // The documentation states the client-honor boundary and the tool fallback (R7.4, R9.4).
    assert!(
        description.contains("honoring the PreToolUse hook"),
        "states the client-honor boundary: {description}"
    );
    assert!(
        description.contains("truenorth_guard_change"),
        "names the tool fallback: {description}"
    );
    assert!(
        description.contains("cannot force-intercept"),
        "states the honest boundary: {description}"
    );
}
