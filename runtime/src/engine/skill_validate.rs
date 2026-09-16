//! SKILL.md convention checks (ports `validate-skill.ts`).
//!
//! `validate_skill` runs the legacy convention checks over a parsed skill: verb-noun
//! naming, required frontmatter, a verify command, a line-count cap, and resolvable
//! `skills/` links. Each check reports pass or fail with a remediation hint.
//!
//! Requirements: 7.1. Design: Part II §1.

// The validator is consumed by the validate_skill tool (task 8b). It is unused until it
// wires the check, so the module-scoped allow prevents a premature dead-code error under
// `clippy -D warnings`. Remove this allow once task 8b wires the consumer.
#![allow(dead_code)]

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::engine::skill_parser::ParsedSkill;

/// The line-count cap for a skill.
///
/// The legacy code always applied 150 through a dead-code ternary; the 120 branch never
/// ran. This ports the effective behavior, a flat 150-line cap, without the dead branch.
const SIZE_CAP_LINES: usize = 150;

/// A single convention check (ports `ValidationCheck`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationCheck {
    /// The check id.
    pub id: String,
    /// Whether the check passed.
    pub pass: bool,
    /// The result message.
    pub message: String,
    /// An optional remediation hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

/// The full validation report (ports `ValidationReport`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    /// The skill name.
    pub skill: String,
    /// Whether every check passed.
    pub pass: bool,
    /// The individual checks.
    pub checks: Vec<ValidationCheck>,
}

/// Validate a parsed skill against the conventions (ports `validateSkillConventions`).
///
/// `line_count` is the skill file's line count, and `repo_root` resolves `skills/` links.
pub fn validate_skill(
    parsed: &ParsedSkill,
    repo_root: &Path,
    line_count: usize,
) -> ValidationReport {
    let mut checks = Vec::new();
    let name = &parsed.name;

    let verb_noun = verb_noun_re().is_match(name);
    checks.push(ValidationCheck {
        id: "verb-noun-naming".to_string(),
        pass: verb_noun,
        message: if verb_noun {
            "Skill name is verb-noun kebab-case".to_string()
        } else {
            format!("Skill name `{name}` is not verb-noun kebab-case")
        },
        remediation: Some(
            "Rename directory to two-word kebab-case (for example develop-tdd)".to_string(),
        ),
    });

    for field in ["name", "description"] {
        let present = parsed.frontmatter.contains_key(field);
        checks.push(ValidationCheck {
            id: format!("frontmatter-{field}"),
            pass: present,
            message: if present {
                format!("Has frontmatter.{field}")
            } else {
                format!("Missing frontmatter.{field}")
            },
            remediation: Some(format!("Add {field} to the YAML frontmatter block")),
        });
    }

    let has_verify = parsed.raw_prose.contains("verify:")
        || parsed
            .code_blocks
            .iter()
            .any(|b| b.value.contains("verify:"));
    checks.push(ValidationCheck {
        id: "verify-command".to_string(),
        pass: has_verify,
        message: if has_verify {
            "Has verify command reference".to_string()
        } else {
            "Missing verify command".to_string()
        },
        remediation: Some("Add a verify: block with a runnable check".to_string()),
    });

    let within_cap = line_count <= SIZE_CAP_LINES;
    checks.push(ValidationCheck {
        id: "size-cap".to_string(),
        pass: within_cap,
        message: if within_cap {
            format!("Within size cap ({line_count}/{SIZE_CAP_LINES} lines)")
        } else {
            format!("Exceeds size cap ({line_count}/{SIZE_CAP_LINES} lines)")
        },
        remediation: Some("Split content into REFERENCE.md or reduce prose".to_string()),
    });

    for link in &parsed.links {
        if let Some(target) = link.url.strip_prefix("skills/") {
            let target = target.strip_suffix("/SKILL.md").unwrap_or(target);
            let resolves = repo_root
                .join("skills")
                .join(target)
                .join("SKILL.md")
                .is_file();
            checks.push(ValidationCheck {
                id: format!("link-{target}"),
                pass: resolves,
                message: if resolves {
                    format!("Link to {} resolves", link.url)
                } else {
                    format!("Broken link: {}", link.url)
                },
                remediation: if resolves {
                    None
                } else {
                    Some(format!("Create skills/{target}/SKILL.md or fix the link"))
                },
            });
        }
    }

    let pass = checks.iter().all(|c| c.pass);
    ValidationReport {
        skill: name.clone(),
        pass,
        checks,
    }
}

/// The verb-noun kebab-case pattern (ports the legacy `VERB_NOUN`).
fn verb_noun_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z]+-[a-z]+(-[a-z]+)*$").expect("regex compiles"))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `skill_validate`.
#[cfg(test)]
#[path = "skill_validate_tests.rs"]
mod tests;
