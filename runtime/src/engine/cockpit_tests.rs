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
    let value: saneyaml::Value = crate::engine::yaml::from_str(content).expect("parse seed");
    let json = serde_json::to_string(&value).expect("serialize seed");
    fs::write(path, json).expect("write state file");
}

/// Seed the relocated release-plan file with the given content.
fn seed_plan(root: &Path, content: &str) {
    let path = release_plan_path(root);
    fs::create_dir_all(path.parent().expect("plan parent")).expect("create tasks dir");
    let value: saneyaml::Value = crate::engine::yaml::from_str(content).expect("parse seed");
    let json = serde_json::to_string(&value).expect("serialize seed");
    fs::write(path, json).expect("write release-plan file");
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
    let value: saneyaml::Value = crate::engine::yaml::from_str(&written).expect("parse state");
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
    assert!(null_bootstrap.contains("\"phase\": \"design\""));

    seed_state(root, "phase: build\n");
    advance_phase(root, Phase::Execute, Phase::Review, "built", "").expect("legacy mapping");
    let legacy_advance = fs::read_to_string(state_path(root)).expect("read state");
    assert!(legacy_advance.contains("\"phase\": \"review\""));
}

#[test]
fn advance_phase_rejects_stale_and_nonadjacent_transitions_without_writing() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    seed_state(root, "phase: design\nactive_epic: e01\n");
    let original = fs::read_to_string(state_path(root)).expect("read seeded state");

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
    let value: saneyaml::Value = crate::engine::yaml::from_str(&written).expect("parse state");
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
    let value: saneyaml::Value = crate::engine::yaml::from_str(&written).expect("parse");
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
    let value: saneyaml::Value = crate::engine::yaml::from_str(&written).expect("parse");
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
    seed_state(root, "git: not-a-mapping\n");
    let malformed = fs::read_to_string(state_path(root)).expect("read seeded state");

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
    assert_eq!(phase_value(Phase::Review), serde_json::json!("review"));
    assert_eq!(
        phase_value(Phase::Integrate),
        serde_json::json!("integrate")
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

#[test]
fn cockpit_reads_reject_directories_without_creating_replacement_files() {
    for (relative, is_state) in [
        (".agent/tasks/state.json", true),
        (".agent/tasks/release-plan.json", false),
    ] {
        let dir = tempdir().expect("temp repo");
        let path = dir.path().join(relative);
        fs::create_dir_all(&path).expect("directory at cockpit path");
        let result = if is_state {
            prepare_advance_phase(dir.path(), Phase::Discover, Phase::Design, "", "")
        } else {
            prepare_record_task(dir.path(), None, None, "task", "cargo test")
        };
        assert!(matches!(result, Err(CockpitError::Io { file, .. }) if file == relative));
        assert!(path.is_dir());
        assert!(!dir.path().join(".agent/tasks/state.json").is_file());
        assert!(!dir.path().join(".agent/tasks/release-plan.json").is_file());
    }
}

#[test]
fn prepared_state_write_rejects_a_concurrent_edit() {
    let dir = tempdir().expect("temp repo");
    seed_state(dir.path(), "phase: discover\n");
    let prepared = prepare_advance_phase(dir.path(), Phase::Discover, Phase::Design, "", "")
        .expect("prepare phase transition");
    seed_state(dir.path(), "phase: discover\nactive_task: newer-task\n");
    let changed = fs::read_to_string(state_path(dir.path())).expect("read concurrent edit");
    assert!(matches!(
        prepared.commit(dir.path()),
        Err(CockpitError::Write(_))
    ));
    assert_eq!(
        fs::read_to_string(state_path(dir.path())).expect("state retained"),
        changed
    );
}
