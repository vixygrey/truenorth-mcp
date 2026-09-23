//! Tests for model and harness agnosticism (task 16.2).
//!
//! Included from `agnostic.rs` via `#[path]`, so `super` is the agnostic module.
//!
//! Requirements: 11.1, 11.2, 11.3, 11.5.

use super::*;
use crate::engine::tier::{TIER_LEAN_TOKEN_BUDGET, Tier, render_skill};

/// A skill with Anthropic-style scaffolding and vendor-directed meta, to strip.
const SCAFFOLDED_SKILL: &str = r#"# Skill

<thinking>
You are Claude. Think step by step.
</thinking>

## Workflow

- [ ] Do the work.

verify: `cargo test`
"#;

#[test]
fn detects_anthropic_xml_wrappers() {
    assert!(has_vendor_scaffolding("<thinking>reason here</thinking>"));
    assert!(has_vendor_scaffolding("some <reasoning> tag"));
}

#[test]
fn detects_vendor_directed_meta() {
    assert!(has_vendor_scaffolding("You are Claude, an AI assistant."));
    assert!(has_vendor_scaffolding("Respond as ChatGPT would."));
    assert!(has_vendor_scaffolding("as Gemini, summarize this."));
}

#[test]
fn plain_skill_content_has_no_scaffolding() {
    // Requirement 11.1: ordinary skill prose carries no vendor scaffolding.
    let plain = "# Develop TDD\n\n- [ ] Write a failing test.\n\nverify: `cargo test`\n";
    assert!(!has_vendor_scaffolding(plain));
}

#[test]
fn reasoning_tier_output_is_agnostic() {
    // Requirement 11.1: the reasoning tier strips the Anthropic XML wrapper, so the
    // emitted payload carries no vendor scaffolding.
    let out = render_skill(SCAFFOLDED_SKILL, Tier::Reasoning, TIER_LEAN_TOKEN_BUDGET);
    assert!(
        !has_vendor_scaffolding(&out),
        "reasoning payload still carries vendor scaffolding: {out}"
    );
}

#[test]
fn lean_tier_output_is_agnostic() {
    let out = render_skill(SCAFFOLDED_SKILL, Tier::Lean, TIER_LEAN_TOKEN_BUDGET);
    assert!(!has_vendor_scaffolding(&out));
}

#[test]
fn rendering_ignores_any_client_notion() {
    // Requirements 11.2, 11.3: rendering is a pure function of the markdown and tier. It
    // takes no client identity, so the output is identical for every model and harness.
    // Rendering the same input twice yields identical output, which stands in for
    // rendering it for two different clients.
    let a = render_skill(SCAFFOLDED_SKILL, Tier::Lean, TIER_LEAN_TOKEN_BUDGET);
    let b = render_skill(SCAFFOLDED_SKILL, Tier::Lean, TIER_LEAN_TOKEN_BUDGET);
    assert_eq!(a, b);
}

#[test]
fn adaptation_note_states_no_adaptation() {
    // Requirement 11.5: the note makes the no-adaptation contract explicit.
    assert!(ADAPTATION_NOTE.contains("No model-specific or harness-specific adaptation"));
    // The note itself carries no vendor scaffolding.
    assert!(!has_vendor_scaffolding(ADAPTATION_NOTE));
}
