//! Tests for the ontology tools (task 12.2).
//!
//! Included from `ontology.rs` via `#[path]`, so `super` is the ontology module.
//!
//! Requirements: 4.1, 4.2, 4.4, 4.7, 4.8, 4.10, 9.9, 9.10.

use super::*;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

fn server_at(root: &Path) -> TrueNorthServer {
    TrueNorthServer {
        ctx: Arc::new(crate::server::ServerContext::new(root.to_path_buf())),
        tool_router: rmcp::handler::server::router::tool::ToolRouter::new(),
    }
}

/// Write a valid ontology fixture at specs/ontology.yaml.
fn seed_ontology_file(root: &Path) {
    let ontology = seed_ontology("orders", &["src/order.rs".to_string()]);
    let mut with_alias = ontology;
    with_alias.entities[0].prohibited_aliases = vec!["is_deleted".to_string()];
    let yaml = serde_yaml::to_string(&with_alias).expect("serialize");
    fs::create_dir_all(root.join("specs")).expect("specs dir");
    fs::write(ontology_path(root), yaml).expect("write ontology");
}

#[test]
fn seed_builds_valid_ontology() {
    // Requirement 4.3: entities and baseline constraints are present.
    let ontology = seed_ontology("order-fulfilment", &["src/order.rs".to_string()]);
    assert_eq!(ontology.domain, "order-fulfilment");
    assert_eq!(ontology.entities.len(), 1);
    assert_eq!(ontology.entities[0].name, "order");
    assert_eq!(ontology.constraints.len(), 2);
    assert_eq!(ontology.constraints[0].id, "C-01");
    assert_eq!(ontology.constraints[1].id, "C-02");
}

#[test]
fn entity_name_from_uses_file_stem() {
    assert_eq!(entity_name_from("src/models/order.rs"), "order");
    assert_eq!(entity_name_from("specs/product/SCOPE.md"), "SCOPE");
    assert_eq!(entity_name_from(""), "Entity");
}

#[test]
fn now_iso8601_is_well_formed() {
    let ts = now_iso8601();
    // YYYY-MM-DDTHH:MM:SSZ is 20 chars, ending in Z.
    assert_eq!(ts.len(), 20, "timestamp `{ts}` is not 20 chars");
    assert!(ts.ends_with('Z'));
    assert_eq!(&ts[4..5], "-");
    assert_eq!(&ts[10..11], "T");
}

#[test]
fn civil_from_days_matches_known_dates() {
    // Day 0 is the unix epoch.
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    // 2000-01-01 is 10957 days after the epoch.
    assert_eq!(civil_from_days(10_957), (2000, 1, 1));
}

#[test]
fn read_ontology_errors_when_absent() {
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let error = server.read_ontology().expect_err("no ontology");
    assert!(error.message.contains("not found"));
}

#[test]
fn read_ontology_parses_a_valid_file() {
    let dir = tempdir().expect("temp dir");
    seed_ontology_file(dir.path());
    let server = server_at(dir.path());
    let ontology = server.read_ontology().expect("parse ontology");
    assert_eq!(ontology.domain, "orders");
}

#[test]
fn resolve_scope_uses_explicit_paths() {
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    let scope = server
        .resolve_scope(Some(vec!["a.rs".to_string(), "b.rs".to_string()]))
        .expect("explicit scope");
    assert_eq!(scope, vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]);
}

#[test]
fn scan_scope_flags_prohibited_alias() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_ontology_file(root);
    let server = server_at(root);
    let ontology = server.read_ontology().expect("read");

    // Write a file that uses the prohibited alias.
    fs::write(root.join("order.rs"), "struct Order { is_deleted: bool }\n").expect("write code");
    let violations = scan_scope(root, &[PathBuf::from("order.rs")], &ontology);
    assert!(!violations.is_empty());
    assert!(violations[0].message.contains("is_deleted"));
}

#[test]
fn scan_scope_clean_code_has_no_violations() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_ontology_file(root);
    let server = server_at(root);
    let ontology = server.read_ontology().expect("read");

    fs::write(
        root.join("order.rs"),
        "struct Order { deleted_at: Option<u64> }\n",
    )
    .expect("write");
    let violations = scan_scope(root, &[PathBuf::from("order.rs")], &ontology);
    assert!(violations.is_empty());
}
