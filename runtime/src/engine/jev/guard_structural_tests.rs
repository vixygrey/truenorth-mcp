//! Structural and scope smoke checks for the active guardrail (task 8, issue #298 spec #300).
//!
//! Included from `guard.rs` via `#[path]`, so `super` is the `guard` module. These are
//! structural guards, not behavior tests. They read the guardrail source files and assert the
//! scope boundaries the design pins:
//!
//! - The guard CLI subcommand carries no test, so `cargo test` does not run the bin (R8.2).
//! - `evaluate_guard` and the combine step hold no hardcoded threshold; the values come from
//!   `JevConfig` (R5.4).
//! - The default build compiles no reqwest for the guardrail source (R8.5).
//! - The guard performs no write and spawns no secondary agent, and it returns a suggested
//!   self-heal instruction only (R9.2, R9.3).
//!
//! The checks read the source text through the cargo manifest directory, the same technique
//! the harness structural checks use (`structural_tests.rs`).

use std::path::PathBuf;

/// The runtime crate root, from the cargo manifest directory.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read a guardrail source file under `src/engine/jev/`.
fn read_jev_source(file: &str) -> String {
    let path = crate_root().join("src/engine/jev").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("read the source `{file}`"))
}

/// The guardrail source files this suite audits. The test siblings are excluded, because a
/// test legitimately names a threshold value or a write helper in an assertion.
const GUARD_SOURCES: [&str; 4] = [
    "guard.rs",
    "guard_combine.rs",
    "guard_evaluate.rs",
    "guard_cli.rs",
];

#[test]
fn the_guard_cli_subcommand_carries_no_test() {
    // The guard subcommand lives on the jev-bench bin, which must carry no test, so
    // `cargo test` does not run the bin (R8.2). A `#[test]` in the bin would make the test
    // harness compile and run it under the feature.
    let bin = crate_root().join("src/bin/jev_bench.rs");
    let source = std::fs::read_to_string(&bin).expect("read the bin source");
    assert!(
        !source.contains("#[test]") && !source.contains("#[tokio::test]"),
        "the jev-bench bin, which holds the guard subcommand, must carry no test (R8.2)"
    );
}

#[test]
fn the_guard_holds_no_hardcoded_threshold() {
    // The thresholds come from `JevConfig`; the guard hardcodes none (R5.4). Scan the combine
    // and evaluate sources for a reference to a config threshold field, and assert no numeric
    // threshold literal is compared against a confidence. A stray `>= 0.85` or `>= 0.90` in
    // the decision path would be a hardcoded threshold defect.
    for file in ["guard_combine.rs", "guard_evaluate.rs"] {
        let source = read_jev_source(file);
        for forbidden in ["0.85", "0.90", "0.70", "0.60"] {
            assert!(
                !source.contains(forbidden),
                "{file} must hold no hardcoded threshold literal `{forbidden}`; \
                 thresholds come from JevConfig (R5.4)"
            );
        }
    }
    // The combine step reads the thresholds from the config, proving the values are injected.
    let combine = read_jev_source("guard_combine.rs");
    assert!(
        combine.contains("config.confidence_high")
            && combine.contains("config.destructive_threshold"),
        "the combine step must read the thresholds from JevConfig (R5.4)"
    );
}

#[test]
fn the_guard_source_references_no_reqwest() {
    // The default build compiles no reqwest for the guardrail (R8.5). The guardrail composes
    // the harness client trait, never reqwest directly. A reqwest reference in a guard source
    // would pull the HTTP crate into the default build.
    for file in GUARD_SOURCES {
        let source = read_jev_source(file);
        // Look for a real code reference (`use reqwest` or `reqwest::`), not the bare word in
        // a comment, so a doc note that names the crate does not trip the check.
        assert!(
            !source.contains("use reqwest") && !source.contains("reqwest::"),
            "{file} must reference no reqwest in code; the guardrail uses the client trait (R8.5)"
        );
    }
}

#[test]
fn the_guard_client_http_use_is_feature_gated() {
    // The guardrail reaches the real client only through a feature-gated path. Any mention of
    // the `client_http` module in a guard source must sit under a `jev-http` cfg, so the
    // default build compiles no HTTP client for the guardrail (R8.5). The guard core sources
    // name no client_http at all; only the tool and the bin build the client, and those are
    // cfg-gated in their own modules.
    for file in GUARD_SOURCES {
        let source = read_jev_source(file);
        // Target a code reference, not a doc mention. A `use ...client_http` or a
        // `client_http::` path in a guard source would pull the HTTP client into the default
        // build; a comment naming the module is fine.
        assert!(
            !source.contains("use crate::engine::jev::client_http")
                && !source.contains("super::client_http")
                && !source.contains("client_http::"),
            "{file} must not use client_http in code; the client is built in the tool and the \
             bin, behind jev-http (R8.5)"
        );
    }
}

#[test]
fn the_guard_performs_no_write() {
    // The guard decides; it never writes (R9.2). No guard source calls a write path.
    for file in GUARD_SOURCES {
        let source = read_jev_source(file);
        for forbidden in [
            "write_under_agent",
            "write_repo_seed",
            "fs::write",
            "std::fs::write",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} must call no write path (`{forbidden}`); the guard only decides (R9.2)"
            );
        }
    }
}

#[test]
fn the_guard_spawns_no_secondary_agent() {
    // The guard triggers no secondary agent and runs no autonomous repair (R9.2, ADR-G5). No
    // guard source spawns a process or a command.
    for file in GUARD_SOURCES {
        let source = read_jev_source(file);
        for forbidden in ["Command::new", "process::Command", "spawn("] {
            assert!(
                !source.contains(forbidden),
                "{file} must spawn no process (`{forbidden}`); the guard runs no secondary \
                 agent (R9.2, R9.3)"
            );
        }
    }
}

#[test]
fn a_rigor_block_carries_a_suggested_fix_instruction_only() {
    // The guard returns a suggested self-heal instruction only; it does not apply it (R9.3).
    // The neutralization packet carries the fix as an optional string, so the guard cannot
    // carry an executable action. This is a type-level guarantee: `suggested_fix` is an
    // `Option<String>`, an instruction, never a callable.
    let packet = super::NeutralizationPacket {
        violated_check: "rigor".to_string(),
        offending_value: None,
        remediation: "Re-align the change.".to_string(),
        suggested_fix: Some("REFACTOR_IMPORTS".to_string()),
    };
    // The fix is a plain instruction string the caller may act on; the guard never runs it.
    assert_eq!(packet.suggested_fix.as_deref(), Some("REFACTOR_IMPORTS"));
}
