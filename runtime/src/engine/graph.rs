//! The skill knowledge graph (ports `graph/types.ts`, `graph/builder.ts`,
//! `graph/store.ts`).
//!
//! `build_graph` mines entity-relation data from parsed skills. The graph persists as
//! JSONL (`saveGraph`), loads back (`loadGraph`), and answers the dependency, search, and
//! open queries the legacy catalog tools expose.
//!
//! Requirements: 7.8, 7.9. Design: Part II §1.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;

use crate::engine::regex_util::compile_static;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::engine::skill_parser::ParsedSkill;

/// A graph entity (ports `GraphEntity`). A skill is an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEntity {
    /// The line type discriminator, always `"entity"`.
    #[serde(rename = "type")]
    pub line_type: String,
    /// The entity name (the skill name).
    pub name: String,
    /// The entity type, always `"Skill"` for a skill.
    #[serde(rename = "entityType")]
    pub entity_type: String,
    /// Free-form observations, one per frontmatter field.
    pub observations: Vec<String>,
}

/// A graph relation (ports `GraphRelation`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphRelation {
    /// The line type discriminator, always `"relation"`.
    #[serde(rename = "type")]
    pub line_type: String,
    /// The source node name.
    pub from: String,
    /// The target node name.
    pub to: String,
    /// The relation type, for example `depends_on`.
    #[serde(rename = "relationType")]
    pub relation_type: String,
}

/// The in-memory skill graph (ports `SkillGraph`). Entities are keyed by name; relations
/// are a list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillGraph {
    /// The entities, keyed by name. A `BTreeMap` keeps a stable order.
    pub entities: BTreeMap<String, GraphEntity>,
    /// The relations, in insertion order.
    pub relations: Vec<GraphRelation>,
}

/// A canonical skill mention that could not be resolved to a current catalog entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MentionDiagnostic {
    /// Skill containing the mention.
    pub source: String,
    /// Referenced target text.
    pub target: String,
    /// One-based source line.
    pub line: usize,
    /// Canonical relation syntax that produced the mention.
    pub syntax: String,
}

/// Diagnostics produced while constructing a graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GraphDiagnostics {
    /// Canonical relation targets absent from the current catalog.
    pub unresolved_mentions: Vec<MentionDiagnostic>,
    /// Exact catalog mentions that were neither related nor explicitly classified.
    pub unclassified_mentions: Vec<MentionDiagnostic>,
    /// Skills with no incoming or outgoing skill relation.
    pub isolated_skills: Vec<String>,
    /// Isolated skills whose contract describes orchestration or routing.
    pub suspicious_isolated_orchestrators: Vec<String>,
    /// Cycles in the `depends_on` relation.
    pub dependency_cycles: Vec<Vec<String>>,
}

/// A graph and the diagnostics observed while building it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphBuildResult {
    /// Constructed graph.
    pub graph: SkillGraph,
    /// Catalog consistency diagnostics.
    pub diagnostics: GraphDiagnostics,
}

impl GraphEntity {
    /// Build a skill entity with the given name and observations.
    fn skill(name: &str, observations: Vec<String>) -> Self {
        Self {
            line_type: "entity".to_string(),
            name: name.to_string(),
            entity_type: "Skill".to_string(),
            observations,
        }
    }
}

impl GraphRelation {
    /// Build a relation.
    fn new(from: &str, to: &str, relation_type: &str) -> Self {
        Self {
            line_type: "relation".to_string(),
            from: from.to_string(),
            to: to.to_string(),
            relation_type: relation_type.to_string(),
        }
    }
}

/// Build the graph from parsed skills.
pub fn build_graph(skills: &[ParsedSkill]) -> SkillGraph {
    build_graph_report(skills).graph
}

/// Build the graph and classify every exact inter-skill mention.
///
/// Relation direction is semantic:
///
/// - `A depends_on B`: A requires B before A can run.
/// - `A invokes B`: A directly runs or routes work to B.
/// - `A handoff_to B`: A may select B as its next lifecycle skill.
/// - `A references B`: A mentions B without execution ordering.
/// - `A enforces B`: A applies B's convention contract.
pub fn build_graph_report(skills: &[ParsedSkill]) -> GraphBuildResult {
    let known: BTreeSet<String> = skills.iter().map(|skill| skill.name.clone()).collect();
    let mut result = GraphBuildResult::default();

    for skill in skills {
        result.graph.entities.insert(
            skill.name.clone(),
            GraphEntity::skill(&skill.name, observations_from(skill)),
        );
    }
    for skill in skills {
        mine_relations(&mut result, skill, &known);
    }

    finish_diagnostics(&mut result, skills);
    result
}

/// The observations for a skill, drawn from its frontmatter.
fn observations_from(skill: &ParsedSkill) -> Vec<String> {
    let mut observations = Vec::new();
    for field in ["description"] {
        if let Some(value) = skill.frontmatter.get(field)
            && let Some(text) = scalar_string(value)
        {
            observations.push(format!("{field}: {text}"));
        }
    }
    observations
}

/// Mine classified relations from one skill.
fn mine_relations(result: &mut GraphBuildResult, skill: &ParsedSkill, known: &BTreeSet<String>) {
    let prose = &skill.relation_text;
    let description = skill
        .frontmatter
        .get("description")
        .and_then(scalar_string)
        .unwrap_or_default();
    let combined = format!("{description}\n{prose}");

    let lower = normalize_relation_text(&combined);
    for target in known {
        if target == &skill.name {
            continue;
        }
        if contains_phrase(&lower, "after ", target) {
            push_unique(
                &mut result.graph,
                GraphRelation::new(&skill.name, target, "depends_on"),
            );
        }
        if contains_phrase(&lower, "before ", target) {
            push_unique(
                &mut result.graph,
                GraphRelation::new(target, &skill.name, "depends_on"),
            );
        }
        if ["run ", "invoke ", "invokes ", "route to "]
            .iter()
            .any(|prefix| contains_phrase(&lower, prefix, target))
        {
            push_unique(
                &mut result.graph,
                GraphRelation::new(&skill.name, target, "invokes"),
            );
        }
        if [
            "next: ",
            "next_skill: ",
            "next_skill = ",
            "hand off to ",
            "hand off directly to ",
        ]
        .iter()
        .any(|prefix| contains_phrase(&lower, prefix, target))
        {
            push_unique(
                &mut result.graph,
                GraphRelation::new(&skill.name, target, "handoff_to"),
            );
        }
    }

    // Explicit paths are canonical even when their target was renamed or removed.
    for caps in skill_ref_re().captures_iter(prose) {
        record_canonical(
            result,
            skill,
            known,
            prose,
            caps.get(1).expect("skill path target"),
            "references",
            "skill path",
        );
    }
    for link in &skill.links {
        for caps in skill_ref_re().captures_iter(&link.url) {
            record_canonical(
                result,
                skill,
                known,
                &link.url,
                caps.get(1).expect("linked skill target"),
                "references",
                "skill link",
            );
        }
    }

    // Every remaining exact catalog mention is deliberately non-control-flow.
    for target in known {
        if target == &skill.name || !contains_skill_name(&lower, target) {
            continue;
        }
        let already_classified = result
            .graph
            .relations
            .iter()
            .any(|relation| relation.from == skill.name && relation.to == *target);
        if !already_classified {
            push_unique(
                &mut result.graph,
                GraphRelation::new(&skill.name, target, "references"),
            );
        }
    }

    for caps in conventions_re().captures_iter(prose) {
        let target = match caps.get(1) {
            Some(section) => format!("CONVENTIONS.md §{}", section.as_str()),
            None => "CONVENTIONS.md".to_string(),
        };
        push_unique(
            &mut result.graph,
            GraphRelation::new(&skill.name, &target, "enforces"),
        );
    }
}

/// Record a canonical relation or an actionable unresolved-target diagnostic.
fn record_canonical(
    result: &mut GraphBuildResult,
    skill: &ParsedSkill,
    known: &BTreeSet<String>,
    source: &str,
    target_match: regex::Match<'_>,
    relation_type: &str,
    syntax: &str,
) {
    let target = target_match.as_str().to_ascii_lowercase();
    if target == skill.name || target == "null" {
        return;
    }
    if known.contains(&target) {
        push_unique(
            &mut result.graph,
            GraphRelation::new(&skill.name, &target, relation_type),
        );
        return;
    }
    let diagnostic = MentionDiagnostic {
        source: skill.name.clone(),
        target,
        line: line_number(source, target_match.start()),
        syntax: syntax.to_string(),
    };
    if !result.diagnostics.unresolved_mentions.contains(&diagnostic) {
        result.diagnostics.unresolved_mentions.push(diagnostic);
    }
}

/// One-based line containing a byte offset.
fn line_number(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

/// Lowercase and collapse Markdown parser spacing for canonical phrase matching.
fn normalize_relation_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for part in text.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(part);
    }
    normalized.make_ascii_lowercase();
    normalized
}

/// Whether text contains a canonical phrase followed by a known skill.
///
/// The parser flattens inline code and following prose without retaining the code
/// delimiter, so the target intentionally needs only a leading phrase boundary.
fn contains_phrase(text: &str, prefix: &str, target: &str) -> bool {
    let needle = format!("{prefix}{target}");
    text.match_indices(&needle).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        !before.is_some_and(is_skill_char)
    })
}

/// Whether text contains a skill name with kebab-case token boundaries.
fn contains_skill_name(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(start, _)| {
        let end = start + name.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        !before.is_some_and(is_skill_char) && !after.is_some_and(is_skill_char)
    })
}

fn is_skill_char(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
}

/// Populate diagnostics that require the complete relation set.
fn finish_diagnostics(result: &mut GraphBuildResult, skills: &[ParsedSkill]) {
    let known: BTreeSet<&str> = result.graph.entities.keys().map(String::as_str).collect();
    let mut incident = BTreeSet::new();
    for relation in &result.graph.relations {
        if known.contains(relation.from.as_str()) {
            incident.insert(relation.from.clone());
        }
        if known.contains(relation.to.as_str()) {
            incident.insert(relation.to.clone());
        }
    }
    result.diagnostics.isolated_skills = result
        .graph
        .entities
        .keys()
        .filter(|name| !incident.contains(*name))
        .cloned()
        .collect();

    for skill in skills {
        let has_outgoing_control = result.graph.relations.iter().any(|relation| {
            relation.from == skill.name
                && matches!(relation.relation_type.as_str(), "invokes" | "handoff_to")
        });
        if has_outgoing_control {
            continue;
        }
        let description = skill
            .frontmatter
            .get("description")
            .and_then(scalar_string)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ["orchestrat", "chain multiple skills", "route work"]
            .iter()
            .any(|marker| description.contains(marker))
        {
            result
                .diagnostics
                .suspicious_isolated_orchestrators
                .push(skill.name.clone());
        }
    }
    result.diagnostics.dependency_cycles = dependency_cycles(&result.graph);
}

/// Find deterministic `depends_on` cycles.
fn dependency_cycles(graph: &SkillGraph) -> Vec<Vec<String>> {
    let mut cycles = BTreeSet::new();
    for start in graph.entities.keys() {
        let mut path = Vec::new();
        visit_dependencies(graph, start, start, &mut path, &mut cycles);
    }
    cycles.into_iter().collect()
}

fn visit_dependencies(
    graph: &SkillGraph,
    start: &str,
    current: &str,
    path: &mut Vec<String>,
    cycles: &mut BTreeSet<Vec<String>>,
) {
    if path.len() >= graph.entities.len() {
        return;
    }
    path.push(current.to_string());
    for next in graph
        .relations
        .iter()
        .filter(|relation| relation.from == current && relation.relation_type == "depends_on")
        .map(|relation| relation.to.as_str())
    {
        if next == start {
            let mut cycle = path.clone();
            cycle.sort();
            cycle.dedup();
            cycles.insert(cycle);
        } else if !path.iter().any(|node| node == next) {
            visit_dependencies(graph, start, next, path, cycles);
        }
    }
    path.pop();
}

/// Push a relation only when an identical one is not already present, so a phrasing that
/// matches twice (for example "after X" in both the description and the prose) yields one
/// edge.
fn push_unique(graph: &mut SkillGraph, relation: GraphRelation) {
    if !graph.relations.contains(&relation) {
        graph.relations.push(relation);
    }
}

/// Serialize the graph to JSONL, entities first then relations (ports `saveGraph`).
pub fn to_jsonl(graph: &SkillGraph) -> String {
    // Entities and relations are plain string-and-vec structs, so serialization does not
    // fail. Skip any value that somehow fails to serialize rather than panic, so a graph
    // write never crashes the process.
    let mut lines: Vec<String> = Vec::new();
    lines.extend(
        graph
            .entities
            .values()
            .filter_map(|entity| serde_json::to_string(entity).ok()),
    );
    lines.extend(
        graph
            .relations
            .iter()
            .filter_map(|relation| serde_json::to_string(relation).ok()),
    );
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

/// Load the graph from a JSONL file, or an empty graph when absent (ports `loadGraph`).
///
/// A malformed line is skipped rather than failing the load, so a partially-written graph
/// still yields the readable entries.
pub fn load_graph(path: &Path) -> SkillGraph {
    let Ok(text) = std::fs::read_to_string(path) else {
        return SkillGraph::default();
    };
    from_jsonl(&text)
}

/// Parse a graph from JSONL text.
pub fn from_jsonl(text: &str) -> SkillGraph {
    let mut graph = SkillGraph::default();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if let Ok(entity) = serde_json::from_str::<GraphEntity>(line)
            && entity.line_type == "entity"
        {
            graph.entities.insert(entity.name.clone(), entity);
            continue;
        }
        if let Ok(relation) = serde_json::from_str::<GraphRelation>(line)
            && relation.line_type == "relation"
        {
            graph.relations.push(relation);
        }
    }
    graph
}

/// Search entities by name, type, or observation text (ports `searchNodes`).
pub fn search_nodes<'a>(graph: &'a SkillGraph, query: &str) -> Vec<&'a GraphEntity> {
    let q = query.to_lowercase();
    graph
        .entities
        .values()
        .filter(|entity| {
            let mut hay = format!("{} {}", entity.name, entity.entity_type);
            for observation in &entity.observations {
                hay.push(' ');
                hay.push_str(observation);
            }
            hay.to_lowercase().contains(&q)
        })
        .collect()
}

/// Open entities by name, dropping unknown names (ports `openNodes`).
pub fn open_nodes<'a>(graph: &'a SkillGraph, names: &[String]) -> Vec<&'a GraphEntity> {
    names
        .iter()
        .filter_map(|name| graph.entities.get(name))
        .collect()
}

/// The forward dependencies of a skill (ports `getForwardDeps`).
pub fn forward_deps(graph: &SkillGraph, name: &str) -> Vec<String> {
    graph
        .relations
        .iter()
        .filter(|r| r.from == name && r.relation_type == "depends_on")
        .map(|r| r.to.clone())
        .collect()
}

/// The reverse dependencies of a skill (ports `getReverseDeps`).
pub fn reverse_deps(graph: &SkillGraph, name: &str) -> Vec<String> {
    graph
        .relations
        .iter()
        .filter(|r| r.to == name && r.relation_type == "depends_on")
        .map(|r| r.from.clone())
        .collect()
}

/// The handoff chain from a skill, following `handoff_to` edges up to 20 hops, stopping
/// on a missing edge or a cycle (ports `getHandoffChain`).
pub fn handoff_chain(graph: &SkillGraph, name: &str) -> Vec<String> {
    let mut chain = vec![name.to_string()];
    let mut seen = std::collections::HashSet::new();
    seen.insert(name.to_string());
    let mut current = name.to_string();

    for _ in 0..20 {
        let next = graph
            .relations
            .iter()
            .find(|r| r.from == current && r.relation_type == "handoff_to")
            .map(|r| r.to.clone());
        match next {
            Some(to) if !seen.contains(&to) => {
                chain.push(to.clone());
                seen.insert(to.clone());
                current = to;
            }
            _ => break,
        }
    }
    chain
}

/// The conventions a skill enforces (ports `getConventions`).
pub fn conventions(graph: &SkillGraph, name: &str) -> Vec<String> {
    graph
        .relations
        .iter()
        .filter(|r| r.from == name && r.relation_type == "enforces")
        .map(|r| r.to.clone())
        .collect()
}

/// Render a YAML scalar as a string, when it is a string, number, or bool.
fn scalar_string(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// The relation-mining regexes, compiled once from build-constant patterns. A skill name
// is kebab-case (a lowercase letter, then lowercase letters, digits, and hyphens), so the
// capture groups below match a skill name shape and the caller validates it against the
// known set.

/// An explicit `skills/<name>/SKILL.md` or sibling `../<name>/SKILL.md` reference.
fn skill_ref_re() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static(r"(?i)(?:skills/|\.\./)([a-z][a-z0-9-]+)/SKILL\.md"));
    &RE
}

/// A `CONVENTIONS.md` reference, with an optional section after `§` or `#`.
fn conventions_re() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static(r"(?i)CONVENTIONS\.md(?:\s*[§#]\s*(\S+))?"));
    &RE
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `graph`.
#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
