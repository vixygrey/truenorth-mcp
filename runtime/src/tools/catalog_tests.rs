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
        .build_skill_graph(rmcp::handler::server::tool::RequestId(
            rmcp::model::NumberOrString::Number(1),
        ))
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
        .build_skill_graph(rmcp::handler::server::tool::RequestId(
            rmcp::model::NumberOrString::Number(1),
        ))
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
    let result = server
        .build_skill_graph(rmcp::handler::server::tool::RequestId(
            rmcp::model::NumberOrString::Number(1),
        ))
        .await
        .expect("build graph");
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

fn result_json(result: CallToolResult) -> serde_json::Value {
    let text = result.content[0].as_text().expect("text result");
    serde_json::from_str(&text.text).expect("JSON result")
}

#[tokio::test]
async fn public_catalog_queries_return_structured_data() {
    let dir = tempdir().expect("temp dir");
    make_skill(
        dir.path(),
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: TDD workflow.\n---\n\n# TDD\n",
    );
    let server = server_at(dir.path());

    let index = result_json(server.index_skills().await.expect("index"));
    assert_eq!(index["count"], 1);
    assert_eq!(index["skills"][0]["name"], "develop-tdd");

    let read = result_json(
        server
            .read_skill(Parameters(NameArg {
                name: "develop-tdd".to_string(),
            }))
            .await
            .expect("read"),
    );
    assert_eq!(read["name"], "develop-tdd");
    assert_eq!(read["frontmatter"]["description"], "TDD workflow.");

    let search = result_json(
        server
            .search_skills(Parameters(SearchSkillsArgs {
                query: "workflow".to_string(),
                exact: false,
            }))
            .await
            .expect("search"),
    );
    assert_eq!(search["count"], 1);
    assert_eq!(search["results"][0]["name"], "develop-tdd");

    let validation = result_json(
        server
            .validate_skill(Parameters(NameArg {
                name: "develop-tdd".to_string(),
            }))
            .await
            .expect("validate"),
    );
    assert!(validation["pass"].is_boolean());
}

#[tokio::test]
async fn search_rejects_whitespace_only_query() {
    let dir = tempdir().expect("temp dir");
    let error = server_at(dir.path())
        .search_skills(Parameters(SearchSkillsArgs {
            query: " \n\t".to_string(),
            exact: false,
        }))
        .await
        .expect_err("blank query");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[tokio::test]
async fn persisted_graph_queries_expose_entities_relations_and_dependencies() {
    let dir = tempdir().expect("temp dir");
    make_skill(
        dir.path(),
        "develop-tdd",
        "---\nname: develop-tdd\ndescription: Write tests.\n---\n\n# TDD\n",
    );
    make_skill(
        dir.path(),
        "ship-change",
        "---\nname: ship-change\ndescription: Ship safely.\n---\n\n# Ship\n\nAfter develop-tdd.\n",
    );
    let server = server_at(dir.path());
    server
        .build_skill_graph(rmcp::handler::server::tool::RequestId(
            rmcp::model::NumberOrString::Number(7),
        ))
        .await
        .expect("build graph");

    let graph = result_json(server.read_graph().await.expect("read graph"));
    assert_eq!(graph["entities"].as_array().expect("entities").len(), 2);
    assert_eq!(graph["relations"].as_array().expect("relations").len(), 1);

    let found = result_json(
        server
            .search_nodes(Parameters(QueryArg {
                query: "write tests".to_string(),
            }))
            .await
            .expect("search nodes"),
    );
    assert_eq!(found["results"][0]["name"], "develop-tdd");

    let opened = result_json(
        server
            .open_nodes(Parameters(NamesArg {
                names: vec!["ship-change".to_string(), "missing".to_string()],
            }))
            .await
            .expect("open nodes"),
    );
    assert_eq!(opened["entities"].as_array().expect("opened").len(), 1);
    assert_eq!(opened["entities"][0]["name"], "ship-change");

    let deps = result_json(
        server
            .get_dependencies(Parameters(NameArg {
                name: "ship-change".to_string(),
            }))
            .await
            .expect("dependencies"),
    );
    assert_eq!(deps["skill"], "ship-change");
    assert_eq!(deps["depends_on"], serde_json::json!(["develop-tdd"]));
    assert_eq!(deps["depended_by"], serde_json::json!([]));
}

#[tokio::test]
async fn git_context_rejects_an_unknown_action_before_invoking_git() {
    let dir = tempdir().expect("temp dir");
    let error = server_at(dir.path())
        .get_git_context(Parameters(GitContextArgs {
            action: Some("publish".to_string()),
        }))
        .await
        .expect_err("unknown action");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}

#[tokio::test]
async fn git_context_status_is_scoped_and_structured() {
    if !crate::isolated_process_tests::in_child(
        "tools::catalog::tests::git_context_status_is_scoped_and_structured",
        &[],
    ) {
        return;
    }
    let dir = tempdir().expect("temp dir");
    let init = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("run git init");
    assert!(init.success());
    make_skill(dir.path(), "develop-tdd", "# TDD\n");
    std::fs::write(dir.path().join("outside.txt"), "ignored\n").expect("outside file");

    let value = result_json(
        server_at(dir.path())
            .get_git_context(Parameters(GitContextArgs { action: None }))
            .await
            .expect("git status"),
    );
    assert_eq!(value["action"], "status");
    let output = value["output"].as_str().expect("status text");
    assert!(output.contains("skills/"));
    assert!(!output.contains("outside.txt"));
}

#[test]
fn catalog_error_and_scalar_helpers_preserve_semantics() {
    let read = skill_error(SkillError::Read {
        name: "develop-tdd".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    });
    assert_eq!(read.code, rmcp::model::ErrorCode::INTERNAL_ERROR);

    let mut frontmatter = std::collections::BTreeMap::new();
    frontmatter.insert(
        "string".to_string(),
        serde_yaml::Value::String("x".to_string()),
    );
    frontmatter.insert(
        "number".to_string(),
        serde_yaml::Value::Number(serde_yaml::Number::from(4)),
    );
    frontmatter.insert("bool".to_string(), serde_yaml::Value::Bool(true));
    frontmatter.insert("list".to_string(), serde_yaml::Value::Sequence(Vec::new()));
    assert_eq!(fm_str(&frontmatter, "string"), "x");
    assert_eq!(fm_str(&frontmatter, "number"), "4");
    assert_eq!(fm_str(&frontmatter, "bool"), "true");
    assert_eq!(fm_str(&frontmatter, "list"), "");
    assert_eq!(fm_str(&frontmatter, "missing"), "");

    assert_eq!(action_name(GitAction::Status), "status");
    assert_eq!(action_name(GitAction::Log), "log");
    assert_eq!(action_name(GitAction::Diff), "diff");
    assert_eq!(
        receipt_error("bad digest".to_string()).code,
        rmcp::model::ErrorCode::INTERNAL_ERROR
    );
    assert_eq!(
        git_error(GitError::Command {
            action: "status".to_string(),
            stderr: "not a repository".to_string(),
        })
        .code,
        rmcp::model::ErrorCode::INTERNAL_ERROR
    );
}
