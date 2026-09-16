//! The skill knowledge graph (ports `graph/types.ts`, `graph/builder.ts`,
//! `graph/store.ts`).
//!
//! `build_graph` mines entity-relation data from parsed skills. The graph persists as
//! JSONL (`saveGraph`), loads back (`loadGraph`), and answers the dependency, search, and
//! open queries the legacy catalog tools expose.
//!
//! Requirements: 7.8, 7.9. Design: Part II §1.

// The graph is consumed by the legacy catalog tools (task 8b). It is unused until they
// wire it, so the module-scoped allow prevents a premature dead-code error under
// `clippy -D warnings`. Remove this allow once task 8b wires the consumer.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use regex::Regex;
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
/// Each skill becomes an entity with a `description` observation. Relations are mined
/// from the joined prose with the legacy regexes: `depends_on`, `gates`, `references`,
/// `enforces`, and `handoff_to`.
pub fn build_graph(skills: &[ParsedSkill]) -> SkillGraph {
    let mut graph = SkillGraph::default();

    for skill in skills {
        let observations = observations_from(skill);
        graph.entities.insert(
            skill.name.clone(),
            GraphEntity::skill(&skill.name, observations),
        );

        mine_relations(&mut graph, skill, &skill.raw_prose);
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

/// Mine the five relation kinds from a skill's prose (ports the builder regexes).
///
/// The prose is the joined paragraph text (`raw_prose`), which already covers every
/// paragraph once. The legacy builder also appended each section's prose, which
/// double-counted every paragraph and produced duplicate relations. This mines each
/// paragraph once, so a single reference yields a single relation.
fn mine_relations(graph: &mut SkillGraph, skill: &ParsedSkill, prose: &str) {
    for caps in handoff_after_re().captures_iter(prose) {
        graph
            .relations
            .push(GraphRelation::new(&caps[1], &caps[2], "depends_on"));
    }

    if let Some(caps) = hard_gate_re().captures(prose)
        && let Some(target) = first_token_re().find(caps[1].trim())
    {
        let cleaned = target.as_str().replace(['`', '\'', '"'], "");
        graph
            .relations
            .push(GraphRelation::new(&skill.name, &cleaned, "gates"));
    }

    for caps in skill_ref_re().captures_iter(prose) {
        graph
            .relations
            .push(GraphRelation::new(&skill.name, &caps[1], "references"));
    }

    for caps in conventions_re().captures_iter(prose) {
        let target = match caps.get(1) {
            Some(section) => format!("CONVENTIONS.md §{}", section.as_str()),
            None => "CONVENTIONS.md".to_string(),
        };
        graph
            .relations
            .push(GraphRelation::new(&skill.name, &target, "enforces"));
    }

    if let Some(description) = skill.frontmatter.get("description").and_then(scalar_string)
        && let Some(caps) = handoff_desc_re().captures(&description)
    {
        graph
            .relations
            .push(GraphRelation::new(&skill.name, &caps[1], "handoff_to"));
    }
}

/// Serialize the graph to JSONL, entities first then relations (ports `saveGraph`).
pub fn to_jsonl(graph: &SkillGraph) -> String {
    let mut lines: Vec<String> = Vec::new();
    for entity in graph.entities.values() {
        lines.push(serde_json::to_string(entity).expect("entity serializes"));
    }
    for relation in &graph.relations {
        lines.push(serde_json::to_string(relation).expect("relation serializes"));
    }
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

// The legacy relation-mining regexes, compiled once.
fn handoff_after_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)run\s+(\S+)\s+after\s+(\S+)").expect("regex compiles"))
}
fn hard_gate_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)HARD GATE:\s*(.+)").expect("regex compiles"))
}
fn first_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\S+-?\S+").expect("regex compiles"))
}
fn skill_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)see\s+skills/(\S+)/SKILL\.md").expect("regex compiles"))
}
fn conventions_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)CONVENTIONS\.md(?:\s*[§#]\s*(\S+))?").expect("regex compiles")
    })
}
fn handoff_desc_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)handoff.*?(\S+-?\S+)").expect("regex compiles"))
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `graph`.
#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
