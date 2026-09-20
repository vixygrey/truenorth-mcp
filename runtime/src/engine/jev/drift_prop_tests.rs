//! Property tests for the drift aspect.
//!
//! Included from `drift.rs` via `#[path]`, so `super` is the `drift` module.
//!
//! Feature: jev-integration-eval, Property 25: drift boundary determinism. The boundary
//! decision is a pure function of the Noul value and the boundary, so two evaluations with
//! the same inputs return the same decision, and the decision equals `noul >= boundary`
//! (R5.4 to R5.6). The model-free literal path match is deterministic and catches exactly a
//! written path that equals a protected file or sits under a protected directory prefix
//! (R5.8).

use proptest::prelude::*;

use super::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The boundary decision is deterministic and equals `noul >= boundary`.
    #[test]
    fn boundary_decision_is_deterministic(noul in 0.0f64..=1.0, boundary in 0.0f64..=1.0) {
        let first = is_out_of_scope(noul, boundary);
        let second = is_out_of_scope(noul, boundary);
        prop_assert_eq!(first, second, "two evaluations agree");
        prop_assert_eq!(first, noul >= boundary, "the decision equals noul >= boundary");
    }

    /// A written path under a protected directory prefix is always caught.
    #[test]
    fn protected_dir_prefix_is_always_caught(suffix in "[a-z/]{0,40}") {
        let protected = vec!["specs/".to_string()];
        let written = format!("specs/{suffix}");
        prop_assert!(path_is_protected(&written, &protected));
    }

    /// A path that shares no protected entry is never caught, and the check is deterministic.
    #[test]
    fn unrelated_path_is_never_caught(name in "[a-z]{1,20}") {
        // The protected set names a directory and a file that the generated path avoids.
        let protected = vec!["specs/".to_string(), "LICENSE".to_string()];
        let written = format!("src/{name}.rs");
        let first = path_is_protected(&written, &protected);
        let second = path_is_protected(&written, &protected);
        prop_assert_eq!(first, second, "the check is deterministic");
        prop_assert!(!first, "an unrelated path is not protected");
    }

    /// A written path that equals a protected file entry is always caught.
    #[test]
    fn protected_file_exact_match_is_caught(name in "[a-z]{1,20}") {
        let protected = vec![name.clone()];
        prop_assert!(path_is_protected(&name, &protected));
    }
}
