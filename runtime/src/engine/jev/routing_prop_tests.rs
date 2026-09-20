//! Property tests for the routing aspect.
//!
//! Included from `routing.rs` via `#[path]`, so `super` is the `routing` module.
//!
//! Feature: jev-integration-eval, Property 28: routing option closure. A chosen option inside
//! the target set (or the decline option) maps to a target that equals the choice and is a member
//! of the set-plus-decline. A chosen option outside the set returns [`JevError::UnexpectedOption`]
//! and no outcome (R3.2, R3.3, R3.4). The property drives the pure `classify_choice` helper, so it
//! needs no async runtime. The async path is covered by `routing_tests.rs`.

use proptest::prelude::*;

use super::*;

/// Report whether an option is a valid routing choice, for the property assertions.
///
/// This mirrors the private closure check: a valid option is a [`ROUTING_TARGETS`] member or the
/// decline option `NONE`.
fn is_member(option: &str) -> bool {
    option == "NONE" || ROUTING_TARGETS.contains(&option)
}

/// A strategy over the chosen option: sometimes a real member or decline, sometimes a random
/// string that can land outside the set.
fn choice_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // A real target or the decline option.
        (0usize..=ROUTING_TARGETS.len()).prop_map(|index| {
            if index == ROUTING_TARGETS.len() {
                "NONE".to_string()
            } else {
                ROUTING_TARGETS[index].to_string()
            }
        }),
        // A free string that is usually outside the set.
        "[a-z_]{1,20}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// An inside option maps to itself and a member; an outside option is UnexpectedOption.
    #[test]
    fn option_closure_holds(
        choice in choice_strategy(),
        confidence in 0.0f64..=1.0,
        low in 0.0f64..=1.0,
        high in 0.0f64..=1.0,
    ) {
        // Keep the threshold pair valid, so the banding step never masks the closure result.
        prop_assume!(low <= high);

        let result = classify_choice(&choice, confidence, low, high);

        if is_member(&choice) {
            let outcome = result.expect("a member classifies to Ok");
            prop_assert_eq!(&outcome.target, &choice, "the target equals the choice");
            prop_assert!(is_member(&outcome.target), "the target is a member of the set");
        } else {
            match result {
                Err(JevError::UnexpectedOption { option, .. }) => {
                    prop_assert_eq!(option, choice, "the error names the offending option");
                }
                other => prop_assert!(false, "expected UnexpectedOption, got {:?}", other),
            }
        }
    }
}
