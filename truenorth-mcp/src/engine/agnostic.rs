//! Model and harness agnosticism (design Overview shift 3).
//!
//! The runtime delivers identical content to every client. It never branches on the
//! requesting model family or harness, so a request from any model or harness receives
//! the same payload (Requirements 11.2, 11.3). Because the server never identifies the
//! client, every response carries the [`ADAPTATION_NOTE`] stating that no model-specific
//! or harness-specific adaptation was applied (Requirement 11.5).
//!
//! `has_vendor_scaffolding` detects Anthropic-style XML wrappers and vendor-directed
//! meta-instructions, so a test can assert an emitted payload carries none
//! (Requirement 11.1). The gate runner and the ontology scanner are language-agnostic by
//! construction, with no per-language configuration (Requirement 11.4).
//!
//! Requirements: 11.1, 11.2, 11.3, 11.4, 11.5. Design: Overview, Part II §4, §5, §6.

// This module is consumed by the skills tool and the agnosticism tests (task 16). It is
// unused until those wire it, so the module-scoped allow prevents a premature dead-code
// error under `clippy -D warnings`. Remove this allow once task 16 wires the consumer.
#![allow(dead_code)]

use std::sync::OnceLock;

use regex::Regex;

/// The note appended to emitted content indicating no client adaptation (Requirement
/// 11.5).
///
/// The server does not identify the requesting model family or harness, so no adaptation
/// is ever applied. The note makes that explicit in the response.
pub const ADAPTATION_NOTE: &str = "No model-specific or harness-specific adaptation was applied. This content is \
     identical for every model family and harness.";

/// Report whether text carries vendor-specific scaffolding (Requirement 11.1).
///
/// The set covers Anthropic-style XML wrapper tags (for example `<thinking>`) and
/// vendor-directed meta-instructions that name a specific model vendor (for example
/// "you are Claude" or "as ChatGPT"). A payload that matches is not agnostic.
pub fn has_vendor_scaffolding(text: &str) -> bool {
    vendor_pattern().is_match(text)
}

/// The vendor-scaffolding pattern.
fn vendor_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // Anthropic-style XML wrappers, and "you are <vendor>" / "as <vendor>" meta
        // directed at a named model vendor. Case-insensitive. Every branch is fixed and
        // compiles at build time.
        Regex::new(
            r"(?i)(</?thinking>|</?reasoning>|</?scratchpad>|</?antml|\b(you are|as)\s+(claude|chatgpt|gpt-?4|gpt-?5|gemini|deepseek|llama|copilot)\b)",
        )
        .expect("vendor scaffolding pattern must compile")
    })
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `agnostic`.
#[cfg(test)]
#[path = "agnostic_tests.rs"]
mod tests;
