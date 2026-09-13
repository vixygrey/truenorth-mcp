//! Property and golden tests for the tier transforms (task 4.2).
//!
//! Included from `tier/mod.rs` via `#[path]`, so `super` is the tier module.
//!
//! Property 4: `render_skill(md, Full) == md`. The reasoning and lean tiers retain every
//! invariant and acceptance-criterion line and remove only meta and guardrail
//! scaffolding, and, for lean, rationale, background, and verbose examples within budget.
//!
//! Requirements: 6.6, 6.7, 6.8.

use super::*;

/// A skill fixture with meta scaffolding, invariants, acceptance criteria, and a
/// rationale section, so each tier property is provable.
const SKILL_FIXTURE: &str = r#"# Develop TDD

<thinking>
Let's think step by step about the approach here.
</thinking>

> **HARD GATE** — Do NOT proceed on `main`. Run kickoff-branch first.

## Philosophy

This section is rationale and background. Take a deep breath and reason it out.
It explains why the discipline matters but encodes no rule.

## Workflow

Think step by step through each stage.

### 1. Planning

- [ ] Read the active epic story tasks
- [ ] Get user approval on the plan

## Checklist

- [ ] Test describes behavior, not implementation
- [ ] Boundary conditions tested: empty, max, min, off-by-one

## Verify

verify: `cargo test && echo OK`
"#;

/// The load-bearing lines that every non-full tier must retain (Property 4).
const INVARIANT_LINES: [&str; 6] = [
    "**HARD GATE**",
    "- [ ] Read the active epic story tasks",
    "- [ ] Get user approval on the plan",
    "- [ ] Test describes behavior, not implementation",
    "- [ ] Boundary conditions tested: empty, max, min, off-by-one",
    "verify: `cargo test && echo OK`",
];

#[test]
fn full_tier_is_identity() {
    // Requirement 6.6: full is byte-for-byte identical.
    assert_eq!(render_skill(SKILL_FIXTURE, Tier::Full), SKILL_FIXTURE);
}

#[test]
fn reasoning_removes_meta_scaffolding() {
    // Requirement 6.7: reasoning removes meta and guardrail scaffolding.
    let out = render_skill(SKILL_FIXTURE, Tier::Reasoning);
    assert!(!out.contains("<thinking>"));
    assert!(!out.contains("</thinking>"));
    assert!(!out.to_lowercase().contains("think step by step"));
    assert!(!out.to_lowercase().contains("take a deep breath"));
}

#[test]
fn reasoning_keeps_every_heading() {
    // Requirement 6.7: reasoning keeps every heading.
    let out = render_skill(SKILL_FIXTURE, Tier::Reasoning);
    for heading in [
        "# Develop TDD",
        "## Philosophy",
        "## Workflow",
        "### 1. Planning",
        "## Checklist",
        "## Verify",
    ] {
        assert!(
            out.contains(heading),
            "reasoning dropped heading `{heading}`"
        );
    }
}

#[test]
fn reasoning_keeps_every_invariant_line() {
    // Property 4: reasoning retains every invariant and acceptance-criterion line.
    let out = render_skill(SKILL_FIXTURE, Tier::Reasoning);
    for line in INVARIANT_LINES {
        assert!(
            out.contains(line),
            "reasoning dropped invariant line `{line}`"
        );
    }
}

#[test]
fn lean_removes_meta_scaffolding() {
    let out = render_skill(SKILL_FIXTURE, Tier::Lean);
    assert!(!out.contains("<thinking>"));
    assert!(!out.to_lowercase().contains("think step by step"));
    assert!(!out.to_lowercase().contains("take a deep breath"));
}

#[test]
fn lean_drops_rationale_section_body() {
    // Requirement 6.8: lean drops rationale and background section bodies.
    let out = render_skill(SKILL_FIXTURE, Tier::Lean);
    assert!(
        !out.contains("It explains why the discipline matters"),
        "lean kept a rationale body line"
    );
    // The Philosophy heading itself is retained (converted to a bullet).
    assert!(
        out.contains("Philosophy"),
        "lean dropped the Philosophy heading"
    );
}

#[test]
fn lean_keeps_every_invariant_line() {
    // Property 4: lean retains every invariant and acceptance-criterion line, even those
    // inside a dropped rationale section.
    let out = render_skill(SKILL_FIXTURE, Tier::Lean);
    for line in INVARIANT_LINES {
        assert!(out.contains(line), "lean dropped invariant line `{line}`");
    }
}

#[test]
fn lean_converts_headings_to_bullets() {
    // Requirement 6.8: lean converts headings to imperative bullets. `### 1. Planning`
    // loses its ordinal prefix and becomes `- Planning`.
    let out = render_skill(SKILL_FIXTURE, Tier::Lean);
    assert!(
        out.contains("- Planning"),
        "lean did not strip the step prefix"
    );
    assert!(out.contains("- Develop TDD"));
}

#[test]
fn lean_respects_the_token_budget() {
    // Requirement 6.8: lean does not exceed the configured token budget.
    let out = render_skill(SKILL_FIXTURE, Tier::Lean);
    let tokens = out.split_whitespace().count();
    assert!(
        tokens <= TIER_LEAN_TOKEN_BUDGET,
        "lean output exceeded the token budget: {tokens} tokens"
    );
}

#[test]
fn lean_truncates_a_large_document_to_budget() {
    // A document far larger than the budget is truncated. The invariant at the top
    // survives, since truncation keeps leading lines.
    let mut big = String::from("# Skill\n\n> **HARD GATE** — stay on a branch.\n\n## Body\n\n");
    for i in 0..5000 {
        big.push_str(&format!(
            "Filler prose line number {i} with several words here.\n"
        ));
    }
    let out = render_skill(&big, Tier::Lean);
    let tokens = out.split_whitespace().count();
    assert!(
        tokens <= TIER_LEAN_TOKEN_BUDGET,
        "budget exceeded: {tokens}"
    );
    assert!(
        out.contains("**HARD GATE**"),
        "truncation dropped the gate line"
    );
}

#[test]
fn duplicate_guardrail_collapses_in_reasoning() {
    // A repeated non-invariant blockquote collapses to its first occurrence. This quote
    // carries no gate marker, so it is guardrail scaffolding, not an invariant.
    let md = "# Skill\n\n> Remember to be careful.\n\n> Remember to be careful.\n";
    let out = render_skill(md, Tier::Reasoning);
    let occurrences = out.matches("Remember to be careful.").count();
    assert_eq!(occurrences, 1, "duplicate guardrail was not collapsed");
}

#[test]
fn real_skill_full_tier_roundtrips_when_present() {
    // Requirement 6.6 against a real skill. Skip when absent, so the test is portable.
    let Some(path) = repo_root_file("skills/develop-tdd/SKILL.md") else {
        return;
    };
    let md = std::fs::read_to_string(&path).expect("read real SKILL.md");
    assert_eq!(render_skill(&md, Tier::Full), md);
}

#[test]
fn real_skill_reasoning_keeps_verify_line_when_present() {
    // Property 4 against a real skill: the `verify:` gate line survives the reasoning
    // tier.
    let Some(path) = repo_root_file("skills/develop-tdd/SKILL.md") else {
        return;
    };
    let md = std::fs::read_to_string(&path).expect("read real SKILL.md");
    if !md.contains("verify:") {
        return;
    }
    let out = render_skill(&md, Tier::Reasoning);
    assert!(out.contains("verify:"), "reasoning dropped the verify line");
}

#[test]
fn lean_keeps_wiring_lines_in_a_dropped_section() {
    // Property 4 (#62): a dropped section removes its prose, but the wiring lines
    // (`Next:`, `Writes:`, `Gate:`) survive, so a chaining agent still learns the next
    // skill and the state writes. Rationale is a drop-body section today, so this test
    // stands on its own before the Handoff title joins the set.
    let md = "# Skill\n\n## Rationale\n\nThis section explains the downstream flow in prose.\nGate: READY. Next: kickoff-branch.\nWrites: `state.yaml` `handoff.next_skill = kickoff-branch`.\n";
    let out = render_skill(md, Tier::Lean);
    assert!(
        out.contains("Next: kickoff-branch"),
        "lean dropped the Next: wiring line"
    );
    assert!(
        out.contains("Writes:"),
        "lean dropped the Writes: wiring line"
    );
    assert!(out.contains("Gate: READY"), "lean dropped the Gate: line");
    assert!(
        !out.contains("explains the downstream flow"),
        "lean kept the dropped-section prose body"
    );
}

#[test]
fn wiring_keyword_in_prose_is_not_preserved() {
    // The anchor is the line start. A sentence that merely contains the word is prose,
    // so a dropped section still removes it.
    let md = "# Skill\n\n## Rationale\n\nThe planner writes to disk and moves next when ready.\n";
    let out = render_skill(md, Tier::Lean);
    assert!(
        !out.contains("moves next when ready"),
        "a mid-line keyword was wrongly preserved"
    );
}

/// Resolve a path relative to the repo root (one level above the crate manifest dir).
fn repo_root_file(relative: &str) -> Option<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    let path = repo_root.join(relative);
    path.exists().then_some(path)
}
