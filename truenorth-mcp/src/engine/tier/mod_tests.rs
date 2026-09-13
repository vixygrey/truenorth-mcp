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
fn estimate_tokens_rounds_up_by_four_chars() {
    // #83: the estimate is the character count divided by four, rounded up.
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("a"), 1); // 1 char -> ceil(1/4) = 1
    assert_eq!(estimate_tokens("abcd"), 1); // 4 chars -> 1
    assert_eq!(estimate_tokens("abcde"), 2); // 5 chars -> ceil(5/4) = 2
    assert_eq!(estimate_tokens("abcdefgh"), 2); // 8 chars -> 2
}

#[test]
fn estimate_tokens_counts_characters_not_bytes() {
    // A multi-byte character counts as one character, not its byte length.
    assert_eq!(estimate_tokens("\u{00e9}\u{00e9}\u{00e9}\u{00e9}"), 1); // 4 chars -> 1
}

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
    // Requirement 6.8: lean does not exceed the configured token budget. The budget is
    // measured in estimated tokens (#83).
    let out = render_skill(SKILL_FIXTURE, Tier::Lean);
    let tokens = estimate_tokens(&out);
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
    let tokens = estimate_tokens(&out);
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
fn lean_drops_references_and_out_of_scope_bodies() {
    // Requirement 6.8 (#62): References citations and Out of scope exclusions are inert
    // for a lean acting agent, so their bodies drop while the heading stays.
    let md = "# Skill\n\n## References\n\n- Fowler's Refactoring Catalog: the canonical vocabulary.\n\n## Out of scope\n\nWork beyond the destination is not routed here.\n";
    let out = render_skill(md, Tier::Lean);
    assert!(
        out.contains("References"),
        "lean dropped the References heading"
    );
    assert!(
        !out.contains("Fowler's Refactoring Catalog"),
        "lean kept a References body line"
    );
    assert!(
        out.contains("Out of scope") || out.contains("Out of Scope"),
        "lean dropped the Out of scope heading"
    );
    assert!(
        !out.contains("Work beyond the destination"),
        "lean kept an Out of scope body line"
    );
}

#[test]
fn lean_drops_handoff_prose_but_keeps_wiring() {
    // Handoff joins the drop-body set (#62). Its prose drops, its wiring survives.
    let md = "# Skill\n\n## Handoff\n\nThe downstream flow is described in this prose sentence.\nNext: kickoff-branch.\nWrites: `state.yaml`.\n";
    let out = render_skill(md, Tier::Lean);
    assert!(out.contains("Handoff"), "lean dropped the Handoff heading");
    assert!(
        !out.contains("described in this prose"),
        "lean kept the Handoff prose body"
    );
    assert!(out.contains("Next: kickoff-branch"), "lean dropped Next:");
    assert!(out.contains("Writes:"), "lean dropped Writes:");
}

#[test]
fn lean_keeps_integration_points_and_notes_bodies() {
    // #62: integration points and notes carry action-relevant prose, so they are NOT in
    // the drop-body set. Their bodies survive.
    let md = "# Skill\n\n## Integration points\n\nThe release-branch gate blocks the merge on a FAIL.\n\n## Notes\n\nHusky v9 does not need shebangs in hook files.\n";
    let out = render_skill(md, Tier::Lean);
    assert!(
        out.contains("release-branch gate blocks the merge"),
        "lean wrongly dropped the Integration points body"
    );
    assert!(
        out.contains("Husky v9 does not need shebangs"),
        "lean wrongly dropped the Notes body"
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

/// A skill with structural sections and a table, so the lean tier has real material to
/// compress. Every load-bearing line is listed in `STRUCTURAL_INVARIANTS`.
const STRUCTURAL_FIXTURE: &str = r#"# Release Branch

> **HARD GATE** — Do NOT merge on a red gate.

## Rationale

This section explains at length why the release discipline matters. It is pure
background prose that a lean acting agent does not need to perform the steps. It runs
several sentences to stand in for a real rationale body of meaningful size.

## Decision rules

| Condition            | Verdict            |
| -------------------- | ------------------ |
| tests fail           | block the merge    |
| lint clean and green | allow the merge    |
| coverage below bar   | request more tests |

## Checklist

- [ ] Diff scanned for unaddressed findings
- [ ] verify: `cargo test`

## References

- Fowler's Refactoring Catalog: the canonical vocabulary for structural change.
- Beck's Tidy First: structural change before behavioral change.

## Handoff

The downstream flow is described here in a prose sentence that a lean agent skips.
Next: integrate.
Writes: `state.yaml` `handoff.next_skill = integrate`.
"#;

/// The load-bearing lines the structural fixture must retain at every non-full tier.
const STRUCTURAL_INVARIANTS: [&str; 6] = [
    "**HARD GATE**",
    "- [ ] Diff scanned for unaddressed findings",
    "verify: `cargo test`",
    "block the merge",
    "Next: integrate",
    "Writes:",
];

#[test]
fn lean_meaningfully_compresses_a_structural_skill() {
    // Acceptance criterion (#62), measured in estimated tokens (#83): lean is at most 60%
    // of full tokens on a skill with structural sections and a table.
    let full = estimate_tokens(&render_skill(STRUCTURAL_FIXTURE, Tier::Full));
    let lean = estimate_tokens(&render_skill(STRUCTURAL_FIXTURE, Tier::Lean));
    assert!(
        (lean as f64) <= 0.60 * (full as f64),
        "lean did not compress enough: {lean} lean vs {full} full tokens"
    );
}

#[test]
fn lean_keeps_every_structural_invariant() {
    // Property 4 (#62): every load-bearing line survives, including a table verdict, the
    // handoff wiring, and a checkbox inside the compressed output.
    let out = render_skill(STRUCTURAL_FIXTURE, Tier::Lean);
    for line in STRUCTURAL_INVARIANTS {
        assert!(out.contains(line), "lean dropped invariant `{line}`");
    }
    // The rationale and references bodies are gone.
    assert!(
        !out.contains("pure\nbackground prose") && !out.contains("background prose"),
        "lean kept the rationale body"
    );
    assert!(
        !out.contains("canonical vocabulary"),
        "lean kept a References body line"
    );
}

#[test]
fn real_skill_lean_compresses_when_present() {
    // Acceptance criterion against a real skill, in estimated tokens (#83). `develop-tdd`
    // has a table and several drop-body sections, so the lean tier has real material to
    // remove. It measures near 0.71 tokens today. The bound is 0.80. Skip when absent, so
    // the test stays portable.
    let Some(path) = repo_root_file("skills/develop-tdd/SKILL.md") else {
        return;
    };
    let md = std::fs::read_to_string(&path).expect("read real SKILL.md");
    let full = estimate_tokens(&render_skill(&md, Tier::Full));
    let lean = estimate_tokens(&render_skill(&md, Tier::Lean));
    assert!(
        (lean as f64) < 0.80 * (full as f64),
        "lean did not compress the real skill: {lean} lean vs {full} full tokens"
    );
}

#[test]
fn table_dense_skill_compresses_in_tokens_not_words() {
    // #83: the token unit shows the table pass's true effect. `gate-trace` has
    // content-dense tables, so its word count barely moves (near 0.95), but its token
    // count drops (near 0.79) as padding and separators are stripped. Skip when absent.
    let Some(path) = repo_root_file("skills/gate-trace/SKILL.md") else {
        return;
    };
    let md = std::fs::read_to_string(&path).expect("read real SKILL.md");
    let full = estimate_tokens(&render_skill(&md, Tier::Full));
    let lean = estimate_tokens(&render_skill(&md, Tier::Lean));
    assert!(
        (lean as f64) < 0.85 * (full as f64),
        "the table-dense skill did not compress in tokens: {lean} lean vs {full} full"
    );
}

/// Resolve a path relative to the repo root (one level above the crate manifest dir).
fn repo_root_file(relative: &str) -> Option<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    let path = repo_root.join(relative);
    path.exists().then_some(path)
}
