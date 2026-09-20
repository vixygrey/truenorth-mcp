//! Property tests for the self-healing aspect.
//!
//! Included from `self_heal.rs` via `#[path]`, so `super` is the `self_heal` module.
//!
//! Feature: jev-integration-eval.
//! Property 29 (option closure): a chosen option inside the fixed set maps to Ok with an
//! instruction; a chosen option outside the set returns [`JevError::UnexpectedOption`] and no
//! instruction (R7.2, R7.3).
//! Property 30 (low-confidence REVERT safety): a REVERT option below the destructive threshold
//! never reverts. The decision is [`SelfHeal::AskHuman`] (R7.5).
//!
//! Both properties drive the pure `decide_self_heal` helper, so they need no async runtime. The
//! async path is covered by `self_heal_tests.rs`.

use proptest::prelude::*;

use super::*;

/// Report whether an option is inside the fixed self-heal set, for the property assertions.
///
/// This mirrors [`parse_option`]: an inside option parses to a variant, an outside option does
/// not.
fn is_member(option: &str) -> bool {
    parse_option(option).is_some()
}

/// A strategy over the chosen option: sometimes a fixed option, sometimes a random string that
/// can land outside the set.
fn choice_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // A real fixed option.
        prop_oneof![
            Just("REVERT".to_string()),
            Just("REFACTOR_IMPORTS".to_string()),
            Just("SIMPLIFY_LOGIC".to_string()),
            Just("ASK_HUMAN".to_string()),
        ],
        // A free string that is usually outside the set.
        "[A-Z_]{1,20}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 29: an inside option yields Ok with an instruction; an outside option is
    /// UnexpectedOption with no instruction.
    #[test]
    fn option_closure_holds(
        choice in choice_strategy(),
        confidence in 0.0f64..=1.0,
        low in 0.0f64..=1.0,
        destructive in 0.0f64..=1.0,
    ) {
        let result = decide_self_heal(&choice, confidence, low, destructive);

        if is_member(&choice) {
            prop_assert!(result.is_ok(), "a member yields Ok");
        } else {
            match result {
                Err(JevError::UnexpectedOption { option, .. }) => {
                    prop_assert_eq!(option, choice, "the error names the offending option");
                }
                other => prop_assert!(false, "expected UnexpectedOption, got {:?}", other),
            }
        }
    }

    /// Property 30: a REVERT below the destructive threshold never reverts; it asks a human.
    ///
    /// The confidence lands in `[low, destructive)`, so it clears the low-confidence override
    /// and triggers the destructive-revert override. Either override yields AskHuman, so the
    /// property holds even where the ranges touch.
    #[test]
    fn low_confidence_revert_never_reverts(
        low in 0.0f64..=1.0,
        gap in 0.0f64..=1.0,
        position in 0.0f64..1.0,
    ) {
        // Build low <= destructive within 0 to 1, then a confidence in [low, destructive).
        let destructive = low + gap * (1.0 - low);
        prop_assume!(destructive > low);
        let confidence = low + position * (destructive - low);

        let decision = decide_self_heal("REVERT", confidence, low, destructive)
            .expect("REVERT is a member, so the decision is Ok");

        prop_assert_eq!(decision.instruction, SelfHeal::AskHuman, "a low-confidence revert asks a human");
        prop_assert_ne!(decision.instruction, SelfHeal::Revert, "the decision never reverts");
    }
}
