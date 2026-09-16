//! Ontology analysis (ADR-2, design §6).
//!
//! The ontology gate scans target-project code for prohibited aliases and reports each
//! one as a [`Violation`] that cites the owning constraint id and names the correct term
//! (Requirements 4.5, 4.6). Analysis sits behind the [`OntologyAnalyzer`] trait. The
//! [`RegexAnalyzer`] is the language-agnostic baseline and is always available. The
//! `pick_analyzer` seam selects an AST analyzer where a grammar exists, else the baseline
//! (Requirement 4.9). The AST analyzer arrives in a later, optional task.
//!
//! Requirements: 4.5, 4.6, 4.9. Design: Part II §6.

// The ontology scan is consumed by the verify-ontology tool (task 12). It is unused until
// that task lands, so the module-scoped allow prevents a premature dead-code error under
// `clippy -D warnings`. Remove this allow once task 12 wires the consumer.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::engine::spec::Ontology;

/// A single ontology violation (design §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The owning constraint id, for example `C-02`.
    pub constraint_id: String,
    /// The file the violation occurred in.
    pub path: PathBuf,
    /// The 1-based line number of the violation.
    pub line: usize,
    /// The formatted, actionable message.
    pub message: String,
}

/// An ontology analysis backend.
///
/// The trait lets the gate select a language-specific AST analyzer where a grammar
/// exists and fall back to the regex baseline otherwise (Requirement 4.9).
pub trait OntologyAnalyzer {
    /// Return the violations in a single file's contents.
    fn scan(&self, path: &Path, contents: &str, ontology: &Ontology) -> Vec<Violation>;
}

/// The language-agnostic regex baseline analyzer (Requirement 4.9).
///
/// It detects a prohibited alias when the alias appears as a whole identifier in the
/// file, so a substring inside a longer identifier does not trigger a false violation.
pub struct RegexAnalyzer;

impl OntologyAnalyzer for RegexAnalyzer {
    fn scan(&self, path: &Path, contents: &str, ontology: &Ontology) -> Vec<Violation> {
        let links = ConstraintLinks::from_ontology(ontology);
        let mut violations = Vec::new();

        for (alias, matcher) in links.alias_matchers() {
            for (index, text) in contents.lines().enumerate() {
                if matcher.is_match(text) {
                    let link = links.owning_constraint(alias);
                    violations.push(Violation {
                        constraint_id: link.constraint_id.clone(),
                        path: path.to_path_buf(),
                        line: index + 1,
                        message: link.message(alias, path, index + 1),
                    });
                }
            }
        }

        violations
    }
}

/// The AST-based analyzer for languages with a tree-sitter grammar (ADR-2, Requirement
/// 4.9).
///
/// It parses the file and reports a prohibited alias only where the alias is a real
/// identifier token in the syntax tree. It skips comments and string literals, so it does
/// not raise the false violations the regex baseline can raise for an alias mentioned in
/// prose or a string. It reuses [`ConstraintLinks`], so the constraint attribution and the
/// message match the regex path exactly. Only the detection precision differs.
///
/// The analyzer is optional and compiles only under the `tree-sitter` feature.
#[cfg(feature = "tree-sitter")]
pub struct AstAnalyzer {
    /// The grammar for the file's language.
    language: tree_sitter::Language,
}

#[cfg(feature = "tree-sitter")]
impl AstAnalyzer {
    /// Build an analyzer for a supported file, selected by extension.
    ///
    /// Return `None` when no bundled grammar covers the file, so the caller falls back to
    /// the regex baseline.
    pub fn for_path(path: &Path) -> Option<Self> {
        let language = grammar_for_extension(path)?;
        Some(Self { language })
    }
}

/// Return the bundled grammar for a file extension, or `None` when none applies.
///
/// Rust is the first supported grammar. The seam grows by adding an arm here and the
/// matching optional grammar dependency.
#[cfg(feature = "tree-sitter")]
fn grammar_for_extension(path: &Path) -> Option<tree_sitter::Language> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some(tree_sitter_rust::LANGUAGE.into()),
        _ => None,
    }
}

#[cfg(feature = "tree-sitter")]
impl OntologyAnalyzer for AstAnalyzer {
    fn scan(&self, path: &Path, contents: &str, ontology: &Ontology) -> Vec<Violation> {
        let links = ConstraintLinks::from_ontology(ontology);
        let aliases = links.aliases();
        if aliases.is_empty() {
            return Vec::new();
        }

        let mut parser = tree_sitter::Parser::new();
        // A grammar-set failure or a parse failure is not a scan error. Fall back to the
        // regex baseline, so a supported file is never left unscanned (Requirement 4.9).
        if parser.set_language(&self.language).is_err() {
            return RegexAnalyzer.scan(path, contents, ontology);
        }
        let Some(tree) = parser.parse(contents, None) else {
            return RegexAnalyzer.scan(path, contents, ontology);
        };

        let bytes = contents.as_bytes();
        let mut violations = Vec::new();
        let mut cursor = tree.walk();
        let mut stack = vec![tree.root_node()];

        // Walk every node. Report an alias only at an identifier-like leaf, so a comment or
        // a string that contains the alias does not trigger a violation.
        while let Some(node) = stack.pop() {
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
            if !is_identifier_node(node.kind()) {
                continue;
            }
            let Ok(text) = node.utf8_text(bytes) else {
                continue;
            };
            if let Some(alias) = aliases.iter().find(|alias| alias.as_str() == text) {
                let link = links.owning_constraint(alias);
                // tree-sitter rows are 0-based; the violation line is 1-based.
                let line = node.start_position().row + 1;
                violations.push(Violation {
                    constraint_id: link.constraint_id.clone(),
                    path: path.to_path_buf(),
                    line,
                    message: link.message(alias, path, line),
                });
            }
        }

        violations.sort_by_key(|violation| violation.line);
        violations
    }
}

/// Report whether a tree-sitter node kind names an identifier token.
///
/// The ontology gate matches an alias only at an identifier, not inside a comment or a
/// string. The identifier node kinds cover the positions where a field, variable, type,
/// or shorthand names the alias.
#[cfg(feature = "tree-sitter")]
fn is_identifier_node(kind: &str) -> bool {
    matches!(
        kind,
        "identifier" | "field_identifier" | "type_identifier" | "shorthand_field_identifier"
    )
}

/// Select an analyzer for a file (Requirement 4.9).
///
/// With the `tree-sitter` feature on, the AST analyzer is returned for a file whose
/// language has a bundled grammar, adding identifier precision. Every other file, and
/// every file when the feature is off, uses the language-agnostic regex baseline. The
/// baseline is never blocked (ADR-2).
pub fn pick_analyzer(path: &Path) -> Box<dyn OntologyAnalyzer> {
    #[cfg(feature = "tree-sitter")]
    {
        if let Some(analyzer) = AstAnalyzer::for_path(path) {
            return Box::new(analyzer);
        }
    }
    // Referenced by the AST arm only when the feature is off; name it to avoid an
    // unused-variable warning in that build.
    let _ = path;
    Box::new(RegexAnalyzer)
}

/// The link from a prohibited alias to its owning constraint.
struct ConstraintLink {
    /// The owning constraint id.
    constraint_id: String,
    /// The owning constraint rule text, used in the remediation hint.
    rule: String,
    /// The entity the alias belongs to, used in the remediation hint.
    entity: String,
}

impl ConstraintLink {
    /// Format the violation message (design §6).
    ///
    /// The message cites the constraint id, states the violating term, and names the
    /// correct modeling drawn from the owning constraint rule (Requirement 4.6).
    fn message(&self, alias: &str, path: &Path, line: usize) -> String {
        format!(
            "Error [Ontology Gate {id}]: `{alias}` violates specs/ontology.yaml for entity `{entity}`. \
             {rule} ({path}:{line})",
            id = self.constraint_id,
            entity = self.entity,
            rule = self.rule,
            path = path.display(),
        )
    }
}

/// The alias-to-constraint links for one ontology.
struct ConstraintLinks {
    /// Every prohibited alias mapped to its owning constraint link.
    links: HashMap<String, ConstraintLink>,
}

impl ConstraintLinks {
    /// Build the links from an ontology.
    ///
    /// For each entity's prohibited alias, the owning constraint is the first constraint
    /// whose rule references the alias, either as a literal substring or through a
    /// `word_*` glob the alias matches. When no constraint references the alias, a
    /// synthesized id derived from the entity owns it, so every alias still cites an id.
    fn from_ontology(ontology: &Ontology) -> Self {
        let mut links = HashMap::new();

        for entity in &ontology.entities {
            for alias in &entity.prohibited_aliases {
                let owner = ontology
                    .constraints
                    .iter()
                    .find(|constraint| rule_references_alias(&constraint.rule, alias));

                let link = match owner {
                    Some(constraint) => ConstraintLink {
                        constraint_id: constraint.id.clone(),
                        rule: constraint.rule.clone(),
                        entity: entity.name.clone(),
                    },
                    None => ConstraintLink {
                        constraint_id: format!("{}-ALIAS", entity.name.to_uppercase()),
                        rule: format!(
                            "Use the canonical `{}` term instead of the alias.",
                            entity.primary_key
                        ),
                        entity: entity.name.clone(),
                    },
                };
                links.insert(alias.clone(), link);
            }
        }

        Self { links }
    }

    /// The owning constraint link for an alias.
    fn owning_constraint(&self, alias: &str) -> &ConstraintLink {
        self.links
            .get(alias)
            .expect("every scanned alias has a link built from the same ontology")
    }

    /// Every prohibited alias, used by the AST analyzer to match identifier tokens.
    #[cfg(feature = "tree-sitter")]
    fn aliases(&self) -> Vec<String> {
        self.links.keys().cloned().collect()
    }

    /// The alias identifier matchers, one per prohibited alias.
    fn alias_matchers(&self) -> Vec<(&str, Regex)> {
        self.links
            .keys()
            .map(|alias| (alias.as_str(), identifier_matcher(alias)))
            .collect()
    }
}

/// Build a whole-identifier matcher for an alias.
///
/// The alias matches only as a complete identifier token, bounded by a non-identifier
/// character or a line edge. So `is_deleted` matches `is_deleted` but not
/// `is_deleted_at`.
fn identifier_matcher(alias: &str) -> Regex {
    let escaped = regex::escape(alias);
    // `(?-w)` is not needed: use explicit boundaries that treat `_` as part of a word, so
    // an underscore-joined longer identifier does not match.
    let pattern = format!(r"(^|[^A-Za-z0-9_])({escaped})($|[^A-Za-z0-9_])");
    Regex::new(&pattern).expect("alias identifier pattern must compile")
}

/// Report whether a constraint rule references an alias.
///
/// A rule references an alias when it contains the alias literally, or when it contains a
/// `word_*` glob that the alias matches (for example the rule `Boolean flags (is_*) are
/// prohibited` references `is_deleted`).
fn rule_references_alias(rule: &str, alias: &str) -> bool {
    if rule.contains(alias) {
        return true;
    }
    glob_patterns(rule)
        .iter()
        .any(|prefix| alias.starts_with(prefix) && alias.len() > prefix.len())
}

/// Extract the prefixes of `word_*` glob patterns from a constraint rule.
///
/// For the rule text `Boolean state flags (is_*) are prohibited`, this returns `["is_"]`.
fn glob_patterns(rule: &str) -> Vec<String> {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        // A `word_*` glob: an identifier fragment ending in `_*`.
        Regex::new(r"([A-Za-z0-9_]+_)\*").expect("glob pattern must compile")
    });
    pattern
        .captures_iter(rule)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `ontology_scan`.
#[cfg(test)]
#[path = "ontology_scan_tests.rs"]
mod tests;
