//! The lean-tier table-compaction pass (#62).
//!
//! `compact_tables` rewrites markdown tables to a leaner form. Tables appear in 45 of the
//! 80 skills, so this is the main compression lever. A table drops its separator row and
//! strips cell padding. A two-column table reflows each body row to `- left -> right`. A
//! row that encodes an invariant or an acceptance criterion is emitted verbatim, so the
//! pass never rewrites a load-bearing rule (Property 4).
//!
//! Requirements: 6.8. Design: Part II §4. Issue: #62.

use super::{collapse_blank_runs, encodes_invariant_or_ac};

/// Compact markdown tables in the lean tier (#62).
///
/// A table is a run of consecutive lines that each trim to start and end with `|`, where
/// the run contains a separator row (a line of only pipes, dashes, colons, and spaces). A
/// run without a separator row is not a table and passes through unchanged.
///
/// Every table drops its separator row and strips cell padding to single spaces. A
/// two-column table additionally reflows each body row to `- left -> right` and drops the
/// header row, which is usually `Condition | Verdict` scaffolding. A table of three or
/// more columns stays tabular. A row that encodes an invariant or an acceptance criterion
/// is emitted verbatim, so the pass never rewrites a load-bearing rule (Property 4).
pub(super) fn compact_tables(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let run_end = table_run_end(&lines, i);
        match run_end {
            Some(end) => {
                out.extend(compact_one_table(&lines[i..end]));
                i = end;
            }
            None => {
                out.push(lines[i].to_string());
                i += 1;
            }
        }
    }

    collapse_blank_runs(&out.join("\n"))
}

/// Return the exclusive end index of a table run starting at `start`, or `None` when the
/// run at `start` is not a table.
///
/// A table run is a maximal block of consecutive table-shaped lines that includes at
/// least one separator row.
fn table_run_end(lines: &[&str], start: usize) -> Option<usize> {
    if !is_table_line(lines[start]) {
        return None;
    }
    let mut end = start;
    let mut has_separator = false;
    while end < lines.len() && is_table_line(lines[end]) {
        if is_separator_row(lines[end]) {
            has_separator = true;
        }
        end += 1;
    }
    has_separator.then_some(end)
}

/// Report whether a trimmed line is table-shaped (starts and ends with `|`).
fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 2 && trimmed.starts_with('|') && trimmed.ends_with('|')
}

/// Report whether a table line is a separator row (only pipes, dashes, colons, spaces).
fn is_separator_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|')
        && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
        && trimmed.contains('-')
}

/// Compact a single table run to its lean form.
fn compact_one_table(rows: &[&str]) -> Vec<String> {
    // Split the header (rows before the separator) from the body (rows after it).
    let sep_index = rows.iter().position(|r| is_separator_row(r));
    let header_count = sep_index.unwrap_or(0);
    let column_count = sep_index
        .map(|idx| cell_count(rows[idx]))
        .unwrap_or_else(|| rows.first().map(|r| cell_count(r)).unwrap_or(0));

    let mut out: Vec<String> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if is_separator_row(row) {
            // The separator row is decoration. Drop it.
            continue;
        }
        // A load-bearing row is emitted verbatim (Property 4).
        if encodes_invariant_or_ac(row) {
            out.push((*row).to_string());
            continue;
        }
        let is_header = index < header_count;
        if column_count == 2 {
            // Drop the two-column header; reflow each body row to `- left -> right`.
            if is_header {
                continue;
            }
            let cells = split_cells(row);
            if cells.len() == 2 {
                out.push(format!("- {} -> {}", cells[0], cells[1]));
            } else {
                // A cell-count mismatch: keep it tabular, do not fabricate a mapping.
                out.push(strip_table_padding(row));
            }
        } else {
            out.push(strip_table_padding(row));
        }
    }
    out
}

/// The number of cells in a table row.
fn cell_count(row: &str) -> usize {
    split_cells(row).len()
}

/// Split a table row into trimmed cell contents, on unescaped pipes.
///
/// The leading and trailing pipes are dropped. An escaped pipe (`\|`) stays inside the
/// cell, so a cell that contains a literal pipe is not split.
fn split_cells(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    let inner = trimmed
        .strip_prefix('|')
        .and_then(|s| s.strip_suffix('|'))
        .unwrap_or(trimmed);

    let mut cells: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_backslash = false;
    for c in inner.chars() {
        if c == '|' && !prev_backslash {
            cells.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
        prev_backslash = c == '\\';
    }
    cells.push(current.trim().to_string());
    cells
}

/// Rejoin a table row with single-space cell padding.
fn strip_table_padding(row: &str) -> String {
    let cells = split_cells(row);
    format!("| {} |", cells.join(" | "))
}

// Unit tests live in a sibling file to hold this module under the size guidance.
#[cfg(test)]
#[path = "tables_tests.rs"]
mod tests;
