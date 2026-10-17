//! Tests for the cockpit serde models.
use super::*;
use std::path::PathBuf;

const STATE_FIXTURE: &str = r#"{
  "active_flow": null,
  "active_epic": null,
  "active_story": null,
  "bug_id": null,
  "bigpowers_version": "2.88.2",
  "handoff": {"next_skill": null, "context": "some handoff context", "epic": null},
  "metrics": {"story_start": null, "story_end": null, "skill_timings": {"verify-work": {"calls": 1, "total_seconds": 1052.0}}},
  "release": {"ci_verified": true, "last_commit": "3daf6225"},
  "git": {"branch": "main"}
}"#;

const RELEASE_PLAN_FIXTURE: &str = r#"{
  "release": {"version": "2.88.2", "codename": "Foundation", "status": "released"},
  "build_order": ["e45", "e48"],
  "done_epics_summary": {"count": 67},
  "epics": [{"id": "e82", "title": "Fork Innovations", "status": "scoped"}]
}"#;

fn parse_state(json: &str) -> StateFile {
    crate::engine::validate::validate_state(json).expect("parse state fixture")
}

#[test]
fn phase_successors_form_the_lifecycle_loop() {
    for (phase, successor) in [
        (Phase::Discover, Phase::Design),
        (Phase::Design, Phase::Plan),
        (Phase::Plan, Phase::Execute),
        (Phase::Execute, Phase::Review),
        (Phase::Review, Phase::Integrate),
        (Phase::Integrate, Phase::Discover),
    ] {
        assert_eq!(phase.successor(), successor);
    }
}

#[test]
fn state_exposes_typed_accessors() {
    assert_eq!(parse_state(STATE_FIXTURE).git_branch(), Some("main"));
}

#[test]
fn state_preserves_an_arbitrary_unknown_field() {
    assert_eq!(
        parse_state(STATE_FIXTURE).get("bigpowers_version"),
        Some(&serde_json::json!("2.88.2"))
    );
}

#[test]
fn state_roundtrip_is_value_stable() {
    let before: serde_json::Value = serde_json::from_str(STATE_FIXTURE).unwrap();
    let state = parse_state(STATE_FIXTURE);
    let serialized = crate::engine::cockpit_json::to_string(&state.root).unwrap();
    let after: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(before, after);
}

#[test]
fn state_mutation_preserves_every_other_field() {
    let mut state = parse_state(STATE_FIXTURE);
    state.set("git", serde_json::json!({"branch": "feat/new-work"}));
    let serialized = crate::engine::cockpit_json::to_string(&state.root).unwrap();
    let reparsed = parse_state(&serialized);
    assert_eq!(reparsed.git_branch(), Some("feat/new-work"));
    for key in [
        "active_flow",
        "active_epic",
        "active_story",
        "bug_id",
        "bigpowers_version",
        "handoff",
        "metrics",
        "release",
    ] {
        assert!(reparsed.get(key).is_some(), "mutation dropped `{key}`");
    }
    assert_eq!(reparsed.get("active_epic"), Some(&serde_json::Value::Null));
}

#[test]
fn release_plan_preserves_unknown_epics_field() {
    let plan = crate::engine::validate::validate_release_plan(RELEASE_PLAN_FIXTURE).unwrap();
    assert!(plan.get("epics").is_some());
    assert_eq!(plan.build_order().map(Vec::len), Some(2));
    let serialized = crate::engine::cockpit_json::to_string(&plan.root).unwrap();
    assert!(
        crate::engine::validate::validate_release_plan(&serialized)
            .unwrap()
            .get("epics")
            .is_some()
    );
}

#[test]
fn release_plan_append_preserves_other_fields() {
    let mut plan = crate::engine::validate::validate_release_plan(RELEASE_PLAN_FIXTURE).unwrap();
    plan.push_build_order(serde_json::json!("e99"));
    assert_eq!(plan.build_order().map(Vec::len), Some(3));
    assert!(plan.get("epics").is_some());
    assert!(plan.get("release").is_some());
}

#[test]
fn real_state_fixture_roundtrips_when_present() {
    let Some(path) = repo_root_file(".agent/tasks/state.json") else {
        return;
    };
    let json = std::fs::read_to_string(&path).expect("read real state.json");
    let before: serde_json::Value = serde_json::from_str(&json).unwrap();
    let state = parse_state(&json);
    let serialized = crate::engine::cockpit_json::to_string(&state.root).unwrap();
    let after: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(before, after, "real state.json must round-trip");
}

#[test]
fn ontology_roundtrips_against_fixture() {
    // Requirement 4.3: the ontology model round-trips against the design's example.
    let yaml = r#"
version: '1'
domain: order-fulfillment
last_updated: 2026-07-26T00:00:00Z
entities:
  - name: Order
    description: A customer purchase moving through fulfillment.
    primary_key: order_id
    invariants:
      - 'total_cents >= 0'
    states: [draft, placed, shipped, cancelled]
    transitions:
      draft: [placed, cancelled]
      placed: [shipped, cancelled]
      shipped: []
      cancelled: []
    prohibited_aliases: [is_deleted, order_no, purchase]
constraints:
  - id: C-01
    rule: 'Soft deletion uses deleted_at, never a boolean flag.'
  - id: C-02
    rule: 'Boolean state flags (is_*) are prohibited; model states explicitly.'
"#;
    let ontology: Ontology = crate::engine::yaml::from_str(yaml).expect("parse ontology fixture");
    assert_eq!(ontology.domain, "order-fulfillment");
    assert_eq!(ontology.entities.len(), 1);
    assert_eq!(
        ontology.entities[0].prohibited_aliases,
        vec!["is_deleted", "order_no", "purchase"]
    );
    assert_eq!(ontology.constraints[0].id, "C-01");

    // Round-trip preserves the model.
    let serialized = crate::engine::yaml::to_string(&ontology).expect("serialize ontology");
    let reparsed: Ontology = crate::engine::yaml::from_str(&serialized).expect("re-parse ontology");
    assert_eq!(ontology, reparsed);
}

/// Resolve a path relative to the repo root (one level above the crate manifest dir).
/// Returns `None` when the file is absent, so fixture-backed tests stay portable.
fn repo_root_file(relative: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    let path = repo_root.join(relative);
    path.exists().then_some(path)
}

// ── Empty stub ontology (issue #143, Requirement 4.9) ─────────────────────────────────

#[test]
fn empty_stub_is_the_empty_stub() {
    assert!(Ontology::empty_stub().is_empty_stub());
}

#[test]
fn empty_stub_round_trips_as_the_empty_stub() {
    // The serialized stub, re-parsed, is still the empty stub. This pins the resource seed
    // (which serializes empty_stub) to the generate-tool stub check (which parses and calls
    // is_empty_stub), so the two cannot drift.
    let yaml = crate::engine::yaml::to_string(&Ontology::empty_stub()).expect("serialize stub");
    let parsed: Ontology = crate::engine::yaml::from_str(&yaml).expect("parse stub");
    assert!(parsed.is_empty_stub());
}

#[test]
fn a_real_ontology_is_not_the_empty_stub() {
    let mut ontology = Ontology::empty_stub();
    ontology.domain = "orders".to_string();
    assert!(!ontology.is_empty_stub());

    let mut with_entity = Ontology::empty_stub();
    with_entity.entities.push(Entity {
        name: "Order".to_string(),
        description: "An order.".to_string(),
        primary_key: "id".to_string(),
        invariants: Vec::new(),
        states: Vec::new(),
        transitions: BTreeMap::new(),
        prohibited_aliases: Vec::new(),
    });
    assert!(!with_entity.is_empty_stub());
}

#[test]
fn is_empty_stub_ignores_the_timestamp() {
    // A stub with only a timestamp set is still the not-yet-defined stub.
    let mut stub = Ontology::empty_stub();
    stub.last_updated = "2026-09-15T00:00:00Z".to_string();
    assert!(stub.is_empty_stub());
}
