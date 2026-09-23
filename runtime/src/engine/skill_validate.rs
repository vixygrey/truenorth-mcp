//! SKILL.md convention checks (ports `validate-skill.ts`).
//!
//! `validate_skill` runs the legacy convention checks over a parsed skill: verb-noun
//! naming, required frontmatter, a verify command, a line-count cap, and resolvable
//! `skills/` links. Each check reports pass or fail with a remediation hint.
//!
//! Requirements: 7.1. Design: Part II §1.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::engine::regex_util::compile_static;
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

    let documented_name_exception = parsed
        .frontmatter
        .get("name_exception")
        .and_then(serde_yaml::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    let verb_noun = verb_noun_re().is_match(name) || documented_name_exception;
    checks.push(ValidationCheck {
        id: "verb-noun-naming".to_string(),
        pass: verb_noun,
        message: if verb_noun {
            "Skill name is verb-noun kebab-case or has a documented exception".to_string()
        } else {
            format!("Skill name `{name}` is not verb-noun kebab-case")
        },
        remediation: Some(
            "Rename to verb-noun kebab-case or add a nonempty name_exception".to_string(),
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

    let kind = parsed
        .frontmatter
        .get("kind")
        .and_then(serde_yaml::Value::as_str);
    let kind_valid = matches!(kind, Some("prose" | "scripted"));
    checks.push(ValidationCheck {
        id: "skill-kind".to_string(),
        pass: kind_valid,
        message: if kind_valid {
            format!("Has supported skill kind `{}`", kind.unwrap_or_default())
        } else {
            "Missing or unsupported skill kind".to_string()
        },
        remediation: Some("Set frontmatter.kind to `prose` or `scripted`".to_string()),
    });

    let has_verify = match kind {
        Some("prose") => true,
        Some("scripted") => parsed
            .frontmatter
            .get("verify")
            .and_then(serde_yaml::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty()),
        _ => false,
    };
    checks.push(ValidationCheck {
        id: "verify-command".to_string(),
        pass: has_verify,
        message: match kind {
            Some("prose") => "Prose skills do not require a runnable command".to_string(),
            Some("scripted") if has_verify => {
                "Scripted skill has a runnable verify command".to_string()
            }
            _ => "Scripted skills require a frontmatter verify command".to_string(),
        },
        remediation: Some(
            "Set frontmatter.verify for a scripted skill, or declare the skill prose".to_string(),
        ),
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
    RE.get_or_init(|| compile_static(r"^[a-z]+-[a-z]+(-[a-z]+)*$"))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `skill_validate`.
#[cfg(test)]
#[path = "skill_validate_tests.rs"]
mod tests;
