//! Property tests for the pruning aspect.
//!
//! Included from `pruning.rs` via `#[path]`, so `super` is the `pruning` module.
//!
//! Feature: jev-integration-eval, Property 26 (line conservation) and Property 27 (order
//! preservation). Both properties drive the pure `select_kept` helper, so they need no async
//! runtime. Property 26: every input line is either kept or dropped, so
//! `kept_count + dropped_count == input_count` and `input_count == lines.len()` (R6.6, R6.7).
//! Property 27: the kept vector is an in-order subsequence of the input, equal to the input
//! filtered by `score >= threshold` in order (R6.4).

use proptest::prelude::*;

use super::*;

/// A strategy over a line list paired with a matching score list of the same length.
///
/// Each line is a short string, and each score is a value from 0 to 1, so the pair drives the
/// keep-or-drop step across the full threshold range.
fn lines_and_scores() -> impl Strategy<Value = (Vec<String>, Vec<f64>)> {
    prop::collection::vec(("[a-z ]{0,20}", 0.0f64..=1.0), 0..40).prop_map(|pairs| {
        let lines = pairs.iter().map(|(line, _)| line.clone()).collect();
        let scores = pairs.iter().map(|(_, score)| *score).collect();
        (lines, scores)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 26: every input line is either kept or dropped, and the input count matches.
    #[test]
    fn line_conservation_holds(
        (lines, scores) in lines_and_scores(),
        keep_threshold in 0.0f64..=1.0,
    ) {
        let outcome = select_kept(&lines, &scores, keep_threshold);
        prop_assert_eq!(
            outcome.kept_count + outcome.dropped_count,
            outcome.input_count,
            "kept plus dropped equals the input count"
        );
        prop_assert_eq!(outcome.input_count, lines.len(), "the input count is the line count");
        prop_assert_eq!(outcome.kept_count, outcome.kept.len(), "the kept count is the kept length");
    }

    /// Property 27: the kept vector is the input filtered by `score >= threshold` in order.
    #[test]
    fn order_preservation_holds(
        (lines, scores) in lines_and_scores(),
        keep_threshold in 0.0f64..=1.0,
    ) {
        let outcome = select_kept(&lines, &scores, keep_threshold);
        let expected: Vec<String> = lines
            .iter()
            .enumerate()
            .filter(|(index, _)| scores.get(*index).is_some_and(|score| *score >= keep_threshold))
            .map(|(_, line)| line.clone())
            .collect();
        prop_assert_eq!(outcome.kept, expected, "the kept vector is the in-order filtered input");
    }
}
