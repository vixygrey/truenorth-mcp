//! Tests for the resources layer (task 14.3).
//!
//! Included from `resources/mod.rs` via `#[path]`, so `super` is the resources module.
//!
//! Requirements: 5.5, 5.7.

use super::*;
use std::fs;
use tempfile::tempdir;

/// Seed a file under the repo root at `rel`.
fn seed(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).expect("parent dir");
    let content = if rel.ends_with(".json") {
        let value: saneyaml::Value = crate::engine::yaml::from_str(content).expect("parse seed");
        serde_json::to_string(&value).expect("serialize seed")
    } else {
        content.to_string()
    };
    fs::write(path, content).expect("write file");
}

#[test]
fn uri_and_backing_paths_are_stable() {
    assert_eq!(
        ResourceDoc::from_uri("truenorth://state"),
        Some(ResourceDoc::State)
    );
    assert_eq!(
        ResourceDoc::from_uri("truenorth://cockpit"),
        Some(ResourceDoc::Cockpit)
    );
    assert_eq!(
        ResourceDoc::from_uri("truenorth://ontology"),
        Some(ResourceDoc::Ontology)
    );
    assert_eq!(
        ResourceDoc::from_uri("truenorth://conventions"),
        Some(ResourceDoc::Conventions)
    );
    assert_eq!(ResourceDoc::from_uri("truenorth://nope"), None);
}

#[test]
fn reject_excluded_or_secret_refuses_telemetry_and_secret_paths() {
    let root = std::path::Path::new("/repo");

    // A telemetry path is excluded from reads (Requirement 1.10).
    let telemetry = root.join(".agent").join("telemetry").join("runs.yml");
    assert!(reject_excluded_or_secret(root, &telemetry).is_err());

    // A secret-denylist path is refused (Requirement 1.7).
    let secret = root.join(".agent").join("config").join(".env");
    assert!(reject_excluded_or_secret(root, &secret).is_err());

    // An ordinary cockpit path is allowed.
    let ok = root.join(".agent").join("tasks").join("state.json");
    assert!(reject_excluded_or_secret(root, &ok).is_ok());
}

#[test]
fn read_returns_current_on_disk_content() {
    // Requirement 5.5: disk is the source of truth.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed(root, ".agent/tasks/state.json", "active_epic: e01\n");

    let content = ResourceDoc::State.read_current(root).expect("read state");
    assert!(content.contains("\"active_epic\":\"e01\""));

    // A later edit is reflected on the next read.
    seed(root, ".agent/tasks/state.json", "active_epic: e02\n");
    let updated = ResourceDoc::State.read_current(root).expect("read updated");
    assert!(updated.contains("\"active_epic\":\"e02\""));
}

#[test]
fn agent_files_are_the_source_of_truth_for_state_and_cockpit() {
    // The runtime reads only the .agent/ JSON cockpit; there is no legacy specs/ fallback
    // for state or release-plan (the three mutable cockpit documents are JSON-only).
    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    seed(root, ".agent/tasks/state.json", "active_epic: current\n");
    let state = ResourceDoc::State.read_current(root).expect("read state");
    assert!(state.contains("current"));

    seed(
        root,
        ".agent/tasks/release-plan.json",
        "build_order:\n- current\n",
    );
    let cockpit = ResourceDoc::Cockpit
        .read_current(root)
        .expect("read cockpit");
    assert!(cockpit.contains("current"));

    // Ontology keeps its legacy specs/ fallback (out of scope for the cockpit JSON cutover).
    seed(root, "specs/ontology.yaml", "domain: legacy\n");
    seed(root, ".agent/ontology.yml", "domain: current\n");
    let ontology = ResourceDoc::Ontology
        .read_current(root)
        .expect("read ontology");
    assert!(ontology.contains("current"));
    assert!(!ontology.contains("legacy"));
}

#[test]
fn absent_backing_file_is_not_found() {
    let dir = tempdir().expect("temp dir");
    let error = ResourceDoc::State
        .read_current(dir.path())
        .expect_err("absent");
    assert!(matches!(error, ResourceReadError::NotFound(_)));
}

#[test]
fn malformed_state_is_invalid() {
    // Requirement 5.7: a malformed backing file yields an Invalid read error.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed(root, ".agent/tasks/state.json", "git: not-a-mapping\n");
    let error = ResourceDoc::State
        .read_current(root)
        .expect_err("malformed");
    assert!(matches!(error, ResourceReadError::Invalid(_)));
}

#[test]
fn reading_absent_ontology_creates_it_under_agent() {
    // Requirement 2.4: an absent ontology is created under .agent/ on first read.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    let content = ResourceDoc::Ontology
        .read_current(root)
        .expect("create-on-read");
    // The created file parses as a YAML document with the seeded shape.
    let value: saneyaml::Value = crate::engine::yaml::from_str(&content).expect("parse seed");
    assert!(value.get("entities").is_some());

    // The file now exists under .agent/, not specs/.
    assert!(root.join(".agent/ontology.yml").is_file());
    assert!(!root.join("specs/ontology.yaml").exists());
}

#[test]
fn ontology_reads_a_legacy_specs_file_when_agent_file_is_absent() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed(root, "specs/ontology.yaml", "domain: legacy-orders\n");

    let content = ResourceDoc::Ontology
        .read_current(root)
        .expect("read legacy ontology");

    assert!(content.contains("domain: legacy-orders"));
    assert!(!root.join(".agent/ontology.yml").exists());
}

#[test]
fn ontology_create_on_read_is_idempotent_and_reads_edits() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    // First read seeds the file.
    ResourceDoc::Ontology.read_current(root).expect("seed");

    // A human edit to the .agent/ file is reflected on the next read.
    seed(
        root,
        ".agent/ontology.yml",
        "version: '1'\ndomain: orders\n",
    );
    let content = ResourceDoc::Ontology.read_current(root).expect("read edit");
    assert!(content.contains("domain: orders"));
}

#[test]
fn conventions_serves_raw_markdown() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed(root, "CONVENTIONS.md", "# Conventions\n\nBe kind.\n");
    let content = ResourceDoc::Conventions
        .read_current(root)
        .expect("read conventions");
    assert!(content.contains("# Conventions"));
}

#[test]
fn cache_retains_last_good_on_parse_failure() {
    // Requirement 5.7: a failed read retains the last successfully parsed content.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let cache = ResourceCache::new();

    // First read succeeds and populates the cache.
    seed(root, ".agent/tasks/state.json", "active_epic: e01\n");
    let good = cache.read(ResourceDoc::State, root).expect("first read");
    assert!(good.contains("e01"));
    assert_eq!(cache.last_good(ResourceDoc::State), Some(good));

    // The file breaks on disk. The read errors, but the cache keeps the last good.
    seed(root, ".agent/tasks/state.json", "git: not-a-mapping\n");
    let error = cache
        .read(ResourceDoc::State, root)
        .expect_err("broken read");
    assert!(matches!(error, ResourceReadError::Invalid(_)));
    assert!(
        cache
            .last_good(ResourceDoc::State)
            .expect("retained")
            .contains("e01"),
        "the last-good content is retained after a parse failure"
    );
}

#[test]
fn cache_updates_last_good_on_success() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let cache = ResourceCache::new();

    seed(root, ".agent/tasks/state.json", "active_epic: e01\n");
    cache.read(ResourceDoc::State, root).expect("read one");
    seed(root, ".agent/tasks/state.json", "active_epic: e02\n");
    cache.read(ResourceDoc::State, root).expect("read two");

    assert!(
        cache
            .last_good(ResourceDoc::State)
            .expect("cached")
            .contains("e02")
    );
}

#[test]
fn one_broken_resource_does_not_block_others() {
    // Requirement 5.7: other resources keep serving when one is broken.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed(root, ".agent/tasks/state.json", "git: not-a-mapping\n");
    seed(root, ".agent/ontology.yml", "version: '1'\ndomain: d\n");

    assert!(ResourceDoc::State.read_current(root).is_err());
    assert!(ResourceDoc::Ontology.read_current(root).is_ok());
}

// ── Ontology feature gate (issue #142, Property 17, Property 18) ──────────────────────

/// The enabled and disabled flags, for readable test setup.
const ENABLED: Features = Features {
    ontology: true,
    jev: false,
    senior_developer: true,
};
const DISABLED: Features = Features {
    ontology: false,
    jev: false,
    senior_developer: true,
};

#[test]
fn served_resources_includes_ontology_when_enabled() {
    let served = served_resources(ENABLED);
    assert!(served.contains(&ResourceDoc::Ontology));
    assert_eq!(served.len(), ALL_RESOURCES.len());
}

#[test]
fn served_resources_excludes_ontology_when_disabled() {
    let served = served_resources(DISABLED);
    assert!(!served.contains(&ResourceDoc::Ontology));
    // Only the ontology resource is dropped; the rest stay served (Requirement 3.6).
    assert_eq!(served.len(), ALL_RESOURCES.len() - 1);
    for doc in [
        ResourceDoc::State,
        ResourceDoc::Cockpit,
        ResourceDoc::Conventions,
        ResourceDoc::Adr,
    ] {
        assert!(served.contains(&doc), "{doc:?} must stay served");
    }
}

#[test]
fn served_from_uri_resolves_ontology_only_when_enabled() {
    assert_eq!(
        served_from_uri("truenorth://ontology", ENABLED),
        Some(ResourceDoc::Ontology)
    );
    // Disabled: the ontology URI is unknown, so the server maps it to unknown-resource
    // (Requirement 3.3).
    assert_eq!(served_from_uri("truenorth://ontology", DISABLED), None);
}

#[test]
fn served_from_uri_leaves_other_resources_resolvable_when_disabled() {
    assert_eq!(
        served_from_uri("truenorth://state", DISABLED),
        Some(ResourceDoc::State)
    );
    assert_eq!(
        served_from_uri("truenorth://conventions", DISABLED),
        Some(ResourceDoc::Conventions)
    );
}

#[test]
fn disabled_ontology_uri_resolution_seeds_no_file() {
    // Requirement 3.4: a disabled ontology URI resolves to None, so read_current never
    // runs and no .agent/ontology.yml is created.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    assert_eq!(served_from_uri("truenorth://ontology", DISABLED), None);
    assert!(
        !root.join(".agent/ontology.yml").exists(),
        "no ontology file is seeded when the feature is disabled"
    );
}
