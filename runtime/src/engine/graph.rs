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
use std::sync::OnceLock;

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

/// Build the graph from parsed skills (ports `buildGraphFromSkills`).
///
/// Each skill becomes an entity with a `description` observation. Relations are mined from
/// the prose and the description, and every skill-to-skill target is validated against the
/// set of known skill names, so a relation never points at a stray English word (#239).
pub fn build_graph(skills: &[ParsedSkill]) -> SkillGraph {
    let mut graph = SkillGraph::default();

    // Collect the known skill names first, so relation mining can reject a target that is
    // not a skill. This is the fix for the garbage targets the token-grabbing regexes
    // produced (for example `gates -> "Do"`), see #239.
    let known: BTreeSet<String> = skills.iter().map(|s| s.name.clone()).collect();

    for skill in skills {
        let observations = observations_from(skill);
        graph.entities.insert(
            skill.name.clone(),
            GraphEntity::skill(&skill.name, observations),
        );

        mine_relations(&mut graph, skill, &known);
    }

    graph
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

/// Mine relations from a skill, validating every skill target against `known` (#239).
///
/// Four relation kinds are mined:
///
/// - `references`: an explicit `skills/<name>/SKILL.md` link. The captured name is a skill
///   by construction, so it is recorded without a graph-membership check.
/// - `depends_on`: a handoff phrasing the skills actually use, `after <skill>` or
///   `before <skill>`, where `<skill>` is a known skill name.
/// - `handoff_to`: an explicit next-skill directive, `handoff.next_skill = <skill>` or
///   `Next: <skill>`, where `<skill>` is a known skill name.
/// - `enforces`: a `CONVENTIONS.md` reference, with an optional section.
///
/// The `HARD GATE` text is no longer mined into a relation. A gate is a property of the
/// skill, not an edge to another node, and the old first-token grab produced garbage
/// targets like `Do` and `this`.
fn mine_relations(graph: &mut SkillGraph, skill: &ParsedSkill, known: &BTreeSet<String>) {
    let prose = &skill.raw_prose;
    let description = skill
        .frontmatter
        .get("description")
        .and_then(scalar_string)
        .unwrap_or_default();

    // The description and the prose together, so a handoff stated in either is seen. The
    // description carries the "Use it after X, before Y" triggers; the prose carries the
    // "Next:" and "handoff.next_skill" directives.
    let combined = format!("{description}\n{prose}");

    // references: an explicit skills/<name>/SKILL.md link. The path form guarantees a
    // skill name, so it is not filtered against `known` (a referenced skill can live
    // outside a single-skill build).
    for caps in skill_ref_re().captures_iter(prose) {
        push_unique(
            graph,
            GraphRelation::new(&skill.name, &caps[1], "references"),
        );
    }

    // depends_on: "after <skill>" or "before <skill>", validated against known names.
    for caps in handoff_edge_re().captures_iter(&combined) {
        let target = caps[2].to_string();
        if target != skill.name && known.contains(&target) {
            push_unique(
                graph,
                GraphRelation::new(&skill.name, &target, "depends_on"),
            );
        }
    }

    // handoff_to: an explicit next-skill directive, validated against known names.
    for caps in next_skill_re().captures_iter(&combined) {
        let target = caps[1].to_string();
        if target != skill.name && known.contains(&target) {
            push_unique(
                graph,
                GraphRelation::new(&skill.name, &target, "handoff_to"),
            );
        }
    }

    // enforces: a CONVENTIONS.md reference, with an optional section.
    for caps in conventions_re().captures_iter(prose) {
        let target = match caps.get(1) {
            Some(section) => format!("CONVENTIONS.md §{}", section.as_str()),
            None => "CONVENTIONS.md".to_string(),
        };
        push_unique(graph, GraphRelation::new(&skill.name, &target, "enforces"));
    }
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

/// An explicit `skills/<name>/SKILL.md` reference.
fn skill_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compile_static(r"(?i)see\s+skills/([a-z][a-z0-9-]+)/SKILL\.md"))
}

/// A handoff phrasing: `after <skill>` or `before <skill>`. The skill name may be wrapped
/// in backticks. Group 2 is the skill name.
fn handoff_edge_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compile_static(r"(?i)\b(after|before)\s+`?([a-z][a-z0-9-]+)`?"))
}

/// An explicit next-skill directive: `Next: <skill>`, `next_skill = <skill>`, or
/// `next_skill: <skill>`. Group 1 is the skill name.
fn next_skill_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compile_static(r"(?i)(?:next_skill\s*[:=]|next:)\s*`?([a-z][a-z0-9-]+)`?"))
}

/// A `CONVENTIONS.md` reference, with an optional section after `§` or `#`.
fn conventions_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compile_static(r"(?i)CONVENTIONS\.md(?:\s*[§#]\s*(\S+))?"))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `graph`.
#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
