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

/// Seed a legacy bigpowers file under `specs/` with the given content.
fn seed_legacy(root: &Path, name: &str, content: &str) {
    let dir = root.join("specs");
    fs::create_dir_all(&dir).expect("create specs dir");
    fs::write(dir.join(name), content).expect("write legacy file");
}

#[test]
fn advance_phase_reads_legacy_state_and_writes_to_agent() {
    // Requirement 2.9: a legacy specs/ cockpit is read, and the write goes to .agent/.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_legacy(
        root,
        "state.yaml",
        "active_epic: e01\nbigpowers_version: 2.88.2\n",
    );

    advance_phase(
        root,
        Phase::Discover,
        Phase::Design,
        "Modeled the domain.",
        "",
    )
    .expect("advance");

    // The write landed under .agent/, carrying the legacy fields (Requirements 2.11, 2.13).
    let written = fs::read_to_string(state_path(root)).expect("read agent state");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse");
    assert_eq!(value.get("phase").and_then(|v| v.as_str()), Some("design"));
    assert_eq!(
        value.get("active_epic").and_then(|v| v.as_str()),
        Some("e01")
    );
    assert_eq!(
        value.get("bigpowers_version").and_then(|v| v.as_str()),
        Some("2.88.2")
    );

    // The legacy file is never mutated (Requirement 1.4).
    let legacy = fs::read_to_string(legacy_state_path(root)).expect("read legacy");
    assert_eq!(legacy, "active_epic: e01\nbigpowers_version: 2.88.2\n");
}

#[test]
fn record_task_reads_legacy_plan_and_writes_to_agent() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_legacy(root, "release-plan.yaml", "build_order:\n- e01\n");

    record_task(
        root,
        Some("e80"),
        Some("epic"),
        "Wire the gate",
        "cargo test",
    )
    .expect("record");

    let written = fs::read_to_string(release_plan_path(root)).expect("read agent plan");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse");
    assert!(value.get("tasks").and_then(|v| v.as_sequence()).is_some());
    // The legacy build_order carried over into the .agent/ file.
    assert!(value.get("build_order").is_some());

    // The legacy file is never mutated (Requirement 1.4).
    let legacy = fs::read_to_string(root.join("specs").join("release-plan.yaml")).expect("legacy");
    assert_eq!(legacy, "build_order:\n- e01\n");
}

#[test]
fn advance_phase_writes_target_phase() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_state(root, "active_epic: e01\nbigpowers_version: 2.88.2\n");

    advance_phase(
        root,
        Phase::Discover,
        Phase::Design,
        "Modeled the domain.",
        "M specs/state.yaml",
    )
    .expect("advance");

    let written = fs::read_to_string(state_path(root)).expect("read state");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse state");
    assert_eq!(value.get("phase").and_then(|v| v.as_str()), Some("design"));
    assert_eq!(
        value
            .get("handoff")
            .and_then(|h| h.get("artifacts_summary"))
            .and_then(|v| v.as_str()),
        Some("Modeled the domain.")
    );
}

#[test]
fn advance_phase_bootstraps_null_and_maps_legacy_phases() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();

    seed_state(root, "phase: null\n");
    advance_phase(root, Phase::Discover, Phase::Design, "modeled", "").expect("bootstrap");
    let null_bootstrap = fs::read_to_string(state_path(root)).expect("read state");
    assert!(null_bootstrap.contains("phase: design"));

    seed_state(root, "phase: build\n");
    advance_phase(root, Phase::Execute, Phase::Review, "built", "").expect("legacy mapping");
    let legacy_advance = fs::read_to_string(state_path(root)).expect("read state");
    assert!(legacy_advance.contains("phase: review"));
}

#[test]
fn advance_phase_rejects_stale_and_nonadjacent_transitions_without_writing() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let original = "phase: design\nactive_epic: e01\n";
    seed_state(root, original);

    for (from, to) in [
        (Phase::Discover, Phase::Plan),
        (Phase::Design, Phase::Execute),
        (Phase::Design, Phase::Discover),
    ] {
        let error = advance_phase(root, from, to, "summary", "").expect_err("must reject");
        assert!(matches!(error, CockpitError::Transition { .. }));
        assert_eq!(
            fs::read_to_string(state_path(root)).expect("read unchanged state"),
            original
        );
    }
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

    advance_phase(root, Phase::Discover, Phase::Design, "Built the slice.", "").expect("advance");

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
    advance_phase(root, Phase::Discover, Phase::Design, "Kicked off.", "").expect("advance");
    assert!(state_path(root).is_file());
}

#[test]
fn record_task_appends_to_release_plan() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_plan(root, "build_order:\n- e01\n");

    record_task(
        root,
        Some("e80"),
        Some("epic"),
        "Wire the gate",
        "cargo test",
    )
    .expect("record");

    let written = fs::read_to_string(release_plan_path(root)).expect("read plan");
    let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse plan");
    let tasks = value
        .get("tasks")
        .and_then(|v| v.as_sequence())
        .expect("tasks");
    assert_eq!(tasks.len(), 1);
    // The task carries the neutral grouping key in place of epic_id (Requirement 4.9).
    assert_eq!(
        tasks[0].get("group_id").and_then(|v| v.as_str()),
        Some("e80")
    );
    assert_eq!(
        tasks[0].get("group_kind").and_then(|v| v.as_str()),
        Some("epic")
    );
    assert!(tasks[0].get("epic_id").is_none());
    // The existing build_order is preserved.
    assert!(value.get("build_order").is_some());
}

#[test]
fn record_task_appends_second_task() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_plan(root, "release:\n  version: 1.0.0\n");

    record_task(root, Some("e80"), Some("epic"), "First", "make a").expect("first");
    record_task(root, None, None, "Second", "make b").expect("second");

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

    let outcome = advance_phase(root, Phase::Review, Phase::Integrate, "summary", "");
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

    advance_phase(root, Phase::Discover, Phase::Design, "planned", "").expect("advance");

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

#[test]
fn read_active_task_returns_the_value_when_present() {
    // A non-empty active_task is returned as the scope-fallback task (#331).
    let dir = tempdir().expect("temp dir");
    seed_state(
        dir.path(),
        "active_task: fix the parser off-by-one\nphase: null\n",
    );

    let task = read_active_task(dir.path()).expect("reads");
    assert_eq!(task.as_deref(), Some("fix the parser off-by-one"));
}

#[test]
fn read_active_task_returns_none_when_null_or_absent() {
    // A null active_task, and an absent state file, both resolve to None.
    let dir = tempdir().expect("temp dir");
    seed_state(dir.path(), "active_task: null\nphase: null\n");
    assert_eq!(read_active_task(dir.path()).expect("reads"), None);

    let empty = tempdir().expect("temp dir");
    assert_eq!(read_active_task(empty.path()).expect("reads"), None);
}

#[test]
fn read_active_task_treats_an_empty_string_as_none() {
    // A blank active_task is not a usable scope definition, so it resolves to None.
    let dir = tempdir().expect("temp dir");
    seed_state(dir.path(), "active_task: \"   \"\nphase: null\n");
    assert_eq!(read_active_task(dir.path()).expect("reads"), None);
}
