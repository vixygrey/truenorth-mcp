//! Tier transforms for skill payloads.
//!
//! A skill renders at one of three tiers (design §4):
//!
//! - `full`: the on-disk markdown, byte-for-byte (Requirement 6.6).
//! - `reasoning`: `strip_meta_steps`, which removes meta and guardrail scaffolding while
//!   keeping every heading and every invariant or acceptance-criterion line
//!   (Requirement 6.7).
//! - `lean`: `compress_for_local_context` over the reasoning output, which additionally
//!   drops rationale, background, and verbose-example section bodies, converts headings
//!   to imperative bullets, dedupes directives, and truncates to the lean token budget,
//!   while keeping every invariant or acceptance-criterion line (Requirement 6.8).
//!
//! Both transforms are pure functions of the input markdown (design §4). Property 4
//! holds: `render_skill(md, Full) == md`, and the reasoning and lean tiers never remove
//! a line that encodes an invariant or an acceptance criterion.
//!
//! Requirements: 6.6, 6.7, 6.8. Design: Part II §4.

// The tier transforms are consumed by the `get_skill` tool (task 8). They are unused
// until that task lands, so the module-scoped allow prevents a premature dead-code error
// under `clippy -D warnings`. Remove this allow once task 8 wires the consumer.
#![allow(dead_code)]

use std::sync::OnceLock;

use regex::Regex;

/// The lean token budget. The lean transform truncates its output to at most this many
/// tokens (design §4). A token is approximated as a whitespace-separated word, which is
/// a conservative over-count for English prose and code directives.
pub const TIER_LEAN_TOKEN_BUDGET: usize = 1500;

/// A skill rendering tier (design §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The on-disk markdown, unchanged.
    Full,
    /// Meta and guardrail scaffolding removed.
    Reasoning,
    /// Compressed to imperative directives within the lean token budget.
    Lean,
}

/// Render a skill's markdown at the given tier (design §4).
///
/// `Full` is the identity transform (Requirement 6.6). `Reasoning` and `Lean` preserve
/// every invariant and acceptance-criterion line (Property 4).
///
/// # Example
///
/// ```ignore
/// let out = tier::render_skill(md, tier::Tier::Reasoning);
/// ```
pub fn render_skill(md: &str, tier: Tier) -> String {
    match tier {
        Tier::Full => md.to_string(),
        Tier::Reasoning => strip_meta_steps(md),
        Tier::Lean => compress_for_local_context(&strip_meta_steps(md)),
    }
}

/// Remove meta and guardrail scaffolding from skill markdown (reasoning tier).
///
/// The transform drops lines that are pure reasoning scaffolding (for example
/// "think step by step") and Anthropic-style XML wrapper tags (for example `<thinking>`),
/// and collapses a repeated guardrail line to its first occurrence. It keeps every
/// heading and every line that encodes an invariant or an acceptance criterion
/// (Requirement 6.7).
pub fn strip_meta_steps(md: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut seen_guardrails: Vec<String> = Vec::new();

    for line in md.lines() {
        // An invariant or acceptance-criterion line is always kept, even when it also
        // looks like a guardrail. Preservation wins over removal (Property 4).
        if encodes_invariant_or_ac(line) {
            out.push(line);
            continue;
        }
        if is_meta_line(line) {
            continue;
        }
        if is_duplicate_guardrail(line, &mut seen_guardrails) {
            continue;
        }
        out.push(line);
    }

    collapse_blank_runs(&out.join("\n"))
}

/// Compress reasoning-tier markdown to imperative directives within the token budget
/// (lean tier).
///
/// The transform drops the bodies of rationale, background, and verbose-example sections,
/// converts headings to imperative bullets, dedupes repeated directive lines, and
/// truncates to [`TIER_LEAN_TOKEN_BUDGET`] tokens. It keeps every line that encodes an
/// invariant or an acceptance criterion (Requirement 6.8).
pub fn compress_for_local_context(md: &str) -> String {
    let without_prose = drop_low_value_sections(md);
    let bulleted = headings_to_bullets(&without_prose);
    let deduped = dedupe_directives(&bulleted);
    truncate_to_budget(&deduped, TIER_LEAN_TOKEN_BUDGET)
}

/// Report whether a line encodes an invariant or an acceptance criterion.
///
/// These lines are load-bearing and must survive every non-full tier (Property 4). The
/// set covers headings, checkbox items, gate and invariant markers, and the `verify:`
/// command line.
fn encodes_invariant_or_ac(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Headings are always retained (Requirement 6.7).
    if trimmed.starts_with('#') {
        return true;
    }
    // Checkbox items are acceptance criteria.
    if trimmed.starts_with("- [ ]") || trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]")
    {
        return true;
    }
    // Gate, invariant, and requirement markers carry load-bearing rules.
    marker_pattern().is_match(trimmed)
}

/// The invariant and acceptance-criterion marker pattern.
fn marker_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // Case-sensitive markers: uppercase GATE / MUST / MUST NOT / NEVER / ALWAYS /
        // INVARIANT signal a rule. The lowercase `verify:` line is the skill's gate
        // command. Every pattern is fixed and compiles at build time.
        Regex::new(r"(HARD GATE|GATE|MUST NOT|MUST|NEVER|ALWAYS|INVARIANT|REQUIRED|\bverify:)")
            .expect("marker pattern must compile")
    })
}

/// Report whether a line is meta or guardrail scaffolding to strip (reasoning tier).
///
/// The set covers Anthropic-style XML wrapper tags and chain-of-thought priming phrases.
fn is_meta_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    meta_pattern().is_match(trimmed)
}

/// The meta and guardrail scaffolding pattern.
fn meta_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // Anthropic-style XML wrappers and chain-of-thought priming. Case-insensitive.
        // Every pattern is fixed and compiles at build time.
        Regex::new(
            r"(?i)(</?thinking>|</?scratchpad>|</?reasoning>|think step by step|let'?s (?:think|reason)|take a deep breath)",
        )
        .expect("meta pattern must compile")
    })
}

/// Report whether a non-heading guardrail line repeats one already emitted.
///
/// A guardrail line is a blockquote callout (starts with `>`). The first occurrence is
/// kept. A later identical occurrence is dropped, so a restated guardrail collapses.
fn is_duplicate_guardrail(line: &str, seen: &mut Vec<String>) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('>') {
        return false;
    }
    let normalized = trimmed.to_string();
    if seen.contains(&normalized) {
        return true;
    }
    seen.push(normalized);
    false
}

/// Section titles whose bodies the lean tier drops as rationale or background.
const LOW_VALUE_SECTION_TITLES: [&str; 5] = [
    "rationale",
    "background",
    "philosophy",
    "examples (verbose)",
    "red flags",
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

/// The heading level of a line (1 for `#`, 2 for `##`, ...), or `None` when not a
/// heading.
fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    // A valid ATX heading has a space after the hashes.
    if trimmed[hashes..].starts_with(' ') {
        Some(hashes)
    } else {
        None
    }
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

/// Collapse a run of two or more blank lines to a single blank line, and trim leading and
/// trailing blank lines.
fn collapse_blank_runs(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut prev_blank = false;

    for line in text.lines() {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        out.push(line);
        prev_blank = blank;
    }

    // Trim leading and trailing blank lines.
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }

    out.join("\n")
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `tier`.
#[cfg(test)]
#[path = "tier_tests.rs"]
mod tests;
