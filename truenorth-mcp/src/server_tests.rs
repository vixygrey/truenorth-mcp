//! Tests for the server context constructors (issue #140).
//!
//! Included from `server.rs` via `#[path]`, so `super` is the server module.
//!
//! These cover the two `ServerContext` constructors: `with_features` (injected, infallible)
//! and `resolve` (reads `.agent/config/rules.yml`). The disabled and error paths use a
//! seeded `rules.yml` under a temp repo root.

use std::fs;

use tempfile::TempDir;

use super::*;

/// Seed `.agent/config/rules.yml` with the given body under a fresh temp repo root.
fn seed_rules(body: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let config = repo.path().join(".agent").join("config");
    fs::create_dir_all(&config).expect("create .agent/config");
    fs::write(config.join("rules.yml"), body).expect("write rules.yml");
    repo
}

#[test]
fn graph_path_resolves_under_agent_tasks() {
    // The skill-graph cache lives under `.agent/`, not in the crate source tree (#162,
    // ADR-0008). The absolute path and the guard-relative path must agree.
    let repo = TempDir::new().expect("temp repo");
    let ctx = ServerContext::with_features(repo.path().to_path_buf(), Features::default());
    let expected = repo
        .path()
        .join(".agent")
        .join("tasks")
        .join("skill-graph.jsonl");
    assert_eq!(ctx.graph_path(), expected);
    // The guard-relative path is `graph_path` minus the repo `.agent/` prefix.
    assert_eq!(
        ctx.graph_path(),
        repo.path()
            .join(".agent")
            .join(ServerContext::graph_rel_path())
    );
    // The path is not in the crate source tree.
    assert!(
        !ctx.graph_path()
            .starts_with(repo.path().join("truenorth-mcp"))
    );
}

#[test]
fn with_features_default_is_enabled() {
    let repo = TempDir::new().expect("temp repo");
    let ctx = ServerContext::with_features(repo.path().to_path_buf(), Features::default());
    assert!(ctx.features.ontology);
}

#[test]
fn with_features_disabled_carries_the_flag() {
    let repo = TempDir::new().expect("temp repo");
    let ctx = ServerContext::with_features(repo.path().to_path_buf(), Features { ontology: false });
    assert!(!ctx.features.ontology);
}

#[test]
fn resolve_reads_a_disabled_flag_from_disk() {
    let repo = seed_rules("features:\n  ontology: false\n");
    let ctx = ServerContext::resolve(repo.path().to_path_buf()).expect("resolves");
    assert!(!ctx.features.ontology);
}

#[test]
fn resolve_defaults_to_enabled_when_config_is_absent() {
    let repo = TempDir::new().expect("temp repo");
    let ctx = ServerContext::resolve(repo.path().to_path_buf()).expect("resolves");
    assert!(ctx.features.ontology);
}

#[test]
fn resolve_errors_on_a_broken_config() {
    let repo = seed_rules("just a bare string\n");
    let error = ServerContext::resolve(repo.path().to_path_buf()).expect_err("broken config");
    assert!(matches!(error, FeaturesError::Parse { .. }));
}

#[test]
fn true_north_server_resolve_defaults_to_enabled() {
    let repo = TempDir::new().expect("temp repo");
    let server = TrueNorthServer::resolve(repo.path().to_path_buf()).expect("resolves");
    assert!(server.ctx.features.ontology);
}

/// The ontology tool names, as registered by the `#[tool]` macro from the method names.
const ONTOLOGY_TOOLS: [&str; 2] = ["truenorth_generate_ontology", "truenorth_verify_ontology"];

/// Build a server from an injected context with the given ontology flag.
fn server_with_ontology(enabled: bool) -> TrueNorthServer {
    let repo = TempDir::new().expect("temp repo");
    TrueNorthServer::from_context(ServerContext::with_features(
        repo.path().to_path_buf(),
        Features { ontology: enabled },
    ))
}

#[test]
fn enabled_registers_both_ontology_tools() {
    let server = server_with_ontology(true);
    for name in ONTOLOGY_TOOLS {
        assert!(
            server.tool_router.has_route(name),
            "enabled server must register `{name}`"
        );
    }
}

#[test]
fn disabled_registers_neither_ontology_tool() {
    let server = server_with_ontology(false);
    for name in ONTOLOGY_TOOLS {
        assert!(
            !server.tool_router.has_route(name),
            "disabled server must not register `{name}`"
        );
    }
}

#[test]
fn disabled_leaves_a_non_ontology_tool_registered() {
    // A representative non-ontology tool stays registered when ontology is off (R2.3).
    let disabled = server_with_ontology(false);
    let enabled = server_with_ontology(true);
    // `index_skills` is a catalog tool, unaffected by the ontology flag.
    assert!(disabled.tool_router.has_route("index_skills"));
    // The two servers differ only by the ontology tools.
    let disabled_count = disabled.tool_router.list_all().len();
    let enabled_count = enabled.tool_router.list_all().len();
    assert_eq!(enabled_count - disabled_count, ONTOLOGY_TOOLS.len());
}
