//! Tests for the SKILL.md parser (task 8b).
//!
//! Included from `skill_parser.rs` via `#[path]`, so `super` is the skill_parser module.
//!
//! Requirements: 7.2, 7.3.

use super::*;
use crate::engine::skill::RawSkill;

/// Build a `RawSkill` from markdown for parsing.
fn raw(markdown: &str) -> RawSkill {
    RawSkill {
        name: "develop-tdd".to_string(),
        path: PathBuf::from("skills/develop-tdd/SKILL.md"),
        markdown: markdown.to_string(),
        truncated: false,
    }
}

const FIXTURE: &str = r#"---
name: develop-tdd
model: sonnet
description: Test-driven development loop.
---

# Develop TDD

Intro paragraph.

## Workflow

Do the work. See [reference](skills/verify-work/SKILL.md).

```bash
echo verify: ok
```
"#;

#[test]
fn parses_frontmatter() {
    let parsed = parse_skill(&raw(FIXTURE));
    assert_eq!(
        parsed.frontmatter.get("name").and_then(|v| v.as_str()),
        Some("develop-tdd")
    );
    assert_eq!(
        parsed.frontmatter.get("model").and_then(|v| v.as_str()),
        Some("sonnet")
    );
}

#[test]
fn parses_headings() {
    let parsed = parse_skill(&raw(FIXTURE));
    let headings: Vec<(u8, &str)> = parsed
        .headings
        .iter()
        .map(|h| (h.depth, h.text.as_str()))
        .collect();
    assert_eq!(headings, vec![(1, "Develop TDD"), (2, "Workflow")]);
}

#[test]
fn parses_code_blocks() {
    let parsed = parse_skill(&raw(FIXTURE));
    assert_eq!(parsed.code_blocks.len(), 1);
    assert_eq!(parsed.code_blocks[0].lang.as_deref(), Some("bash"));
    assert!(parsed.code_blocks[0].value.contains("verify: ok"));
}

#[test]
fn parses_links() {
    let parsed = parse_skill(&raw(FIXTURE));
    assert_eq!(parsed.links.len(), 1);
    assert_eq!(parsed.links[0].text, "reference");
    assert_eq!(parsed.links[0].url, "skills/verify-work/SKILL.md");
}

#[test]
fn parses_sections_split_on_headings() {
    let parsed = parse_skill(&raw(FIXTURE));
    // A section per heading; the intro paragraph attaches to the first heading section.
    let headings: Vec<Option<&str>> = parsed
        .sections
        .iter()
        .map(|s| s.heading.as_deref())
        .collect();
    assert!(headings.contains(&Some("Develop TDD")));
    assert!(headings.contains(&Some("Workflow")));
}

#[test]
fn raw_prose_joins_paragraphs() {
    let parsed = parse_skill(&raw(FIXTURE));
    assert!(parsed.raw_prose.contains("Intro paragraph."));
    assert!(parsed.raw_prose.contains("Do the work."));
}

#[test]
fn missing_frontmatter_yields_empty_map() {
    let parsed = parse_skill(&raw("# No Frontmatter\n\nBody.\n"));
    assert!(parsed.frontmatter.is_empty());
    assert_eq!(parsed.headings.len(), 1);
}

#[test]
fn malformed_frontmatter_is_tolerated() {
    // A malformed YAML block yields an empty map, not an error.
    let parsed = parse_skill(&raw("---\n: : : bad yaml\n---\n\n# Title\n"));
    assert!(parsed.frontmatter.is_empty());
    assert_eq!(parsed.headings.len(), 1);
}
