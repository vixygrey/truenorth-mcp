//! Property test for the error non-panic property.
//!
//! Included from `client_http.rs` via `#[path]`, so `super` is the client_http module. The
//! test is not feature-gated, because the pure status mapper is always compiled. It runs
//! offline with no network call and no reqwest client.
//!
//! Feature: jev-integration-eval, Property 32: error non-panic. The pure status mapper is
//! total and panic-free. For any `u16` status, [`super::map_status`] returns a decision
//! without a panic, and every mapped status yields its documented variant (Requirement
//! 10.1, 10.2, 10.6, 10.7).

use proptest::prelude::*;

use super::{JEV_API_KEY_VAR, StatusAction, map_status};
use crate::engine::jev::JevError;

/// Assert one status maps to its documented action.
///
/// The mapper takes the shared [`JEV_API_KEY_VAR`], so the default test build has a caller
/// for the const and the mapper alike.
fn assert_mapping(status: u16) {
    let action = map_status(status, JEV_API_KEY_VAR, Some("field".to_string()));
    match status {
        200 => assert_eq!(action, StatusAction::Parse),
        401 => assert!(matches!(
            action,
            StatusAction::Terminal(JevError::Unauthorized { .. })
        )),
        422 => assert!(matches!(
            action,
            StatusAction::Terminal(JevError::Validation { .. })
        )),
        429 | 529 => assert_eq!(action, StatusAction::Retry),
        _ => assert!(matches!(
            action,
            StatusAction::Terminal(JevError::UnexpectedStatus { .. })
        )),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// For a status drawn from the mapped set plus arbitrary other `u16` values, the mapper
    /// returns without a panic and yields the documented variant.
    #[test]
    fn map_status_is_total_and_panic_free(
        status in prop_oneof![
            Just(200u16),
            Just(401u16),
            Just(422u16),
            Just(429u16),
            Just(529u16),
            any::<u16>(),
        ]
    ) {
        assert_mapping(status);
    }
}
