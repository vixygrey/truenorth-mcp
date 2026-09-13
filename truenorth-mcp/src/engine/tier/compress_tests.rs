//! Unit tests for the lean compression passes (#62).
//!
//! Included from `compress.rs` via `#[path]`, so `super` is the `compress` module. These
//! tests exercise the internal passes directly, while the end-to-end tier properties live
//! in `tier/mod_tests.rs`.

use super::*;

#[test]
fn two_column_table_reflows_to_arrows() {
    // A two-column table drops its header and reflows each body row to `- left -> right`.
    let md = "\
| Condition | Verdict |
| --- | --- |
| tests fail | block the merge |
| lint clean | allow the merge |";
    let out = compact_tables(md);
    assert!(
        out.contains("- tests fail -> block the merge"),
        "got: {out}"
    );
    assert!(
        out.contains("- lint clean -> allow the merge"),
        "got: {out}"
    );
    // The header and separator are gone.
    assert!(!out.contains("Condition"), "header was not dropped: {out}");
    assert!(!out.contains("---"), "separator was not dropped: {out}");
}

#[test]
fn three_column_table_stays_tabular_with_padding_stripped() {
    // A table of three or more columns stays tabular. The separator drops and the cell
    // padding collapses to single spaces.
    let md = "\
| Skill        | Phase   | Note                |
| ------------ | ------- | ------------------- |
| plan-work    | design  | writes the plan     |";
    let out = compact_tables(md);
    assert!(
        out.contains("| plan-work | design | writes the plan |"),
        "padding not stripped: {out}"
    );
    assert!(!out.contains("------"), "separator was not dropped: {out}");
    // No arrow reflow for a three-column table.
    assert!(
        !out.contains("->"),
        "three-column table was reflowed: {out}"
    );
}

#[test]
fn invariant_row_is_emitted_verbatim() {
    // Property 4: a row that encodes a rule is not reflowed or repadded. It survives
    // byte-for-byte, so a load-bearing table row is never rewritten.
    let row = "| always | MUST run verify-work before the merge |";
    let md = format!(
        "\
| Condition | Rule |
| --- | --- |
{row}"
    );
    let out = compact_tables(&md);
    assert!(out.contains(row), "invariant row was rewritten: {out}");
}

#[test]
fn a_pipe_line_without_a_separator_is_not_a_table() {
    // A run of pipe-shaped lines with no separator row is not treated as a table, so it
    // passes through unchanged.
    let md = "| this is just | a line with pipes |\n| and another | one here |";
    let out = compact_tables(md);
    assert_eq!(out, md, "a non-table run was altered");
}

#[test]
fn escaped_pipe_stays_inside_the_cell() {
    // A cell that contains an escaped pipe is not split on it.
    let md = "\
| Left | Right |
| --- | --- |
| a \\| b | c |";
    let out = compact_tables(md);
    assert!(
        out.contains("a \\| b -> c"),
        "escaped pipe split the cell: {out}"
    );
}

#[test]
fn one_column_table_strips_padding_without_arrows() {
    let md = "\
| Item   |
| ------ |
| first  |";
    let out = compact_tables(md);
    assert!(out.contains("| first |"), "padding not stripped: {out}");
    assert!(!out.contains("->"), "one-column table was reflowed: {out}");
}

#[test]
fn row_with_cell_count_mismatch_stays_tabular() {
    // A body row whose cell count differs from the two-column header is not reflowed, so
    // the pass never fabricates a mapping. It is padding-stripped instead.
    let md = "\
| Left | Right |
| --- | --- |
| only-one-cell |";
    let out = compact_tables(md);
    assert!(out.contains("| only-one-cell |"), "got: {out}");
    assert!(
        !out.contains("only-one-cell ->"),
        "a mismatched row was reflowed: {out}"
    );
}

#[test]
fn prose_around_a_table_is_untouched() {
    let md = "\
Before the table.

| A | B |
| --- | --- |
| x | y |

After the table.";
    let out = compact_tables(md);
    assert!(out.contains("Before the table."));
    assert!(out.contains("After the table."));
    assert!(out.contains("- x -> y"), "got: {out}");
}
