//! Tests for cockpit read/write orchestration (task 9.2).
//!
//! Included from `cockpit.rs` via `#[path]`, so `super` is the cockpit module.
//!
//! Requirements: 2.3, 2.5, 2.12, 9.3.

use super::*;
use std::fs;
use tempfile::tempdir;

/// Seed the relocated state file with the given content.
fn seed_state(root: &Path, content: &str) {
    let path = state_path(root);
    fs::create_dir_all(path.parent().expect("state parent")).expect("create tasks dir");
    fs::write(path, content).expect("write state file");
}

/// Seed the relocated release-plan file with the given content.
fn seed_plan(root: &Path, content: &str) {
    let path = release_plan_path(root);
    fs::create_dir_all(path.parent().expect("plan parent")).expect("create tasks dir");
    fs::write(path, content).expect("write release-plan file");
}

#[test]
fn advance_phase_writes_target_phase() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_state(root, "active_epic: e01\nbigpowers_version: 2.88.2\n");

    advance_phase(
        root,
        Phase::Review,
        "Hardened the gate.",
        "M specs/state.yaml",
    )
    .expect("advance");

    let written = fs::read_to_string(state_path(root)).expect("read state");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse state");
    assert_eq!(value.get("phase").and_then(|v| v.as_str()), Some("review"));
    assert_eq!(
        value
            .get("handoff")
            .and_then(|h| h.get("artifacts_summary"))
            .and_then(|v| v.as_str()),
        Some("Hardened the gate.")
    );
}

#[test]
fn advance_phase_preserves_other_fields() {
    // Requirement 9.3: every unmodified field, including bigpowers_version, survives.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_state(
        root,
        "active_epic: e01\nbug_id: null\nbigpowers_version: 2.88.2\n",
    );

    advance_phase(root, Phase::Execute, "Built the slice.", "").expect("advance");

    let written = fs::read_to_string(state_path(root)).expect("read state");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse state");
    assert_eq!(
        value.get("active_epic").and_then(|v| v.as_str()),
        Some("e01")
    );
    assert!(value.get("bug_id").is_some());
    assert_eq!(
        value.get("bigpowers_version").and_then(|v| v.as_str()),
        Some("2.88.2")
    );
}

#[test]
fn advance_phase_seeds_state_when_absent() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    advance_phase(root, Phase::Discover, "Kicked off.", "").expect("advance");
    assert!(state_path(root).is_file());
}

#[test]
fn record_task_appends_to_release_plan() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_plan(root, "build_order:\n- e01\n");

    record_task(root, "e80", "Wire the gate", "cargo test").expect("record");

    let written = fs::read_to_string(release_plan_path(root)).expect("read plan");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse plan");
    let tasks = value
        .get("tasks")
        .and_then(|v| v.as_sequence())
        .expect("tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].get("epic_id").and_then(|v| v.as_str()),
        Some("e80")
    );
    // The existing build_order is preserved.
    assert!(value.get("build_order").is_some());
}

#[test]
fn record_task_appends_second_task() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_plan(root, "release:\n  version: 1.0.0\n");

    record_task(root, "e80", "First", "make a").expect("first");
    record_task(root, "e80", "Second", "make b").expect("second");

    let written = fs::read_to_string(release_plan_path(root)).expect("read plan");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse plan");
    let tasks = value
        .get("tasks")
        .and_then(|v| v.as_sequence())
        .expect("tasks");
    assert_eq!(tasks.len(), 2);
    assert!(value.get("release").is_some());
}

#[test]
fn read_rejects_malformed_state_leaving_file_unchanged() {
    // Requirement 2.12 / 9.2: a malformed existing file is rejected and left unchanged.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let malformed = "git: not-a-mapping\n";
    seed_state(root, malformed);

    let outcome = advance_phase(root, Phase::Review, "summary", "");
    assert!(matches!(outcome, Err(CockpitError::Validation(_))));

    // The file on disk is byte-for-byte unchanged.
    let after = fs::read_to_string(state_path(root)).expect("read state");
    assert_eq!(after, malformed);
}

#[test]
fn write_atomic_leaves_no_temp_residue() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_state(root, "active_epic: e01\n");

    advance_phase(root, Phase::Plan, "planned", "").expect("advance");

    // No `.state.yml.*.tmp` residue remains in the tasks directory.
    let tasks = state_path(root)
        .parent()
        .expect("tasks parent")
        .to_path_buf();
    let residue: Vec<_> = fs::read_dir(&tasks)
        .expect("read tasks dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(residue.is_empty(), "atomic write left temp residue");
}

#[test]
fn phase_value_is_kebab_case() {
    assert_eq!(
        phase_value(Phase::Review),
        serde_yaml::Value::String("review".to_string())
    );
    assert_eq!(
        phase_value(Phase::Integrate),
        serde_yaml::Value::String("integrate".to_string())
    );
}
