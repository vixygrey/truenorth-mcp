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
//! The lean passes live in the `compress` submodule. The shared helpers
//! `encodes_invariant_or_ac`, `heading_level`, and `collapse_blank_runs` stay here,
//! because both the reasoning and lean tiers use them.
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

mod compress;

pub use compress::compress_for_local_context;

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

/// Report whether a line encodes an invariant or an acceptance criterion.
///
/// These lines are load-bearing and must survive every non-full tier (Property 4). The
/// set covers headings, checkbox items, gate and invariant markers, and the `verify:`
/// command line.
///
/// Visible to the `compress` submodule, which applies the same preservation rule inside
/// its section-drop and table passes.
pub(super) fn encodes_invariant_or_ac(line: &str) -> bool {
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

/// The heading level of a line (1 for `#`, 2 for `##`, ...), or `None` when not a
/// heading.
///
/// Visible to the `compress` submodule, which walks sections by heading level.
pub(super) fn heading_level(line: &str) -> Option<usize> {
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

/// Collapse a run of two or more blank lines to a single blank line, and trim leading and
/// trailing blank lines.
///
/// Visible to the `compress` submodule, which calls it after each lean pass.
pub(super) fn collapse_blank_runs(text: &str) -> String {
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
#[path = "mod_tests.rs"]
mod tests;
