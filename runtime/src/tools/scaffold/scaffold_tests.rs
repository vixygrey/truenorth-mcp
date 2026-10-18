//! Unit tests for the greenfield scaffold core emission.
//!
//! Included from `scaffold.rs` via `#[path]`, so `super` is the scaffold module. The tool
//! method takes only `Parameters`, so a test drives it directly against a temp repo.
//!
//! Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.12.

use std::fs;

use rmcp::handler::server::wrapper::Parameters;
use tempfile::TempDir;

use super::*;

fn server(repo: &TempDir) -> TrueNorthServer {
    TrueNorthServer::test_server(repo.path().to_path_buf())
}

async fn scaffold(srv: &TrueNorthServer, profile: Option<&str>) -> Result<(), ErrorData> {
    srv.truenorth_scaffold_project(
        Parameters(ScaffoldArgs {
            profile: profile.map(str::to_string),
        }),
        rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
    )
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
    assert!(root.join(".agent/tasks/state.json").is_file());
    assert!(root.join(".agent/memories/glossary.md").is_file());
    assert!(!root.join(".agent/telemetry/runs.yml").exists());
    // Backlog ownership is explicit and independent of the methodology profile.
    assert_eq!(
        fs::read_to_string(root.join(".agent/tasks/backlog.yml")).expect("read backlog"),
        "version: '1'\nownership:\n  mode: local\nbacklog: []\n"
    );
    assert_eq!(
        fs::read_to_string(root.join(".agent/tasks/execution-status.yml")).expect("read status"),
        "tasks: {}\ngroups: {}\ndevelopment_status: {}\n"
    );
    // Root docs carry the workspace and operating-model contracts.
    assert!(root.join("AGENTS.md").is_file());
    assert!(root.join("CONVENTIONS.md").is_file());
    let agents = fs::read_to_string(root.join("AGENTS.md")).expect("read agent guidance");
    assert!(agents.contains(".agent/"));
    assert!(agents.contains("local stdio tool for individuals and small teams"));
    assert!(agents.contains("one mutating server supported per worktree"));
    assert!(agents.contains("Parallel mutating agents use separate Git worktrees"));
    assert!(agents.contains("Git is the durable collaboration"));
    assert!(agents.contains("Shared network-filesystem mutation"));
    assert!(agents.contains("multi-user hosting"));
    assert!(agents.contains("centralized policy enforcement"));
    assert!(agents.contains("https://github.com/vixygrey/truenorth-mcp#operating-model"));
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
        "tasks: {}\ngroups: {}\ndevelopment_status: {}\n"
    );
    assert!(repo.path().join(".agent/tasks/release-plan.json").is_file());
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
    let state = repo.path().join(".agent/tasks/state.json");
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
async fn oversized_scaffold_receipt_writes_nothing() {
    let repo = TempDir::new().expect("temp repo");
    let srv = TrueNorthServer::from_context(crate::server::ServerContext::with_config(
        repo.path().to_path_buf(),
        crate::engine::features::Features::default(),
        crate::engine::features::TokenCaps {
            skill_lean_tokens: 100,
            tool_payload_tokens: 1,
        },
    ));
    let error = srv
        .truenorth_scaffold_project(
            Parameters(ScaffoldArgs { profile: None }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect_err("receipt exceeds cap");
    assert!(error.message.contains("token cap"));
    assert!(!repo.path().join(".agent").exists());
    assert!(!repo.path().join("AGENTS.md").exists());
}

#[tokio::test]
async fn scaffold_prints_the_hooks_path_command_without_running_it() {
    // Requirement 5.7: the command is reported, not run. So core.hooksPath is not set.
    let repo = TempDir::new().expect("temp repo");
    let srv = server(&repo);
    let result = srv
        .truenorth_scaffold_project(
            Parameters(ScaffoldArgs { profile: None }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
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

#[test]
fn bug_form_does_not_request_its_own_external_link() {
    let form = bug_form(profile::ISSUE_PER_TASK);

    assert!(!form.contains("External tracker link"));
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

#[test]
fn profile_name_boundaries_reject_empty_and_overlong_values() {
    for invalid in ["".to_string(), "p".repeat(MAX_PROFILE_NAME + 1)] {
        let error = resolve_scaffold_profile(Some(&invalid)).expect_err("invalid profile length");
        assert!(error.message.contains("`profile`"));
        assert!(error.message.contains("1 to 64"));
    }
}

#[test]
fn scaffold_plan_reports_and_preserves_mixed_existing_targets() {
    let repo = TempDir::new().expect("temp repo");
    let state = repo.path().join(".agent/tasks/state.json");
    fs::create_dir_all(state.parent().expect("state parent")).expect("tasks dir");
    fs::write(&state, "human state\n").expect("seed state");
    fs::write(repo.path().join("AGENTS.md"), "human agents\n").expect("seed agents");

    let emissions =
        scaffold_project(repo.path(), profile::ISSUE_PER_TASK).expect("scaffold project");
    assert!(emissions.contains(&Emission::Skipped(".agent/tasks/state.json".to_string())));
    assert!(emissions.contains(&Emission::Skipped("AGENTS.md".to_string())));
    assert!(emissions.contains(&Emission::Wrote(".agent/layout.yml".to_string())));
    assert_eq!(fs::read_to_string(state).expect("state"), "human state\n");
    assert_eq!(
        fs::read_to_string(repo.path().join("AGENTS.md")).expect("agents"),
        "human agents\n"
    );
    assert!(repo.path().join(".agent/layout.yml").is_file());
}

#[test]
fn result_json_separates_written_and_skipped_paths() {
    let value = result_json(
        profile::GENERIC,
        &[
            Emission::Wrote(".agent/layout.yml".to_string()),
            Emission::Skipped("AGENTS.md".to_string()),
        ],
    );
    assert_eq!(value["profile"], "generic");
    assert_eq!(value["wrote"], serde_json::json!([".agent/layout.yml"]));
    assert_eq!(value["skipped"], serde_json::json!(["AGENTS.md"]));
    assert_eq!(value["next_step"], HOOKS_PATH_COMMAND);
}

#[test]
fn template_fallback_and_sprint_vocabulary_are_rendered_deterministically() {
    assert_eq!(starter_seed("tasks/custom.yml"), "{}\n");
    let sprint_profile = profile::Profile {
        name: "sprint-test",
        vocab: profile::GroupingVocab::Sprint,
        rule: profile::GroupingRule::Optional,
        starter_files: &[],
        branch_pattern: "",
        require_issue_id: false,
    };
    assert!(conventions_md(sprint_profile).contains("is `sprint`"));
}
