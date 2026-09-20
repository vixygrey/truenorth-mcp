//! Property test for the guard CLI exit code (task 6, issue #298).
//!
//! Included from `guard_cli.rs` via `#[path]`, so `super` is the `guard_cli` module.
//!
//! Feature: jev-active-guardrail, Property 41: hook exit code. For all guard decisions, the
//! CLI exits non-zero on a block and zero on an allow or an annotate (R7.2, R7.3). The mapping
//! is pure, so the test runs offline with no client and no runtime.

use proptest::prelude::*;

use super::super::guard::{GuardDecision, NeutralizationPacket};
use super::*;

/// Build an arbitrary guard decision from a discriminant and generated note text.
///
/// The three arms cover the three decision kinds: a block with a packet, an allow with notes,
/// and an annotate with notes. The generated text drives the note and packet fields.
fn decision_strategy() -> impl Strategy<Value = GuardDecision> {
    prop_oneof![
        "[a-zA-Z ]{0,40}".prop_map(|remediation| {
            GuardDecision::Block(NeutralizationPacket {
                violated_check: "drift".to_string(),
                offending_value: None,
                remediation,
                suggested_fix: None,
            })
        }),
        prop::collection::vec("[a-zA-Z ]{0,20}", 0..4)
            .prop_map(|notes| GuardDecision::Allow { notes }),
        prop::collection::vec("[a-zA-Z ]{0,20}", 0..4)
            .prop_map(|notes| GuardDecision::Annotate { notes }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 41: the exit code is non-zero exactly for a block, and zero otherwise.
    #[test]
    fn p41_exit_code_is_nonzero_exactly_for_a_block(decision in decision_strategy()) {
        let code = exit_code_for(&decision);
        let is_block = matches!(decision, GuardDecision::Block(_));
        if is_block {
            prop_assert_ne!(code, EXIT_ALLOW, "a block exits non-zero");
            prop_assert_eq!(code, EXIT_BLOCK);
        } else {
            prop_assert_eq!(code, EXIT_ALLOW, "an allow or an annotate exits zero");
        }
    }

    /// A block always carries a stderr message; an allow and an annotate carry none.
    #[test]
    fn a_block_carries_a_message_and_a_pass_carries_none(decision in decision_strategy()) {
        let message = block_message(&decision);
        let is_block = matches!(decision, GuardDecision::Block(_));
        prop_assert_eq!(message.is_some(), is_block, "only a block prints a message");
    }
}
