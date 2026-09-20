//! Example tests for the guard deterministic layer (task 1, issue #293).
//!
//! Included from `guard.rs` via `#[path]`, so `super` is the `guard` module. These pin the
//! deterministic layer: a protected-path write and a secret-content write each block with a
//! named packet and no Jev call, a clean change passes to the probabilistic layer, and the
//! packet names no secret value (jev-active-guardrail R2, R6, P34, P35, P40).

use super::*;

/// The protected set the example tests reuse: two directories and one file.
fn protected() -> Vec<String> {
    vec![
        "specs/".to_string(),
        ".github/workflows/".to_string(),
        "LICENSE".to_string(),
    ]
}

/// Build a proposed change over one path with the given content.
fn change(path: &str, content: &str) -> ProposedChange {
    ProposedChange {
        paths: vec![path.to_string()],
        content: content.to_string(),
    }
}

/// Extract the packet from a decision, or fail the test.
fn expect_block(decision: Option<GuardDecision>) -> NeutralizationPacket {
    match decision {
        Some(GuardDecision::Block(packet)) => packet,
        other => panic!("expected a Block, got {other:?}"),
    }
}

#[test]
fn a_write_under_a_protected_dir_blocks() {
    let decision = deterministic_layer(&change("specs/adr/0001.md", "clean body"), &protected());
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "protected-path");
    assert_eq!(packet.offending_value.as_deref(), Some("specs/adr/0001.md"));
    assert!(
        packet.suggested_fix.is_none(),
        "a deterministic block has no self-heal"
    );
}

#[test]
fn a_write_to_a_protected_file_blocks() {
    let decision = deterministic_layer(&change("LICENSE", "clean body"), &protected());
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "protected-path");
    assert_eq!(packet.offending_value.as_deref(), Some("LICENSE"));
}

#[test]
fn a_write_under_the_workflows_dir_blocks() {
    let decision = deterministic_layer(
        &change(".github/workflows/ci.yml", "clean body"),
        &protected(),
    );
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "protected-path");
    assert_eq!(
        packet.offending_value.as_deref(),
        Some(".github/workflows/ci.yml")
    );
}

#[test]
fn the_protected_path_check_runs_before_the_secret_scan() {
    // The content also carries a secret. The protected-path block wins, because the layer
    // runs the path match first (R2, deterministic layer order).
    let decision = deterministic_layer(
        &change("specs/plan.md", "AWS_SECRET_ACCESS_KEY=abc123"),
        &protected(),
    );
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "protected-path");
}

#[test]
fn secret_content_blocks_and_names_the_marker_not_the_value() {
    let secret_body = "AWS_SECRET_ACCESS_KEY=super-sensitive-value";
    let decision = deterministic_layer(&change("src/config.rs", secret_body), &protected());
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "secret");
    // The offending value names the matched marker, never the secret value (R6.3, P40).
    let offending = packet
        .offending_value
        .clone()
        .expect("a secret block names a marker");
    assert_eq!(offending, "secret-marker");
    assert!(
        !offending.contains("sensitive"),
        "the marker carries no secret value"
    );
    assert!(
        !packet.remediation.contains("sensitive"),
        "the remediation carries no secret value"
    );
}

#[test]
fn credentials_content_blocks_with_the_credentials_marker() {
    let decision = deterministic_layer(
        &change("src/auth.rs", "load the credentials file"),
        &protected(),
    );
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "secret");
    assert_eq!(
        packet.offending_value.as_deref(),
        Some("credentials-marker")
    );
}

#[test]
fn env_file_content_blocks_with_the_env_marker() {
    // The env-file denylist pattern is path-anchored to the end of the string, so it matches
    // a `.env` path form that ends the content, such as a bare path on its own line.
    let decision = deterministic_layer(&change("notes.txt", "config/.env"), &protected());
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "secret");
    assert_eq!(packet.offending_value.as_deref(), Some("env-file"));
}

#[test]
fn pem_file_content_blocks_with_the_pem_marker() {
    // The pem-file denylist pattern is anchored to a `.pem` extension at the end of the
    // string, so the content ends with the path form.
    let decision = deterministic_layer(&change("notes.txt", "server.pem"), &protected());
    let packet = expect_block(decision);

    assert_eq!(packet.violated_check, "secret");
    assert_eq!(packet.offending_value.as_deref(), Some("pem-file"));
}

#[test]
fn a_clean_change_passes_the_deterministic_layer() {
    // No protected path and no secret content, so the layer returns None and the caller
    // runs the probabilistic layer next (issue #294).
    let decision = deterministic_layer(
        &change("src/main.rs", "fn main() { println!(\"ok\"); }"),
        &protected(),
    );
    assert!(
        decision.is_none(),
        "a clean change is not blocked deterministically"
    );
}

#[test]
fn a_multi_path_change_blocks_on_any_protected_path() {
    let change = ProposedChange {
        paths: vec!["src/main.rs".to_string(), "specs/plan.md".to_string()],
        content: "clean body".to_string(),
    };
    let packet = expect_block(deterministic_layer(&change, &protected()));

    assert_eq!(packet.violated_check, "protected-path");
    assert_eq!(packet.offending_value.as_deref(), Some("specs/plan.md"));
}

#[test]
fn the_deterministic_layer_is_the_same_on_repeated_evaluation() {
    let change = change("specs/plan.md", "clean body");
    let first = deterministic_layer(&change, &protected());
    let second = deterministic_layer(&change, &protected());
    assert_eq!(
        first, second,
        "the same input returns the same result (R2.5)"
    );
}

#[test]
fn the_neutralization_packet_serializes_without_a_secret() {
    let packet = expect_block(deterministic_layer(
        &change("src/config.rs", "SECRET_TOKEN=do-not-leak"),
        &protected(),
    ));
    let json = serde_json::to_string(&packet).expect("the packet serializes");
    assert!(
        !json.contains("do-not-leak"),
        "the serialized packet carries no secret value"
    );
    assert!(json.contains("\"violated_check\":\"secret\""));
}
