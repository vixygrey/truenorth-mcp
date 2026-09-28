//! Tests for the legacy catalog tools (task 8b).
//!
//! Included from `catalog.rs` via `#[path]`, so `super` is the catalog module.
//!
//! Requirements: 7.3, 7.4, 7.5, 7.7, 7.9.

use super::*;
use std::sync::Arc;
use tempfile::tempdir;

/// Build a `TrueNorthServer` rooted at a temp dir.
fn server_at(root: &std::path::Path) -> TrueNorthServer {
    TrueNorthServer {
        ctx: Arc::new(crate::server::ServerContext::with_features(
            root.to_path_buf(),
            crate::engine::features::Features::default(),
        )),
        tool_router: rmcp::handler::server::router::tool::ToolRouter::new(),
    }
}

#[tokio::test]
async fn catalog_json_respects_the_same_payload_cap() {
    let repo = tempdir().expect("temporary repository");
    make_skill(repo.path(), "develop-tdd", "# TDD\n");
    let server = TrueNorthServer::from_context(crate::server::ServerContext::with_config(
        repo.path().to_path_buf(),
        crate::engine::features::Features::default(),
        crate::engine::features::TokenCaps {
            skill_lean_tokens: 100,
            tool_payload_tokens: 1,
        },
    ));
    let error = server
        .index_skills()
        .await
        .expect_err("catalog JSON exceeds one token");
    assert!(error.message.contains("token cap"));
}

#[tokio::test]
async fn oversized_graph_receipt_rejects_before_writing_cache() {
    let repo = tempdir().expect("temporary repository");
    make_skill(repo.path(), "develop-tdd", "# TDD\n");
    let server = TrueNorthServer::from_context(crate::server::ServerContext::with_config(
        repo.path().to_path_buf(),
        crate::engine::features::Features::default(),
        crate::engine::features::TokenCaps {
            skill_lean_tokens: 100,
            tool_payload_tokens: 1,
        },
    ));
    let error = server
        .build_skill_graph()
        .await
        .expect_err("receipt exceeds cap");
    assert!(error.message.contains("token cap"));
    assert!(!server.ctx.graph_path().exists());
}

#[tokio::test]
async fn unresolved_canonical_skill_target_rejects_before_writing_cache() {
    let repo = tempdir().expect("temporary repository");
    make_skill(
        repo.path(),
        "orchestrate-project",
        "# Orchestrate\n\nSee skills/removed-skill/SKILL.md.\n",
    );
    let server = server_at(repo.path());
    let error = server
        .build_skill_graph()
        .await
        .expect_err("unresolved target");
    assert!(error.message.contains("removed-skill"), "{}", error.message);
    assert!(!server.ctx.graph_path().exists());
}

#[tokio::test]
async fn graph_receipt_reports_catalog_diagnostics() {
    let repo = tempdir().expect("temporary repository");
    make_skill(
        repo.path(),
        "orchestrate-project",
        "---\ndescription: Orchestrate a project workflow.\n---\n\n# Orchestrate\n",
    );
    let server = server_at(repo.path());
    let result = server.build_skill_graph().await.expect("build graph");
    let text = result.content[0]
        .as_text()
        .expect("text result")
        .text
        .clone();
    let value: serde_json::Value = serde_json::from_str(&text).expect("JSON receipt");
    assert_eq!(
        value["diagnostics"]["suspicious_isolated_orchestrators"][0],
        "orchestrate-project"
    );
    assert_eq!(
        value["diagnostics"]["unresolved_mentions"],
        serde_json::json!([])
    );
}

/// Write a minimal skill for discovery.
fn make_skill(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join("skills").join(name);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    std::fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
}

#[test]
fn parse_named_skill_returns_parsed_structure() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    make_skill(
        root,
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: TDD loop.\n---\n\n# TDD\n\nBody.\n",
    );
    let server = server_at(root);
    let parsed = server.parse_named_skill("develop-tdd").expect("parse");
    assert_eq!(parsed.name, "develop-tdd");
    assert_eq!(parsed.headings.len(), 1);
}

#[test]
fn parse_named_skill_errors_on_missing() {
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let error = server
        .parse_named_skill("absent")
        .expect_err("missing skill");
    assert!(error.message.contains("not found"), "{}", error.message);
    assert!(error.message.contains("absent"), "{}", error.message);
}

#[test]
fn parse_git_action_defaults_to_status() {
    assert_eq!(parse_git_action(None).unwrap(), GitAction::Status);
}

#[test]
fn parse_git_action_rejects_unsupported() {
    let error = parse_git_action(Some("deploy")).expect_err("unsupported action");
    assert!(error.message.contains("unsupported git action"));
}

#[test]
fn load_persisted_graph_errors_when_absent() {
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let error = server.load_persisted_graph().expect_err("no graph");
    assert!(error.message.contains("unavailable"));
}

#[test]
fn run_search_finds_matching_skill() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    make_skill(
        root,
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: TDD loop.\n---\n\n# TDD\n",
    );
    make_skill(
        root,
        "plan-work",
        "---\nname: plan-work\ndescription: Plan work.\n---\n\n# Plan\n",
    );
    let server = server_at(root);
    let results = server.run_search(&SearchSkillsArgs {
        query: "TDD".to_string(),
        exact: false,
    });
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "develop-tdd");
}

#[test]
fn run_search_exact_matches_name_only() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    make_skill(root, "develop-tdd", "---\nname: develop-tdd\n---\n\n# X\n");
    let server = server_at(root);
    let results = server.run_search(&SearchSkillsArgs {
        query: "develop-tdd".to_string(),
        exact: true,
    });
    assert_eq!(results.len(), 1);

    // An exact search for a non-matching name returns nothing.
    let empty = server.run_search(&SearchSkillsArgs {
        query: "plan-work".to_string(),
        exact: true,
    });
    assert!(empty.is_empty());
}
