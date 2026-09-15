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
