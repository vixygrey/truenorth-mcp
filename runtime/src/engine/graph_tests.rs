//! Tests for the skill graph (task 8b).
//!
//! Included from `graph.rs` via `#[path]`, so `super` is the graph module.
//!
//! Requirements: 7.8, 7.9.

use super::*;
use crate::engine::skill::RawSkill;
use crate::engine::skill_parser::parse_skill;
use tempfile::tempdir;

/// Parse markdown into a `ParsedSkill` under a given name.
fn parsed(name: &str, markdown: &str) -> ParsedSkill {
    let raw = RawSkill {
        name: name.to_string(),
        path: std::path::PathBuf::from(format!("skills/{name}/SKILL.md")),
        markdown: markdown.to_string(),
        truncated: false,
    };
    parse_skill(&raw)
}

#[test]
fn builds_entity_with_description_observation() {
    // The skill graph records only the `description` observation. The vendor `model` tier
    // and the `effort` tag are dropped from skills (model-agnostic decoupling, #52), so
    // they never become observations even when present in stale frontmatter.
    let skill = parsed(
        "develop-tdd",
        "---\nmodel: sonnet\neffort: standard\ndescription: TDD loop.\n---\n\n# TDD\n",
    );
    let graph = build_graph(&[skill]);
    let entity = graph.entities.get("develop-tdd").expect("entity built");
    assert_eq!(entity.entity_type, "Skill");
    assert!(
        entity
            .observations
            .contains(&"description: TDD loop.".to_string())
    );
    assert!(
        !entity.observations.iter().any(|o| o.starts_with("model:")),
        "the vendor model tier must not become an observation"
    );
    assert!(
        !entity.observations.iter().any(|o| o.starts_with("effort:")),
        "the effort tag must not become an observation"
    );
}

#[test]
fn mines_references_relation() {
    let skill = parsed(
        "develop-tdd",
        "# TDD\n\nSee skills/verify-work/SKILL.md for the gate.\n",
    );
    let graph = build_graph(&[skill]);
    let refs: Vec<&GraphRelation> = graph
        .relations
        .iter()
        .filter(|r| r.relation_type == "references")
        .collect();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].from, "develop-tdd");
    assert_eq!(refs[0].to, "verify-work");
}

#[test]
fn mines_enforces_relation_with_section() {
    let skill = parsed(
        "develop-tdd",
        "# TDD\n\nObey CONVENTIONS.md § Testing here.\n",
    );
    let graph = build_graph(&[skill]);
    let enforces: Vec<&GraphRelation> = graph
        .relations
        .iter()
        .filter(|r| r.relation_type == "enforces")
        .collect();
    assert_eq!(enforces.len(), 1);
    assert_eq!(enforces[0].to, "CONVENTIONS.md §Testing");
}

#[test]
fn jsonl_round_trips() {
    let skill = parsed(
        "develop-tdd",
        "---\nmodel: sonnet\n---\n\n# TDD\n\nSee skills/verify-work/SKILL.md.\n",
    );
    let graph = build_graph(&[skill]);
    let jsonl = to_jsonl(&graph);
    let reparsed = from_jsonl(&jsonl);
    assert_eq!(graph, reparsed);
}

#[test]
fn save_and_load_round_trip() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("truenorth-mcp").join("graph.jsonl");
    let skill = parsed("develop-tdd", "# TDD\n");
    let graph = build_graph(&[skill]);

    save_graph(&path, &graph).expect("save graph");
    let loaded = load_graph(&path);
    assert_eq!(graph, loaded);
}

#[test]
fn load_absent_file_is_empty_graph() {
    let dir = tempdir().expect("temp dir");
    let loaded = load_graph(&dir.path().join("missing.jsonl"));
    assert!(loaded.entities.is_empty());
    assert!(loaded.relations.is_empty());
}

#[test]
fn from_jsonl_skips_malformed_lines() {
    let text = "{\"type\":\"entity\",\"name\":\"a\",\"entityType\":\"Skill\",\"observations\":[]}\nnot json\n";
    let graph = from_jsonl(text);
    assert_eq!(graph.entities.len(), 1);
    assert!(graph.entities.contains_key("a"));
}

#[test]
fn search_and_open_nodes() {
    // The graph records the description observation. Search matches on it. The vendor
    // model tier is no longer an observation, so search by description, not by a model.
    let skill = parsed(
        "develop-tdd",
        "---\ndescription: The red-green-refactor loop.\n---\n\n# TDD\n",
    );
    let graph = build_graph(&[skill]);

    let hits = search_nodes(&graph, "red-green-refactor");
    assert_eq!(hits.len(), 1);

    let opened = open_nodes(&graph, &["develop-tdd".to_string(), "absent".to_string()]);
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].name, "develop-tdd");
}

#[test]
fn handoff_chain_follows_edges_and_stops_on_cycle() {
    // Build a graph by hand with a handoff cycle a -> b -> a.
    let mut graph = SkillGraph::default();
    graph
        .relations
        .push(GraphRelation::new("a", "b", "handoff_to"));
    graph
        .relations
        .push(GraphRelation::new("b", "a", "handoff_to"));
    let chain = handoff_chain(&graph, "a");
    // a, then b, then stop because a is already seen.
    assert_eq!(chain, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn forward_and_reverse_deps() {
    let mut graph = SkillGraph::default();
    graph
        .relations
        .push(GraphRelation::new("planner", "builder", "depends_on"));
    assert_eq!(forward_deps(&graph, "planner"), vec!["builder".to_string()]);
    assert_eq!(reverse_deps(&graph, "builder"), vec!["planner".to_string()]);
}
