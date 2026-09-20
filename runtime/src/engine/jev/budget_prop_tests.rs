//! Property test for the token-budget bound.
//!
//! Included from `mod.rs` via `#[path]`, so `super` is the `jev` module.
//!
//! Feature: jev-integration-eval, Property 33: token-budget bound. The pre-call guard
//! rejects exactly the over-budget requests before any call. For a state string of any
//! length, the guard is `Ok` when the estimate is at or under the budget, and it is
//! `Err(BudgetExceeded)` when the estimate is over. A rejection reports the same estimate
//! that [`estimate_tokens`] returns and the fixed [`TOKEN_BUDGET`] (Requirement 4.10).

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::*;

/// Generate a state string that ranges from empty to well over the budget.
///
/// The small arm covers the common under-budget case. The large arm reaches past four
/// times the budget, so the generator exercises both sides of the boundary.
fn state_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::collection::vec(any::<char>(), 0..64).prop_map(|chars| chars.into_iter().collect()),
        (0usize..(TOKEN_BUDGET * BYTES_PER_TOKEN * 2)).prop_map(|len| "z".repeat(len)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The guard accepts a request at or under the budget and rejects one over it, and a
    /// rejection reports the exact estimate and the fixed budget.
    #[test]
    fn guard_rejects_exactly_the_over_budget_requests(state in state_strategy()) {
        let request = JevRequest::new(serde_json::json!(state), BTreeMap::new());
        let estimate = estimate_tokens(&request);
        let result = guard_budget(&request);

        if estimate <= TOKEN_BUDGET {
            prop_assert!(result.is_ok(), "an at-or-under-budget request is accepted");
        } else {
            match result {
                Err(JevError::BudgetExceeded { estimate: reported, budget }) => {
                    prop_assert_eq!(reported, estimate);
                    prop_assert_eq!(budget, TOKEN_BUDGET);
                }
                other => prop_assert!(false, "expected BudgetExceeded, got {:?}", other),
            }
        }
    }
}
