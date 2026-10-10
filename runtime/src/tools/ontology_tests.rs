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
        ctx: Arc::new(crate::server::ServerContext::with_features(
            root.to_path_buf(),
            crate::engine::features::Features::default(),
        )),
        tool_router: rmcp::handler::server::router::tool::ToolRouter::new(),
    }
}

/// Write a valid ontology fixture at .agent/ontology.yml.
fn seed_ontology_file(root: &Path) {
    let ontology = seed_ontology("orders", &["src/order.rs".to_string()]);
    let mut with_alias = ontology;
    with_alias.entities[0].prohibited_aliases = vec!["is_deleted".to_string()];
    let yaml = serde_yaml::to_string(&with_alias).expect("serialize");
    let path = ontology_path(root);
    fs::create_dir_all(path.parent().expect("parent")).expect("agent dir");
    fs::write(path, yaml).expect("write ontology");
}

#[test]
fn seed_builds_valid_ontology() {
    // Requirement 4.3: entities and baseline constraints are present.
    let ontology = seed_ontology("order-fulfillment", &["src/order.rs".to_string()]);
    assert_eq!(ontology.domain, "order-fulfillment");
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

// ── Path reconciliation (issue #143, Property 19, 20, 21) ─────────────────────────────

use rmcp::handler::server::wrapper::Parameters;

/// Parse the `.agent/ontology.yml` file at a root into an Ontology.
fn read_agent_ontology(root: &Path) -> Ontology {
    let text = fs::read_to_string(root.join(".agent/ontology.yml")).expect("read .agent ontology");
    serde_yaml::from_str(&text).expect("parse")
}

#[test]
fn read_ontology_falls_back_to_legacy_specs() {
    // Requirement 4.3: with no .agent/ontology.yml, a legacy specs/ontology.yaml is read.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let ontology = seed_ontology("legacy-orders", &["src/order.rs".to_string()]);
    let yaml = serde_yaml::to_string(&ontology).expect("serialize");
    fs::create_dir_all(root.join("specs")).expect("specs dir");
    fs::write(root.join("specs/ontology.yaml"), yaml).expect("write legacy");

    let server = server_at(root);
    let read = server.read_ontology().expect("legacy fallback read");
    assert_eq!(read.domain, "legacy-orders");
}

#[tokio::test]
async fn generate_writes_under_agent() {
    // Requirement 4.1, 4.2: the generate tool writes .agent/ontology.yml, not specs/.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let server = server_at(root);

    let result = server
        .truenorth_generate_ontology(
            Parameters(GenerateOntologyArgs {
                domain: "orders".to_string(),
                source_paths: vec!["src/order.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect("generate");

    assert!(root.join(".agent/ontology.yml").is_file());
    assert!(!root.join("specs/ontology.yaml").exists());
    assert_eq!(read_agent_ontology(root).domain, "orders");
    let text = result.content[0].as_text().expect("text result");
    let json: serde_json::Value = serde_json::from_str(&text.text).expect("JSON result");
    assert_eq!(json["receipt"]["operation"], "truenorth_generate_ontology");
    assert_eq!(json["receipt"]["changes"][0]["path"], ".agent/ontology.yml");
    assert_eq!(
        json["receipt"]["changes"][0]["sha256"],
        crate::engine::digest::sha256(&fs::read(root.join(".agent/ontology.yml")).expect("read"))
    );
}

#[tokio::test]
async fn generate_overwrites_the_empty_stub() {
    // Requirement 4.6: the empty stub is overwritable, so a resource-seeded stub does not
    // deadlock the generate tool.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let stub = serde_yaml::to_string(&Ontology::empty_stub()).expect("serialize stub");
    fs::create_dir_all(root.join(".agent")).expect("agent dir");
    fs::write(root.join(".agent/ontology.yml"), stub).expect("seed stub");

    let server = server_at(root);
    server
        .truenorth_generate_ontology(
            Parameters(GenerateOntologyArgs {
                domain: "orders".to_string(),
                source_paths: vec!["src/order.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect("overwrite stub");

    assert_eq!(read_agent_ontology(root).domain, "orders");
}

#[tokio::test]
async fn generate_refuses_to_overwrite_a_real_ontology() {
    // Requirement 4.7: a real ontology is left unchanged and the tool errors.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_ontology_file(root); // domain "orders", a real ontology
    let before = fs::read_to_string(root.join(".agent/ontology.yml")).expect("read before");

    let server = server_at(root);
    let error = server
        .truenorth_generate_ontology(
            Parameters(GenerateOntologyArgs {
                domain: "different".to_string(),
                source_paths: vec!["src/other.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(1)),
        )
        .await
        .expect_err("refuse overwrite");
    assert!(error.message.contains(".agent/ontology.yml"));

    let after = fs::read_to_string(root.join(".agent/ontology.yml")).expect("read after");
    assert_eq!(before, after, "the real ontology is unchanged");
}

#[tokio::test]
async fn verify_notes_the_empty_stub() {
    // Requirement 4.10: verify against the empty stub passes with the not-yet-defined note.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let stub = serde_yaml::to_string(&Ontology::empty_stub()).expect("serialize stub");
    fs::create_dir_all(root.join(".agent")).expect("agent dir");
    fs::write(root.join(".agent/ontology.yml"), stub).expect("seed stub");

    let server = server_at(root);
    let result = server
        .truenorth_verify_ontology(Parameters(VerifyOntologyArgs { scope_paths: None }))
        .await
        .expect("verify passes on stub");

    let text = result.content[0]
        .as_text()
        .expect("text content")
        .text
        .clone();
    assert!(text.contains("not yet defined"), "note present: {text}");
}

fn tool_json(result: CallToolResult) -> serde_json::Value {
    let text = result.content[0].as_text().expect("text result");
    serde_json::from_str(&text.text).expect("JSON result")
}

#[tokio::test]
async fn generate_rejects_domain_and_source_path_boundaries_without_writing() {
    let dir = tempdir().expect("temp dir");
    let server = server_at(dir.path());
    for args in [
        GenerateOntologyArgs {
            domain: String::new(),
            source_paths: vec!["src".to_string()],
        },
        GenerateOntologyArgs {
            domain: "é".repeat(MAX_DOMAIN + 1),
            source_paths: vec!["src".to_string()],
        },
        GenerateOntologyArgs {
            domain: "orders".to_string(),
            source_paths: Vec::new(),
        },
    ] {
        let error = server
            .truenorth_generate_ontology(
                Parameters(args),
                rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(9)),
            )
            .await
            .expect_err("invalid generation input");
        assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(!ontology_path(dir.path()).exists());
    }
}

#[tokio::test]
async fn generate_accepts_a_multibyte_domain_at_the_character_cap() {
    let dir = tempdir().expect("temp dir");
    let domain = "é".repeat(MAX_DOMAIN);
    server_at(dir.path())
        .truenorth_generate_ontology(
            Parameters(GenerateOntologyArgs {
                domain: domain.clone(),
                source_paths: vec!["src/order.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(10)),
        )
        .await
        .expect("character-count boundary");
    assert_eq!(read_agent_ontology(dir.path()).domain, domain);
}

#[tokio::test]
async fn generate_rejects_malformed_existing_ontology_without_replacing_it() {
    let dir = tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(".agent")).expect("agent dir");
    let malformed = "domain: [unterminated\n";
    fs::write(ontology_path(dir.path()), malformed).expect("malformed ontology");

    let error = server_at(dir.path())
        .truenorth_generate_ontology(
            Parameters(GenerateOntologyArgs {
                domain: "orders".to_string(),
                source_paths: vec!["src".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(11)),
        )
        .await
        .expect_err("malformed existing ontology");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_REQUEST);
    assert_eq!(
        fs::read_to_string(ontology_path(dir.path())).expect("unchanged file"),
        malformed
    );
}

#[tokio::test]
async fn oversized_generate_result_does_not_write_ontology() {
    let dir = tempdir().expect("temp dir");
    let server = TrueNorthServer::from_context(crate::server::ServerContext::with_config(
        dir.path().to_path_buf(),
        crate::engine::features::Features::default(),
        crate::engine::features::TokenCaps {
            skill_lean_tokens: 100,
            tool_payload_tokens: 1,
        },
    ));
    let error = server
        .truenorth_generate_ontology(
            Parameters(GenerateOntologyArgs {
                domain: "orders".to_string(),
                source_paths: vec!["src/order.rs".to_string()],
            }),
            rmcp::handler::server::tool::RequestId(rmcp::model::NumberOrString::Number(12)),
        )
        .await
        .expect_err("result cap");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    assert!(!ontology_path(dir.path()).exists());
}

#[tokio::test]
async fn verify_reports_empty_clean_and_violating_explicit_scopes() {
    let dir = tempdir().expect("temp dir");
    seed_ontology_file(dir.path());
    let server = server_at(dir.path());

    let empty = tool_json(
        server
            .truenorth_verify_ontology(Parameters(VerifyOntologyArgs {
                scope_paths: Some(Vec::new()),
            }))
            .await
            .expect("empty scope"),
    );
    assert_eq!(empty["passed"], true);
    assert_eq!(empty["files_scanned"], 0);

    fs::write(
        dir.path().join("clean.rs"),
        "struct Order { deleted_at: Option<u64> }\n",
    )
    .expect("clean source");
    let clean = tool_json(
        server
            .truenorth_verify_ontology(Parameters(VerifyOntologyArgs {
                scope_paths: Some(vec!["clean.rs".to_string(), "missing.rs".to_string()]),
            }))
            .await
            .expect("clean scope"),
    );
    assert_eq!(clean["passed"], true);
    assert_eq!(clean["files_scanned"], 2);

    fs::write(
        dir.path().join("bad.rs"),
        "struct Order { is_deleted: bool }\n",
    )
    .expect("violating source");
    let error = server
        .truenorth_verify_ontology(Parameters(VerifyOntologyArgs {
            scope_paths: Some(vec!["bad.rs".to_string()]),
        }))
        .await
        .expect_err("prohibited alias");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_REQUEST);
    assert_eq!(error.data.expect("violation count")["violations"], 1);
}

#[tokio::test]
async fn verify_maps_malformed_ontology_and_unavailable_git_scope() {
    if !crate::isolated_process_tests::in_child(
        "tools::ontology::tests::verify_maps_malformed_ontology_and_unavailable_git_scope",
        &[],
    ) {
        return;
    }
    let malformed_dir = tempdir().expect("temp dir");
    fs::create_dir_all(malformed_dir.path().join(".agent")).expect("agent dir");
    fs::write(ontology_path(malformed_dir.path()), "entities: [").expect("malformed");
    let malformed = server_at(malformed_dir.path())
        .truenorth_verify_ontology(Parameters(VerifyOntologyArgs {
            scope_paths: Some(Vec::new()),
        }))
        .await
        .expect_err("parse failure");
    assert_eq!(malformed.code, rmcp::model::ErrorCode::INVALID_REQUEST);

    let nongit_dir = tempdir().expect("temp dir");
    seed_ontology_file(nongit_dir.path());
    let scope_error = server_at(nongit_dir.path())
        .truenorth_verify_ontology(Parameters(VerifyOntologyArgs { scope_paths: None }))
        .await
        .expect_err("git scope unavailable");
    assert_eq!(scope_error.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
}

#[test]
fn primary_ontology_takes_precedence_over_legacy_and_parse_io_is_typed() {
    let dir = tempdir().expect("temp dir");
    seed_ontology_file(dir.path());
    let legacy = seed_ontology("legacy", &["legacy.rs".to_string()]);
    fs::create_dir_all(dir.path().join("specs")).expect("specs dir");
    fs::write(
        legacy_ontology_path(dir.path()),
        serde_yaml::to_string(&legacy).expect("serialize"),
    )
    .expect("legacy ontology");
    assert_eq!(
        ontology_read_path(dir.path()).expect("resolved"),
        ontology_path(dir.path())
    );

    let missing = read_and_parse(&dir.path().join("absent.yml")).expect_err("missing");
    assert_eq!(missing.code, rmcp::model::ErrorCode::INVALID_REQUEST);
    assert_eq!(
        receipt_error("invalid change".to_string()).code,
        rmcp::model::ErrorCode::INTERNAL_ERROR
    );
}
