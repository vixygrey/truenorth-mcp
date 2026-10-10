use std::collections::BTreeMap;
use std::fs;

use tempfile::tempdir;

use super::*;
use crate::engine::workspace_upgrade::manifest::{MANIFEST_VERSION, ManagedFile};

fn desired(path: &str, body: &str) -> DesiredFile {
    DesiredFile {
        path: path.to_string(),
        bytes: body.as_bytes().to_vec(),
        mode: "0644".to_string(),
    }
}

fn installed(entries: &[(&str, &str)]) -> WorkspaceManifest {
    WorkspaceManifest {
        manifest_version: MANIFEST_VERSION.to_string(),
        bundle_version: "old".to_string(),
        workspace_schema_version: "2".to_string(),
        profile: "generic".to_string(),
        skill_sets: vec!["core".to_string()],
        managed: entries
            .iter()
            .map(|(path, body)| {
                (
                    (*path).to_string(),
                    ManagedFile {
                        source_sha256: sha256(body.as_bytes()),
                        mode: "0644".to_string(),
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn planner_reports_every_action_and_preserves_local_content() {
    let repo = tempdir().expect("repo");
    for (path, body) in [
        ("update.txt", "old"),
        ("preserve.txt", "custom"),
        ("conflict.txt", "custom"),
        ("remove.txt", "old"),
        ("collision.txt", "mine"),
    ] {
        fs::write(repo.path().join(path), body).expect("local file");
    }
    let desired: BTreeMap<String, DesiredFile> = [
        desired("add.txt", "new"),
        desired("collision.txt", "bundle"),
        desired("conflict.txt", "new"),
        desired("preserve.txt", "old"),
        desired("update.txt", "new"),
    ]
    .into_iter()
    .map(|file| (file.path.clone(), file))
    .collect();
    let old = installed(&[
        ("update.txt", "old"),
        ("preserve.txt", "old"),
        ("conflict.txt", "old"),
        ("remove.txt", "old"),
    ]);

    let plan = build_plan(
        repo.path(),
        &desired,
        Some(&old),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");
    let actions: BTreeMap<_, _> = plan
        .actions
        .iter()
        .map(|entry| (entry.path.as_str(), entry.action))
        .collect();
    assert_eq!(actions["add.txt"], PlanAction::Add);
    assert_eq!(actions["collision.txt"], PlanAction::Preserve);
    assert_eq!(actions["conflict.txt"], PlanAction::Conflict);
    assert_eq!(actions["preserve.txt"], PlanAction::Preserve);
    assert_eq!(actions["remove.txt"], PlanAction::Remove);
    assert_eq!(actions["update.txt"], PlanAction::Update);
    assert_eq!(
        fs::read_to_string(repo.path().join("conflict.txt")).unwrap(),
        "custom"
    );
    assert!(!plan.next_manifest.managed.contains_key("collision.txt"));
    assert!(!plan.next_manifest.managed.contains_key("remove.txt"));
}

#[test]
fn planner_preserves_local_deletion_and_flags_divergent_deletion() {
    let repo = tempdir().expect("repo");
    let old = installed(&[("same.txt", "old"), ("changed.txt", "old")]);
    let desired: BTreeMap<String, DesiredFile> =
        [desired("same.txt", "old"), desired("changed.txt", "new")]
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();
    let plan = build_plan(
        repo.path(),
        &desired,
        Some(&old),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");
    assert_eq!(plan.actions[0].action, PlanAction::Conflict);
    assert_eq!(plan.actions[1].action, PlanAction::Preserve);
}

fn action<'a>(plan: &'a UpgradePlan, path: &str) -> &'a PlanEntry {
    plan.actions
        .iter()
        .find(|entry| entry.path == path)
        .expect("planned path")
}

#[test]
fn planner_classifies_absent_and_modified_removals() {
    let repo = tempdir().expect("repo");
    fs::write(repo.path().join("modified.txt"), "mine").expect("local file");
    let old = installed(&[("absent.txt", "old"), ("modified.txt", "old")]);

    let plan = build_plan(
        repo.path(),
        &BTreeMap::new(),
        Some(&old),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");

    assert_eq!(action(&plan, "absent.txt").action, PlanAction::Remove);
    assert_eq!(action(&plan, "absent.txt").reason, "already_absent");
    assert_eq!(action(&plan, "modified.txt").action, PlanAction::Conflict);
    assert_eq!(
        action(&plan, "modified.txt").reason,
        "removed_locally_modified_file"
    );
    assert_eq!(plan.counts.remove, 1);
    assert_eq!(plan.counts.conflict, 1);
}

#[test]
fn planner_handles_nonregular_and_already_updated_managed_paths() {
    let repo = tempdir().expect("repo");
    fs::create_dir(repo.path().join("directory")).expect("directory");
    fs::write(repo.path().join("already-new.txt"), "new").expect("new local");
    fs::write(repo.path().join("locally-modified.txt"), "mine").expect("modified local");
    let old = installed(&[
        ("directory", "old"),
        ("already-new.txt", "old"),
        ("locally-modified.txt", "old"),
    ]);
    let desired: BTreeMap<_, _> = [
        desired("directory", "new"),
        desired("already-new.txt", "new"),
        desired("locally-modified.txt", "old"),
    ]
    .into_iter()
    .map(|file| (file.path.clone(), file))
    .collect();

    let plan = build_plan(
        repo.path(),
        &desired,
        Some(&old),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");

    assert_eq!(
        action(&plan, "directory").reason,
        "managed_path_is_not_a_regular_file"
    );
    assert_eq!(
        action(&plan, "already-new.txt").reason,
        "already_matches_new_source"
    );
    assert_eq!(
        action(&plan, "locally-modified.txt").reason,
        "locally_modified_bundle_unchanged"
    );
    assert_eq!(plan.from_bundle_version.as_deref(), Some("old"));
    assert_eq!(plan.to_bundle_version, "new");
    assert_eq!(plan.next_manifest.bundle_version, "new");
}

#[test]
fn planner_reports_workspace_inspection_errors_without_mutation() {
    let repo = tempdir().expect("repo");
    fs::write(repo.path().join("parent"), "not a directory").expect("blocking file");
    let desired = desired("parent/child", "new");
    let desired = BTreeMap::from([(desired.path.clone(), desired)]);

    let result = build_plan(
        repo.path(),
        &desired,
        None,
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    );

    assert!(matches!(
        result,
        Err(PlanError::Inspect { path, .. }) if path == "parent/child"
    ));
    assert_eq!(
        fs::read_to_string(repo.path().join("parent")).expect("unchanged blocker"),
        "not a directory"
    );
}
