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
