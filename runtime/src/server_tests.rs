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

/// The required `.agent/` layout entries, mirroring `agent_ws::REQUIRED_ENTRIES`. Seeded
/// as directories (no extension in the tuple's `dir` flag) or files.
const LAYOUT_DIRS: [&str; 5] = ["config", "spec", "tasks", "memories", "telemetry"];
const LAYOUT_FILES: [&str; 8] = [
    "layout.yml",
    "profile.yml",
    "config/rules.yml",
    "spec/requirements.md",
    "tasks/state.yml",
    "memories/lessons.md",
    "memories/glossary.md",
    "telemetry/runs.yml",
];

/// Seed a complete, valid `.agent/` layout under a fresh temp repository root, so
/// `ServerContext::resolve` validates and caches the contract (Requirement 1.12).
fn seed_valid_layout() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(".agent");
    for dir in LAYOUT_DIRS {
        fs::create_dir_all(agent.join(dir)).expect("create layout dir");
    }
    for file in LAYOUT_FILES {
        let path = agent.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        // `config/rules.yml` is parsed by the features reader, so it must be a valid YAML
        // mapping. Every other seeded file only needs to exist for the presence check.
        let body = if file == "config/rules.yml" {
            "features:\n  ontology: true\n"
        } else {
            "seed\n"
        };
        fs::write(&path, body).expect("write layout file");
    }
    repo
}

#[test]
fn resolve_caches_a_complete_layout_contract() {
    // The wiring: resolve validates `.agent/layout.yml` and caches the last valid contract.
    let repo = seed_valid_layout();
    let ctx = ServerContext::resolve(repo.path().to_path_buf()).expect("resolves");
    let cached = ctx
        .layout
        .last_valid()
        .expect("a complete layout is cached");
    assert_eq!(cached.agent_root, repo.path().join(".agent"));
}

#[test]
fn resolve_skips_when_no_layout_contract_is_present() {
    // A legacy `specs/` cockpit or an unscaffolded repo has no `.agent/layout.yml`. The
    // validation is non-fatal and caches nothing (Requirement 1.12).
    let repo = TempDir::new().expect("temp repo");
    let ctx = ServerContext::resolve(repo.path().to_path_buf()).expect("resolves");
    assert!(ctx.layout.last_valid().is_none());
}

#[test]
fn resolve_is_non_fatal_on_an_incomplete_layout_contract() {
    // A present-but-incomplete contract logs a warning and keeps serving, retaining the
    // last valid contract (here, none). Resolve still succeeds (Requirement 1.12).
    let repo = seed_valid_layout();
    fs::remove_file(repo.path().join(".agent").join("tasks").join("state.yml"))
        .expect("remove a required file");
    let ctx = ServerContext::resolve(repo.path().to_path_buf()).expect("resolves");
    assert!(ctx.layout.last_valid().is_none());
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
