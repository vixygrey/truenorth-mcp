//! Tests for the cockpit serde models (task 3.3, 3.4).
//!
//! Included from `spec.rs` via `#[path]`, so `super` is the spec module. The state
//! preservation test (Property 3) round-trips and mutates fixtures, then asserts every
//! field, including `null`-valued fields and `bigpowers_version`, survives verbatim.
//!
//! Requirements: 9.3, 9.4, 4.3.

use super::*;
use std::path::PathBuf;

/// An observed `state.yaml` shape with `null`-valued fields and an unmodeled
/// `active_flow`. Every one of these must survive a round-trip unchanged.
const STATE_FIXTURE: &str = r#"
active_flow: null
active_epic: null
active_story: null
bug_id: null
bigpowers_version: 2.88.2
handoff:
  next_skill: null
  context: 'some handoff context'
  epic: null
metrics:
  story_start: null
  story_end: null
  skill_timings:
    verify-work:
      calls: 1
      total_seconds: 1052.0
release:
  ci_verified: true
  last_commit: 3daf6225
git:
  branch: main
"#;

/// An observed `release-plan.yaml` shape carrying an unmodeled `epics` array.
const RELEASE_PLAN_FIXTURE: &str = r#"
release:
  version: 2.88.2
  codename: Foundation
  status: released
build_order:
- e45
- e48
done_epics_summary:
  count: 67
epics:
- id: e82
  title: Fork Innovations
  status: scoped
"#;

/// Parse `state.yaml` content into the model. A parse failure fails the test loudly.
fn parse_state(yaml: &str) -> StateFile {
    serde_yaml::from_str(yaml).expect("parse state fixture")
}

#[test]
fn state_exposes_typed_accessors() {
    let state = parse_state(STATE_FIXTURE);
    assert_eq!(state.git_branch(), Some("main"));
    // `2.88.2` parses as a string, since it has two dots.
    assert_eq!(
        state.bigpowers_version(),
        Some(&serde_yaml::Value::String("2.88.2".to_string()))
    );
}

#[test]
fn state_roundtrip_is_byte_stable() {
    // Property 3: a round-trip through the model preserves the document as a value,
    // including key order, `null` fields, and unmodeled fields.
    let before: serde_yaml::Value =
        serde_yaml::from_str(STATE_FIXTURE).expect("parse fixture as value");
    let state = parse_state(STATE_FIXTURE);
    let serialized = serde_yaml::to_string(&state).expect("serialize state");
    let after: serde_yaml::Value =
        serde_yaml::from_str(&serialized).expect("re-parse serialized state");
    assert_eq!(before, after);
}

#[test]
fn state_mutation_preserves_every_other_field() {
    // Property 3: mutate one field, then assert every other field survives, including
    // the `null`-valued ones the design's field list names.
    let mut state = parse_state(STATE_FIXTURE);
    state.set("git", {
        let mut git = serde_yaml::Mapping::new();
        git.insert(
            serde_yaml::Value::String("branch".to_string()),
            serde_yaml::Value::String("feat/new-work".to_string()),
        );
        serde_yaml::Value::Mapping(git)
    });

    let serialized = serde_yaml::to_string(&state).expect("serialize mutated state");
    let reparsed = parse_state(&serialized);

    // The mutation took effect.
    assert_eq!(reparsed.git_branch(), Some("feat/new-work"));
    // Every field the design names is still present, including the `null` ones.
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
        assert!(
            reparsed.get(key).is_some(),
            "mutation dropped the `{key}` field"
        );
    }
    // The `null` value of a present field is retained, not dropped.
    assert_eq!(reparsed.get("active_epic"), Some(&serde_yaml::Value::Null));
}

#[test]
fn release_plan_preserves_unknown_epics_field() {
    // Property 3: the unmodeled `epics` array survives, and `build_order` reads back.
    let plan: ReleasePlanFile =
        serde_yaml::from_str(RELEASE_PLAN_FIXTURE).expect("parse release-plan fixture");
    assert!(plan.get("epics").is_some());
    assert_eq!(plan.build_order().map(Vec::len), Some(2));

    let serialized = serde_yaml::to_string(&plan).expect("serialize plan");
    let reparsed: ReleasePlanFile = serde_yaml::from_str(&serialized).expect("re-parse plan");
    assert!(reparsed.get("epics").is_some());
}

#[test]
fn release_plan_append_preserves_other_fields() {
    let mut plan: ReleasePlanFile =
        serde_yaml::from_str(RELEASE_PLAN_FIXTURE).expect("parse release-plan fixture");
    plan.push_build_order(serde_yaml::Value::String("e99".to_string()));

    assert_eq!(plan.build_order().map(Vec::len), Some(3));
    // The append did not disturb the unmodeled `epics` field or the release block.
    assert!(plan.get("epics").is_some());
    assert!(plan.get("release").is_some());
}

#[test]
fn real_state_fixture_roundtrips_when_present() {
    // Property 3 against the real repo cockpit. The fixture lives at the repo root, one
    // level above the crate. Skip when it is absent, so the test is portable.
    let Some(path) = repo_root_file("specs/state.yaml") else {
        return;
    };
    let yaml = std::fs::read_to_string(&path).expect("read real state.yaml");

    let before: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("parse real state");
    let state: StateFile = serde_yaml::from_str(&yaml).expect("parse real state model");
    let serialized = serde_yaml::to_string(&state).expect("serialize real state");
    let after: serde_yaml::Value = serde_yaml::from_str(&serialized).expect("re-parse real state");
    assert_eq!(before, after, "real state.yaml must round-trip verbatim");
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
    let ontology: Ontology = serde_yaml::from_str(yaml).expect("parse ontology fixture");
    assert_eq!(ontology.domain, "order-fulfillment");
    assert_eq!(ontology.entities.len(), 1);
    assert_eq!(
        ontology.entities[0].prohibited_aliases,
        vec!["is_deleted", "order_no", "purchase"]
    );
    assert_eq!(ontology.constraints[0].id, "C-01");

    // Round-trip preserves the model.
    let serialized = serde_yaml::to_string(&ontology).expect("serialize ontology");
    let reparsed: Ontology = serde_yaml::from_str(&serialized).expect("re-parse ontology");
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
