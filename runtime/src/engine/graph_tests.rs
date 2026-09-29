//! Tests for the skill graph (task 8b).
//!
//! Included from `graph.rs` via `#[path]`, so `super` is the graph module.
//!
//! Requirements: 7.8, 7.9.

use super::*;
use crate::engine::skill::{RawSkill, discover_skills, read_skill_raw};
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
    let graph = build_graph(&[skill, parsed("verify-work", "# Verify\n")]);
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
fn write_and_load_round_trip() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("truenorth-mcp").join("graph.jsonl");
    let skill = parsed("develop-tdd", "# TDD\n");
    let graph = build_graph(&[skill]);

    // Persist through the same serialize path the catalog tool uses (`to_jsonl` plus a
    // guarded write), then confirm `load_graph` reads it back from disk.
    std::fs::create_dir_all(path.parent().expect("graph parent")).expect("graph dir");
    std::fs::write(&path, to_jsonl(&graph)).expect("write graph");
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

#[test]
fn dependency_direction_matches_execution_order() {
    let research = parsed(
        "research-first",
        "---\ndescription: Use it after survey-context and before elaborate-spec.\n---\n\n# Research First\n",
    );
    let survey = parsed("survey-context", "# Survey Context\n");
    let elaborate = parsed("elaborate-spec", "# Elaborate Spec\n");
    let graph = build_graph(&[research, survey, elaborate]);

    assert_eq!(
        forward_deps(&graph, "research-first"),
        vec!["survey-context".to_string()]
    );
    assert_eq!(
        forward_deps(&graph, "elaborate-spec"),
        vec!["research-first".to_string()]
    );
}

#[test]
fn mines_canonical_invocation_forms() {
    let router = parsed(
        "orchestrate-project",
        "# Orchestrate\n\nRun `build-group`, invoke `verify-work`, then route to `release-branch`.\n",
    );
    let graph = build_graph(&[
        router,
        parsed("build-group", "# Build\n"),
        parsed("verify-work", "# Verify\n"),
        parsed("release-branch", "# Release\n"),
    ]);
    let invoked: BTreeSet<String> = graph
        .relations
        .iter()
        .filter(|r| r.from == "orchestrate-project" && r.relation_type == "invokes")
        .map(|r| r.to.clone())
        .collect();
    assert_eq!(
        invoked,
        BTreeSet::from([
            "build-group".to_string(),
            "release-branch".to_string(),
            "verify-work".to_string(),
        ])
    );
}

#[test]
fn mines_hand_off_to_as_a_handoff() {
    let plan = parsed("plan-release", "# Plan\n\nHand off to `slice-tasks`.\n");
    let graph = build_graph(&[plan, parsed("slice-tasks", "# Slice\n")]);
    assert_eq!(
        handoff_chain(&graph, "plan-release"),
        vec!["plan-release".to_string(), "slice-tasks".to_string()]
    );
}

#[test]
fn classifies_remaining_exact_skill_mentions_as_references() {
    let delegator = parsed(
        "delegate-task",
        "# Delegate\n\nDistinct from `dispatch-agents`; choose based on concurrency.\n",
    );
    let graph = build_graph(&[delegator, parsed("dispatch-agents", "# Dispatch\n")]);
    assert!(graph.relations.iter().any(|r| {
        r.from == "delegate-task" && r.to == "dispatch-agents" && r.relation_type == "references"
    }));
}

#[test]
fn reports_unresolved_canonical_targets_and_omits_stale_edges() {
    let skill = parsed(
        "develop-tdd",
        "# TDD\n\nSee skills/removed-skill/SKILL.md and ../renamed-skill/SKILL.md.\n",
    );
    let result = build_graph_report(&[skill]);
    assert_eq!(result.diagnostics.unresolved_mentions.len(), 2);
    assert!(result.graph.relations.is_empty());
}

#[test]
fn reports_suspicious_isolated_orchestrators() {
    let skill = parsed(
        "orchestrate-project",
        "---\ndescription: Orchestrate a project workflow.\n---\n\n# Orchestrate\n",
    );
    let result = build_graph_report(&[skill]);
    assert_eq!(
        result.diagnostics.suspicious_isolated_orchestrators,
        vec!["orchestrate-project".to_string()]
    );
}

#[test]
fn mines_handoff_to_from_next_skill_directive() {
    // A "Next: <skill>" directive becomes a handoff_to edge when the target is known.
    let survey = parsed(
        "survey-context",
        "# Survey Context\n\n## Handoff\n\nGate: READY. Next: plan-work.\n",
    );
    let plan = parsed("plan-work", "# Plan Work\n");
    let graph = build_graph(&[survey, plan]);

    let chain = handoff_chain(&graph, "survey-context");
    assert_eq!(
        chain,
        vec!["survey-context".to_string(), "plan-work".to_string()]
    );
}

#[test]
fn does_not_mine_a_relation_to_an_unknown_target() {
    // "after lunch" must not become a depends_on edge, because `lunch` is not a skill.
    // This is the #239 regression guard: a target that is not a known skill is dropped.
    let skill = parsed(
        "develop-tdd",
        "---\ndescription: Write the code after lunch and before dinner.\n---\n\n# TDD\n",
    );
    let graph = build_graph(&[skill]);
    assert!(
        graph.relations.is_empty(),
        "no relation should point at a non-skill word, got {:?}",
        graph.relations
    );
}

#[test]
fn hard_gate_text_is_not_mined_into_a_relation() {
    // The old miner grabbed the first word after "HARD GATE:" as a `gates` target, which
    // produced garbage like `gates -> Do`. A gate is a property of the skill, not an edge.
    let skill = parsed(
        "develop-tdd",
        "# TDD\n\n> **HARD GATE**: Do NOT proceed on main. Run kickoff-branch first.\n",
    );
    let graph = build_graph(&[skill]);
    assert!(
        !graph.relations.iter().any(|r| r.relation_type == "gates"),
        "the HARD GATE text must not produce a gates relation, got {:?}",
        graph.relations
    );
    // And the stray words from the gate sentence must not appear as any target.
    for r in &graph.relations {
        assert!(
            !["Do", "this", "read", "NOT", "Run"].contains(&r.to.as_str()),
            "a stray English word leaked as a relation target: {r:?}"
        );
    }
}

#[test]
fn every_skill_relation_target_resolves_to_a_known_entity() {
    // The load-bearing #239 property: over a representative multi-skill graph, every
    // skill-to-skill relation target is a known entity. Only the CONVENTIONS.md enforces
    // target is exempt, because it is not a skill.
    let skills = vec![
        parsed(
            "research-first",
            "---\ndescription: Use it after survey-context and before elaborate-spec.\n---\n\n# Research First\n\nObey CONVENTIONS.md here.\n",
        ),
        parsed(
            "survey-context",
            "# Survey Context\n\nSee skills/plan-work/SKILL.md.\n\n## Handoff\n\nNext: plan-work.\n",
        ),
        parsed("elaborate-spec", "# Elaborate Spec\n"),
        parsed("plan-work", "# Plan Work\n"),
    ];
    let known: std::collections::BTreeSet<String> = skills.iter().map(|s| s.name.clone()).collect();
    let graph = build_graph(&skills);

    for r in &graph.relations {
        if r.to.starts_with("CONVENTIONS.md") {
            continue;
        }
        assert!(
            known.contains(&r.to),
            "relation target `{}` is not a known skill (relation {r:?})",
            r.to
        );
    }
    // Sanity: the graph is not empty, so the assertion above is meaningful.
    assert!(
        graph
            .relations
            .iter()
            .any(|r| r.relation_type == "depends_on"),
        "expected at least one depends_on edge in the representative graph"
    );
}

#[test]
fn checked_in_catalog_has_complete_resolved_graph_contract() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf();
    let entries = discover_skills(&root);
    let skills: Vec<ParsedSkill> = entries
        .iter()
        .map(|entry| read_skill_raw(&root, &entry.name).expect("read checked-in skill"))
        .map(|raw| parse_skill(&raw))
        .collect();
    let result = build_graph_report(&skills);

    assert_eq!(result.graph.entities.len(), entries.len());
    assert!(
        result.diagnostics.unresolved_mentions.is_empty(),
        "unresolved mentions: {:?}",
        result.diagnostics.unresolved_mentions
    );
    assert!(result.diagnostics.unclassified_mentions.is_empty());
    assert!(!result.graph.entities.contains_key("build-epic"));
    for relation in &result.graph.relations {
        if relation.relation_type != "enforces" {
            assert!(
                result.graph.entities.contains_key(&relation.to),
                "stale relation target: {relation:?}"
            );
        }
    }
    for orchestrator in [
        "orchestrate-project",
        "execute-plan",
        "change-request",
        "compose-workflow",
    ] {
        assert!(
            result.graph.relations.iter().any(|relation| {
                relation.from == orchestrator
                    && matches!(relation.relation_type.as_str(), "invokes" | "handoff_to")
            }),
            "{orchestrator} has no outgoing control-flow edge: {:?}",
            result
                .graph
                .relations
                .iter()
                .filter(|relation| relation.from == orchestrator)
                .collect::<Vec<_>>()
        );
    }
    for orchestrator in ["build-group", "release-branch"] {
        assert!(
            result.graph.relations.iter().any(|relation| {
                relation.from == orchestrator
                    && relation.to == "trace-requirement"
                    && relation.relation_type == "invokes"
            }),
            "{orchestrator} must invoke the retained traceability contract"
        );
    }
    assert!(!result.graph.entities.contains_key("gate-trace"));
    assert!(
        result.diagnostics.dependency_cycles.is_empty(),
        "dependency cycles: {:?}",
        result.diagnostics.dependency_cycles
    );
    assert_eq!(
        to_jsonl(&result.graph),
        to_jsonl(&build_graph_report(&skills).graph)
    );
}
