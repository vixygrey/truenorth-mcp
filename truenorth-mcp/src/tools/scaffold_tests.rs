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
    TrueNorthServer::new(repo.path().to_path_buf())
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
    // Root docs wired to .agent/.
    assert!(root.join("AGENTS.md").is_file());
    assert!(root.join("CONVENTIONS.md").is_file());
    assert!(
        fs::read_to_string(root.join("AGENTS.md"))
            .unwrap()
            .contains(".agent/")
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
