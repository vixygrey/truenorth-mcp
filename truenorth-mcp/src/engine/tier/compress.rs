//! The lean-tier compression passes.
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

use super::tables::compact_tables;
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
    let plain = strip_decoration(&compact);
    let bulleted = headings_to_bullets(&plain);
    let deduped = dedupe_directives(&bulleted);
    truncate_to_budget(&deduped, TIER_LEAN_TOKEN_BUDGET)
}

/// Strip decorative formatting from lean markdown (#62).
///
/// The pass drops a standalone horizontal rule (a line of only `-`, `*`, or `_` of length
/// three or more) and trims trailing whitespace from every line. A horizontal rule is pure
/// decoration for a lean agent. A heading, a checkbox, or any line that encodes an
/// invariant is never a horizontal rule, so this pass cannot remove a load-bearing line.
fn strip_decoration(md: &str) -> String {
    // The leading YAML frontmatter block is delimited by `---` fences that look like
    // horizontal rules. Keep every line before the first heading untouched, so the
    // frontmatter fences survive. A body horizontal rule after the first heading is
    // decoration and is dropped.
    let first_heading = md
        .lines()
        .position(|line| heading_level(line).is_some())
        .unwrap_or(0);

    let out: Vec<&str> = md
        .lines()
        .enumerate()
        .filter(|(index, line)| *index < first_heading || !is_horizontal_rule(line))
        .map(|(_, line)| line.trim_end())
        .collect();
    collapse_blank_runs(&out.join("\n"))
}

/// Report whether a line is a standalone horizontal rule (`---`, `***`, or `___`).
fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let first = trimmed.chars().next().expect("length checked above");
    matches!(first, '-' | '*' | '_') && trimmed.chars().all(|c| c == first)
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
