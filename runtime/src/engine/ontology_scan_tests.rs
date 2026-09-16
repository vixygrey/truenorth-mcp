//! Property and unit tests for the ontology scan (task 6.2).
//!
//! Included from `ontology_scan.rs` via `#[path]`, so `super` is the ontology_scan
//! module.
//!
//! Property 1: for every scanned file and every prohibited alias that appears as an
//! identifier, `scan` returns at least one violation citing the owning constraint id.
//!
//! Requirements: 4.5, 4.6.

use super::*;
use crate::engine::spec::{Constraint, Entity, Ontology};
use std::collections::BTreeMap;

/// The design's example ontology: an `Order` entity with prohibited aliases, and the
/// C-01 and C-02 constraints.
fn order_ontology() -> Ontology {
    Ontology {
        version: "1".to_string(),
        domain: "order-fulfillment".to_string(),
        last_updated: "2026-07-26T00:00:00Z".to_string(),
        entities: vec![Entity {
            name: "Order".to_string(),
            description: "A customer purchase moving through fulfillment.".to_string(),
            primary_key: "order_id".to_string(),
            invariants: vec!["total_cents >= 0".to_string()],
            states: vec!["draft".to_string(), "placed".to_string()],
            transitions: BTreeMap::new(),
            prohibited_aliases: vec![
                "is_deleted".to_string(),
                "order_no".to_string(),
                "purchase".to_string(),
            ],
        }],
        constraints: vec![
            Constraint {
                id: "C-01".to_string(),
                rule: "Soft deletion uses deleted_at, never a boolean flag.".to_string(),
            },
            Constraint {
                id: "C-02".to_string(),
                rule: "Boolean state flags (is_*) are prohibited; model states explicitly."
                    .to_string(),
            },
        ],
    }
}

#[test]
fn prohibited_alias_as_identifier_yields_a_violation() {
    // Property 1: an alias appearing as an identifier yields at least one violation.
    let ontology = order_ontology();
    let code = "struct Order {\n    is_deleted: bool,\n}\n";
    let violations = RegexAnalyzer.scan(Path::new("src/order.rs"), code, &ontology);
    assert!(
        !violations.is_empty(),
        "a prohibited alias must yield a violation"
    );
}

#[test]
fn is_deleted_cites_c02_with_remediation() {
    // Requirement 4.6: the violation cites the constraint id, the term, and a remediation
    // hint. `is_deleted` matches the `is_*` glob in C-02, so C-02 owns it.
    let ontology = order_ontology();
    let code = "    is_deleted: bool,\n";
    let violations = RegexAnalyzer.scan(Path::new("src/models/order.rs"), code, &ontology);

    let violation = violations
        .iter()
        .find(|v| v.message.contains("is_deleted"))
        .expect("is_deleted yields a violation");
    assert_eq!(violation.constraint_id, "C-02");
    assert!(violation.message.contains("[Ontology Gate C-02]"));
    assert!(violation.message.contains("`is_deleted`"));
    assert!(violation.message.contains("model states explicitly"));
    assert!(violation.message.contains("src/models/order.rs:1"));
    assert_eq!(violation.line, 1);
}

#[test]
fn literal_alias_matches_its_constraint() {
    // `order_no` is not referenced by any constraint rule, so it cites a synthesized
    // entity-derived id rather than a real constraint.
    let ontology = order_ontology();
    let code = "let order_no = 5;\n";
    let violations = RegexAnalyzer.scan(Path::new("src/order.rs"), code, &ontology);
    let violation = violations
        .iter()
        .find(|v| v.message.contains("order_no"))
        .expect("order_no yields a violation");
    assert_eq!(violation.constraint_id, "ORDER-ALIAS");
    assert!(violation.message.contains("order_id"));
}

#[test]
fn every_prohibited_alias_cites_an_id() {
    // Property 1: every alias that occurs yields a violation with a non-empty id.
    let ontology = order_ontology();
    let code = "is_deleted order_no purchase\n";
    let violations = RegexAnalyzer.scan(Path::new("f.txt"), code, &ontology);
    let mut hit: Vec<&str> = violations
        .iter()
        .map(|v| v.constraint_id.as_str())
        .collect();
    hit.sort_unstable();
    hit.dedup();
    assert_eq!(violations.len(), 3, "all three aliases must be flagged");
    for v in &violations {
        assert!(!v.constraint_id.is_empty(), "every violation cites an id");
    }
}

#[test]
fn substring_inside_longer_identifier_does_not_match() {
    // `is_deleted` must not match inside `is_deleted_at`, so a longer identifier is safe.
    let ontology = order_ontology();
    let code = "    is_deleted_at: Option<DateTime>,\n";
    let violations = RegexAnalyzer.scan(Path::new("src/order.rs"), code, &ontology);
    assert!(
        violations
            .iter()
            .all(|v| !v.message.contains("`is_deleted`")),
        "a substring inside a longer identifier must not match"
    );
}

#[test]
fn clean_code_yields_no_violations() {
    let ontology = order_ontology();
    let code = "struct Order {\n    order_id: u64,\n    deleted_at: Option<DateTime>,\n}\n";
    let violations = RegexAnalyzer.scan(Path::new("src/order.rs"), code, &ontology);
    assert!(violations.is_empty(), "clean code has no violations");
}

#[test]
fn line_numbers_are_one_based() {
    let ontology = order_ontology();
    let code = "line one\nline two\nis_deleted here\n";
    let violations = RegexAnalyzer.scan(Path::new("f.rs"), code, &ontology);
    let violation = violations.first().expect("one violation");
    assert_eq!(violation.line, 3);
}

#[test]
fn pick_analyzer_returns_a_working_analyzer() {
    // The seam returns the regex baseline today. It must scan like the baseline.
    let ontology = order_ontology();
    let analyzer = pick_analyzer(Path::new("src/order.rs"));
    let violations = analyzer.scan(Path::new("src/order.rs"), "is_deleted: bool\n", &ontology);
    assert!(!violations.is_empty());
}

// AST analyzer tests (task 6.3). These compile only under the `tree-sitter` feature. They
// prove the precision win over the regex baseline: the AST analyzer flags an alias at a
// real identifier, but not the same alias inside a comment or a string literal.
#[cfg(feature = "tree-sitter")]
mod ast {
    use super::*;

    #[test]
    fn ast_flags_alias_at_an_identifier() {
        // An alias used as a struct field identifier yields a violation.
        let ontology = order_ontology();
        let analyzer =
            AstAnalyzer::for_path(Path::new("src/order.rs")).expect("Rust has a bundled grammar");
        let code = "struct Order {\n    is_deleted: bool,\n}\n";
        let violations = analyzer.scan(Path::new("src/order.rs"), code, &ontology);

        let violation = violations
            .iter()
            .find(|v| v.message.contains("`is_deleted`"))
            .expect("is_deleted at an identifier yields a violation");
        assert_eq!(violation.constraint_id, "C-02");
        assert_eq!(violation.line, 2);
    }

    #[test]
    fn ast_does_not_flag_an_alias_in_a_comment() {
        // The precision win: the regex baseline flags the alias in a comment, the AST
        // analyzer does not.
        let ontology = order_ontology();
        let code = "struct Order {\n    // is_deleted is banned; use deleted_at\n    deleted_at: u64,\n}\n";
        let path = Path::new("src/order.rs");

        let regex_hits = RegexAnalyzer.scan(path, code, &ontology);
        assert!(
            regex_hits
                .iter()
                .any(|v| v.message.contains("`is_deleted`")),
            "the regex baseline flags the alias in the comment"
        );

        let analyzer = AstAnalyzer::for_path(path).expect("Rust grammar");
        let ast_hits = analyzer.scan(path, code, &ontology);
        assert!(
            ast_hits.is_empty(),
            "the AST analyzer must not flag an alias inside a comment"
        );
    }

    #[test]
    fn ast_does_not_flag_an_alias_in_a_string_literal() {
        // An alias inside a string literal is not an identifier, so the AST analyzer skips
        // it while the regex baseline flags it.
        let ontology = order_ontology();
        let code = "fn log() {\n    println!(\"is_deleted was removed\");\n}\n";
        let path = Path::new("src/log.rs");

        let regex_hits = RegexAnalyzer.scan(path, code, &ontology);
        assert!(
            regex_hits
                .iter()
                .any(|v| v.message.contains("`is_deleted`")),
            "the regex baseline flags the alias in the string"
        );

        let analyzer = AstAnalyzer::for_path(path).expect("Rust grammar");
        let ast_hits = analyzer.scan(path, code, &ontology);
        assert!(
            ast_hits.is_empty(),
            "the AST analyzer must not flag an alias inside a string literal"
        );
    }

    #[test]
    fn ast_does_not_flag_a_substring_of_a_longer_identifier() {
        // `is_deleted` inside `is_deleted_at` is a different identifier token.
        let ontology = order_ontology();
        let analyzer = AstAnalyzer::for_path(Path::new("src/order.rs")).expect("Rust grammar");
        let code = "struct Order {\n    is_deleted_at: Option<u64>,\n}\n";
        let violations = analyzer.scan(Path::new("src/order.rs"), code, &ontology);
        assert!(
            violations
                .iter()
                .all(|v| !v.message.contains("`is_deleted`")),
            "a substring of a longer identifier must not match"
        );
    }

    #[test]
    fn ast_attribution_matches_the_regex_baseline() {
        // For an alias at a real identifier, the AST analyzer produces the same constraint
        // id and message as the regex baseline. Only the detection precision differs.
        let ontology = order_ontology();
        let code = "let is_deleted = true;\n";
        let path = Path::new("src/order.rs");

        let regex_hit = RegexAnalyzer
            .scan(path, code, &ontology)
            .into_iter()
            .find(|v| v.message.contains("`is_deleted`"))
            .expect("regex flags the identifier");
        let ast_hit = AstAnalyzer::for_path(path)
            .expect("Rust grammar")
            .scan(path, code, &ontology)
            .into_iter()
            .find(|v| v.message.contains("`is_deleted`"))
            .expect("AST flags the identifier");

        assert_eq!(ast_hit.constraint_id, regex_hit.constraint_id);
        assert_eq!(ast_hit.message, regex_hit.message);
        assert_eq!(ast_hit.line, regex_hit.line);
    }

    #[test]
    fn pick_analyzer_selects_ast_for_rust_and_regex_otherwise() {
        // A `.rs` file gets the AST analyzer: it skips a comment-only alias. A file with no
        // grammar (`.txt`) falls back to the regex baseline, which flags it.
        let ontology = order_ontology();
        let comment_only = "// is_deleted\n";

        let rust = pick_analyzer(Path::new("src/order.rs"));
        assert!(
            rust.scan(Path::new("src/order.rs"), comment_only, &ontology)
                .is_empty(),
            "the Rust file uses the AST analyzer, which skips the comment"
        );

        let text = pick_analyzer(Path::new("notes.txt"));
        assert!(
            !text
                .scan(Path::new("notes.txt"), comment_only, &ontology)
                .is_empty(),
            "the unsupported file falls back to the regex baseline"
        );
    }
}
