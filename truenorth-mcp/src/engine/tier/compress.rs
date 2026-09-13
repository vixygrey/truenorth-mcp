//! The lean-tier compression passes.
//!
//! `compress_for_local_context` runs over the reasoning-tier output. It drops the bodies
//! of low-value sections, converts headings to imperative bullets, dedupes repeated
//! directives, and truncates to the lean token budget, while keeping every line that
//! encodes an invariant or an acceptance criterion (Requirement 6.8, Property 4).
//!
//! The shared helpers `encodes_invariant_or_ac`, `heading_level`, and
//! `collapse_blank_runs` live in the parent `tier` module, because both the reasoning
//! and lean tiers use them.
//!
//! Requirements: 6.8. Design: Part II §4. Issue: #62.

use std::sync::OnceLock;

use regex::Regex;

use super::{TIER_LEAN_TOKEN_BUDGET, collapse_blank_runs, encodes_invariant_or_ac, heading_level};

/// Compress reasoning-tier markdown to imperative directives within the token budget
/// (lean tier).
///
/// The transform drops the bodies of rationale, background, and verbose-example sections,
/// converts headings to imperative bullets, dedupes repeated directive lines, and
/// truncates to [`TIER_LEAN_TOKEN_BUDGET`] tokens. It keeps every line that encodes an
/// invariant or an acceptance criterion (Requirement 6.8).
pub fn compress_for_local_context(md: &str) -> String {
    let without_prose = drop_low_value_sections(md);
    let compact = compact_tables(&without_prose);
    let bulleted = headings_to_bullets(&compact);
    let deduped = dedupe_directives(&bulleted);
    truncate_to_budget(&deduped, TIER_LEAN_TOKEN_BUDGET)
}

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
fn compact_tables(md: &str) -> String {
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

/// Section titles whose bodies the lean tier drops.
///
/// The first five are rationale or background prose. The last three are structural
/// sections a lean acting agent does not execute on: citations, scope exclusions, and the
/// handoff prose. The Handoff wiring lines survive regardless, because
/// `encodes_invariant_or_ac` treats `Next:`, `Writes:`, and `Gate:` as load-bearing
/// (#62). `integration points` and `notes` stay intact, because they carry
/// action-relevant prose with no reliable line shape to protect.
const LOW_VALUE_SECTION_TITLES: [&str; 8] = [
    "rationale",
    "background",
    "philosophy",
    "examples (verbose)",
    "red flags",
    "references",
    "out of scope",
    "handoff",
];

/// Drop the bodies of rationale, background, and verbose-example sections (lean tier).
///
/// A section runs from a heading to the next heading of the same or higher level. The
/// heading itself is kept, so the document structure survives. Any line inside the
/// section that encodes an invariant or acceptance criterion is kept, so a dropped
/// section never removes a load-bearing rule (Property 4).
fn drop_low_value_sections(md: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut dropping_at_level: Option<usize> = None;

    for line in md.lines() {
        if let Some(level) = heading_level(line) {
            match dropping_at_level {
                // A heading at the same or higher level ends the dropped section.
                Some(active) if level <= active => dropping_at_level = None,
                _ => {}
            }
            if dropping_at_level.is_none() && is_low_value_heading(line) {
                dropping_at_level = Some(level);
            }
            // The heading line itself is always kept.
            out.push(line);
            continue;
        }

        if dropping_at_level.is_some() {
            // Inside a dropped section, keep only load-bearing lines.
            if encodes_invariant_or_ac(line) {
                out.push(line);
            }
            continue;
        }

        out.push(line);
    }

    collapse_blank_runs(&out.join("\n"))
}

/// Report whether a heading's title names a low-value section (case-insensitive).
fn is_low_value_heading(line: &str) -> bool {
    let title = line
        .trim_start()
        .trim_start_matches('#')
        .trim()
        .to_lowercase();
    LOW_VALUE_SECTION_TITLES.iter().any(|low| title == *low)
}

/// Convert headings to imperative bullets (lean tier).
///
/// A heading such as `## Step 1: Do X` becomes `- Do X`. A numeric or `Step N:` prefix is
/// stripped, so the bullet reads as a directive. Non-heading lines pass through.
fn headings_to_bullets(md: &str) -> String {
    let out: Vec<String> = md
        .lines()
        .map(|line| match heading_level(line) {
            Some(_) => {
                let title = line.trim_start().trim_start_matches('#').trim();
                format!("- {}", strip_step_prefix(title))
            }
            None => line.to_string(),
        })
        .collect();
    out.join("\n")
}

/// Strip a leading `Step N:` or `N.` ordinal prefix from a heading title.
fn strip_step_prefix(title: &str) -> &str {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        // A leading step or numeric ordinal, for example `Step 1:` or `1.` or `1)`.
        Regex::new(r"(?i)^(step\s+\d+\s*:\s*|\d+\s*[.)]\s*)").expect("step prefix must compile")
    });
    match pattern.find(title) {
        Some(m) => &title[m.end()..],
        None => title,
    }
}

/// Remove a repeated directive line, keeping its first occurrence (lean tier).
///
/// A line that encodes an invariant or acceptance criterion is never deduped away, so a
/// repeated gate line survives every occurrence (Property 4). Blank lines are not
/// deduped, so paragraph spacing is left to the blank-run collapse.
fn dedupe_directives(md: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || encodes_invariant_or_ac(line) {
            out.push(line);
            continue;
        }
        if seen.contains(&trimmed) {
            continue;
        }
        seen.push(trimmed);
        out.push(line);
    }

    collapse_blank_runs(&out.join("\n"))
}

/// Truncate text to at most `budget` whitespace-separated tokens (lean tier).
///
/// The truncation keeps whole lines. It stops adding lines once the running token count
/// would exceed the budget. A single line longer than the budget is kept whole, so a
/// load-bearing directive is never cut mid-line.
fn truncate_to_budget(md: &str, budget: usize) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut tokens = 0usize;

    for line in md.lines() {
        let line_tokens = line.split_whitespace().count();
        if tokens + line_tokens > budget && !out.is_empty() {
            break;
        }
        tokens += line_tokens;
        out.push(line);
    }

    collapse_blank_runs(&out.join("\n"))
}

// Unit tests for the lean passes live in a sibling file to hold this module under the
// size guidance. The `#[path]` include keeps them a child module of `compress`.
#[cfg(test)]
#[path = "compress_tests.rs"]
mod tests;
