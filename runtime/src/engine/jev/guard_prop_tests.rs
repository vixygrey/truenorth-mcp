//! Property tests for the guard deterministic layer (task 1, issue #293).
//!
//! Included from `guard.rs` via `#[path]`, so `super` is the `guard` module.
//!
//! Feature: jev-active-guardrail.
//!
//! - Property 34: deterministic protected-path block. For all changes that write a
//!   protected path, the deterministic layer returns Block with no Jev call, regardless of
//!   the flag (R2.1, R2.3, R2.4).
//! - Property 35: deterministic secret block. For all changes whose content matches the
//!   secret denylist, the deterministic layer returns Block and sends nothing to Jev (R2.2,
//!   R3.3).
//!
//! The deterministic layer takes no client, so it cannot make a Jev call by construction.
//! Each property drives a named [`FakeClient`] alongside the call and asserts a zero call
//! count, so the "no Jev call" guarantee reads explicitly in the test, matching the plan.

use proptest::prelude::*;

use super::super::client_fake::FakeClient;
use super::*;

/// A stable protected set: two directory prefixes and one file.
fn protected_set() -> Vec<String> {
    vec![
        "specs/".to_string(),
        ".github/workflows/".to_string(),
        "LICENSE".to_string(),
    ]
}

/// Report whether a decision is a Block naming the given violated check.
fn is_block_with_check(decision: &Option<GuardDecision>, check: &str) -> bool {
    matches!(
        decision,
        Some(GuardDecision::Block(packet)) if packet.violated_check == check
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 34: a write under a protected directory prefix always blocks, and the layer
    /// makes no Jev call, whatever the flag would be. The flag does not reach this layer;
    /// the boolean models both flag states to state the invariant plainly (R2.3, R2.4).
    #[test]
    fn p34_protected_dir_write_always_blocks(
        suffix in "[a-z0-9/_.-]{0,40}",
        clean_content in "[a-zA-Z0-9 ]{0,60}",
        _flag in any::<bool>(),
    ) {
        let fake = FakeClient::new();
        let change = ProposedChange {
            paths: vec![format!("specs/{suffix}")],
            content: clean_content,
        };

        let decision = deterministic_layer(&change, &protected_set());

        prop_assert!(
            is_block_with_check(&decision, "protected-path"),
            "a protected-path write blocks: {decision:?}"
        );
        prop_assert_eq!(fake.call_count(), 0, "the deterministic layer makes no Jev call");
    }

    /// Property 34: a write to a protected file entry always blocks.
    #[test]
    fn p34_protected_file_write_always_blocks(
        clean_content in "[a-zA-Z0-9 ]{0,60}",
        _flag in any::<bool>(),
    ) {
        let fake = FakeClient::new();
        let change = ProposedChange {
            paths: vec!["LICENSE".to_string()],
            content: clean_content,
        };

        let decision = deterministic_layer(&change, &protected_set());

        prop_assert!(is_block_with_check(&decision, "protected-path"));
        prop_assert_eq!(fake.call_count(), 0);
    }

    /// Property 35: content that carries a secret marker always blocks, and the layer sends
    /// nothing to Jev. The generated content embeds a denylist marker, so the scan matches.
    #[test]
    fn p35_secret_content_always_blocks(
        prefix in "[a-zA-Z0-9 ]{0,30}",
        suffix in "[a-zA-Z0-9 ]{0,30}",
        marker in prop::sample::select(vec!["secret", "credentials"]),
    ) {
        let fake = FakeClient::new();
        // The written path is not protected, so the secret scan, not the path match, blocks.
        let change = ProposedChange {
            paths: vec!["src/plain.rs".to_string()],
            content: format!("{prefix} {marker} {suffix}"),
        };

        let decision = deterministic_layer(&change, &protected_set());

        prop_assert!(
            is_block_with_check(&decision, "secret"),
            "secret content blocks: {decision:?}"
        );
        // The packet names a marker, never the generated content (R6.3, P40).
        if let Some(GuardDecision::Block(packet)) = &decision {
            let offending = packet.offending_value.clone().unwrap_or_default();
            prop_assert!(
                offending == "secret-marker" || offending == "credentials-marker",
                "the offending value names a marker: {offending}"
            );
        }
        prop_assert_eq!(fake.call_count(), 0, "the deterministic layer sends nothing to Jev");
    }

    /// A change with no protected path and no secret content is never blocked by the
    /// deterministic layer, so it passes to the probabilistic layer (issue #294). The
    /// generated content avoids the denylist markers and the path avoids the protected set.
    #[test]
    fn a_clean_change_is_not_blocked(name in "[a-z]{1,20}") {
        let fake = FakeClient::new();
        let change = ProposedChange {
            paths: vec![format!("src/{name}.rs")],
            content: format!("fn {name}() {{}}"),
        };

        let decision = deterministic_layer(&change, &protected_set());

        prop_assert!(decision.is_none(), "a clean change is not blocked: {decision:?}");
        prop_assert_eq!(fake.call_count(), 0);
    }
}
