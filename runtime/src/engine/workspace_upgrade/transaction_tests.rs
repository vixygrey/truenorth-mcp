use std::collections::BTreeMap;
use std::fs;

use tempfile::tempdir;

use super::{TransactionError, cleanup, prepare, resume, rollback, transaction_exists};
use crate::engine::workspace_upgrade::bundle::DesiredFile;
use crate::engine::workspace_upgrade::plan::build_plan;

#[test]
fn resume_is_idempotent_and_rollback_restores_the_preimage() {
    let temp = tempdir().expect("tempdir");
    fs::create_dir(temp.path().join(".agent")).expect("agent root");
    fs::write(temp.path().join("managed.txt"), "old\n").expect("old file");

    let mut desired = BTreeMap::new();
    desired.insert(
        "managed.txt".to_string(),
        DesiredFile {
            path: "managed.txt".to_string(),
            bytes: b"new\n".to_vec(),
            mode: "0644".to_string(),
        },
    );
    let installed = crate::engine::workspace_upgrade::manifest::WorkspaceManifest {
        manifest_version: "1".to_string(),
        bundle_version: "old".to_string(),
        workspace_schema_version: "1".to_string(),
        profile: "generic".to_string(),
        skill_sets: Vec::new(),
        managed: BTreeMap::from([(
            "managed.txt".to_string(),
            crate::engine::workspace_upgrade::manifest::ManagedFile {
                source_sha256: crate::engine::workspace_upgrade::manifest::sha256(b"old\n"),
                mode: "0644".to_string(),
            },
        )]),
    };
    let plan = build_plan(
        temp.path(),
        &desired,
        Some(&installed),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");

    prepare(temp.path(), &plan, &desired).expect("prepare");
    assert!(transaction_exists(temp.path()));
    resume(temp.path()).expect("first resume");
    resume(temp.path()).expect("idempotent resume");
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("read"),
        b"new\n"
    );

    rollback(temp.path()).expect("rollback");
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("read"),
        b"old\n"
    );
    assert!(!transaction_exists(temp.path()));
    cleanup(temp.path()).expect("cleanup remains safe");
}

#[test]
fn resume_and_rollback_cover_added_and_removed_files() {
    let temp = tempdir().expect("tempdir");
    fs::create_dir(temp.path().join(".agent")).expect("agent root");
    fs::write(temp.path().join("removed.txt"), "old\n").expect("removed file");

    let desired = BTreeMap::from([(
        "added.txt".to_string(),
        DesiredFile {
            path: "added.txt".to_string(),
            bytes: b"new\n".to_vec(),
            mode: "0644".to_string(),
        },
    )]);
    let installed = crate::engine::workspace_upgrade::manifest::WorkspaceManifest {
        manifest_version: "1".to_string(),
        bundle_version: "old".to_string(),
        workspace_schema_version: "1".to_string(),
        profile: "generic".to_string(),
        skill_sets: Vec::new(),
        managed: BTreeMap::from([(
            "removed.txt".to_string(),
            crate::engine::workspace_upgrade::manifest::ManagedFile {
                source_sha256: crate::engine::workspace_upgrade::manifest::sha256(b"old\n"),
                mode: "0644".to_string(),
            },
        )]),
    };
    let plan = build_plan(
        temp.path(),
        &desired,
        Some(&installed),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");

    prepare(temp.path(), &plan, &desired).expect("prepare");
    resume(temp.path()).expect("resume");
    assert_eq!(
        fs::read(temp.path().join("added.txt")).expect("added file"),
        b"new\n"
    );
    assert!(!temp.path().join("removed.txt").exists());

    rollback(temp.path()).expect("rollback");
    assert!(!temp.path().join("added.txt").exists());
    assert_eq!(
        fs::read(temp.path().join("removed.txt")).expect("restored file"),
        b"old\n"
    );
    assert!(!temp.path().join(".agent/workspace-manifest.yml").exists());
}

fn update_fixture() -> (
    tempfile::TempDir,
    BTreeMap<String, DesiredFile>,
    crate::engine::workspace_upgrade::plan::UpgradePlan,
) {
    let temp = tempdir().expect("tempdir");
    fs::create_dir(temp.path().join(".agent")).expect("agent root");
    fs::write(temp.path().join("managed.txt"), "old\n").expect("old file");
    let desired = BTreeMap::from([(
        "managed.txt".to_string(),
        DesiredFile {
            path: "managed.txt".to_string(),
            bytes: b"new\n".to_vec(),
            mode: "0644".to_string(),
        },
    )]);
    let installed = crate::engine::workspace_upgrade::manifest::WorkspaceManifest {
        manifest_version: "1".to_string(),
        bundle_version: "old".to_string(),
        workspace_schema_version: "1".to_string(),
        profile: "generic".to_string(),
        skill_sets: Vec::new(),
        managed: BTreeMap::from([(
            "managed.txt".to_string(),
            crate::engine::workspace_upgrade::manifest::ManagedFile {
                source_sha256: crate::engine::workspace_upgrade::manifest::sha256(b"old\n"),
                mode: "0644".to_string(),
            },
        )]),
    };
    let plan = build_plan(
        temp.path(),
        &desired,
        Some(&installed),
        "new",
        "2",
        "generic",
        vec!["core".to_string()],
    )
    .expect("plan");
    (temp, desired, plan)
}

#[test]
fn prepare_rejects_an_existing_transaction_without_changing_files() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");
    let journal =
        fs::read(temp.path().join(".agent/runtime/upgrade/transaction.json")).expect("journal");

    assert!(matches!(
        prepare(temp.path(), &plan, &desired),
        Err(TransactionError::Prepare(_))
    ));
    assert_eq!(
        fs::read(temp.path().join(".agent/runtime/upgrade/transaction.json"))
            .expect("journal after rejection"),
        journal
    );
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"old\n"
    );
}

#[test]
fn prepare_replaces_abandoned_transaction_directory() {
    let (temp, desired, plan) = update_fixture();
    let root = temp.path().join(".agent/runtime/upgrade");
    fs::create_dir_all(&root).expect("abandoned root");
    fs::write(root.join("orphan"), "stale").expect("orphan");

    prepare(temp.path(), &plan, &desired).expect("prepare");

    assert!(!root.join("orphan").exists());
    assert!(root.join("transaction.json").is_file());
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"old\n"
    );
}

#[test]
fn resume_rejects_external_changes_without_overwriting_them() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");
    fs::write(temp.path().join("managed.txt"), "external\n").expect("external change");

    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::UnexpectedState { path }) if path == "managed.txt"
    ));
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"external\n"
    );
    assert!(transaction_exists(temp.path()));
}

#[test]
fn rollback_rejects_external_changes_without_overwriting_them() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");
    resume(temp.path()).expect("resume");
    fs::write(temp.path().join("managed.txt"), "external\n").expect("external change");

    assert!(matches!(
        rollback(temp.path()),
        Err(TransactionError::UnexpectedState { path }) if path == "managed.txt"
    ));
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"external\n"
    );
    assert!(transaction_exists(temp.path()));
}

#[test]
fn rollback_of_unapplied_transaction_preserves_preimage_and_cleans_up() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");

    rollback(temp.path()).expect("rollback");

    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"old\n"
    );
    assert!(!temp.path().join(".agent/workspace-manifest.yml").exists());
    assert!(!transaction_exists(temp.path()));
}

#[test]
fn missing_staged_content_fails_resume_before_workspace_mutation() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");
    fs::remove_file(temp.path().join(".agent/runtime/upgrade/staged/0"))
        .expect("remove staged content");

    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::Apply { path, .. }) if path == "managed.txt"
    ));
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"old\n"
    );
    assert!(transaction_exists(temp.path()));
}

#[test]
fn missing_backup_content_fails_rollback_instead_of_losing_preimage() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");
    resume(temp.path()).expect("resume");
    fs::remove_file(temp.path().join(".agent/runtime/upgrade/backup/0")).expect("remove backup");

    assert!(matches!(
        rollback(temp.path()),
        Err(TransactionError::Rollback { path, .. }) if path == "managed.txt"
    ));
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"new\n"
    );
    assert!(transaction_exists(temp.path()));
}

#[test]
fn journal_parse_and_schema_errors_leave_workspace_untouched() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().join(".agent/runtime/upgrade");
    fs::create_dir_all(&root).expect("transaction root");
    fs::write(temp.path().join("managed.txt"), "original\n").expect("managed");
    let journal = root.join("transaction.json");

    fs::write(&journal, "{").expect("malformed journal");
    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::Prepare(_))
    ));

    fs::write(
        &journal,
        r#"{"schema_version":2,"target_bundle_version":"new","operations":[]}"#,
    )
    .expect("unsupported journal");
    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::Prepare(_))
    ));
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"original\n"
    );
}

#[test]
fn nonregular_transaction_target_is_rejected() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().join(".agent/runtime/upgrade");
    fs::create_dir_all(&root).expect("transaction root");
    fs::create_dir(temp.path().join("managed")).expect("directory target");
    fs::write(
        root.join("transaction.json"),
        r#"{
          "schema_version": 1,
          "target_bundle_version": "new",
          "operations": [{
            "path": "managed",
            "before": null,
            "after": null,
            "backup": null,
            "staged": null
          }]
        }"#,
    )
    .expect("journal");

    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::UnexpectedState { path }) if path == "managed"
    ));
    assert!(temp.path().join("managed").is_dir());
}

#[test]
fn resume_rejects_a_tampered_journal_path_and_mutates_nothing() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");
    fs::write(temp.path().join("outside.txt"), "secret\n").expect("outside file");

    let journal = temp.path().join(".agent/runtime/upgrade/transaction.json");
    let outside_state = crate::engine::workspace_upgrade::manifest::sha256(b"secret\n");
    let tampered = format!(
        r#"{{
          "schema_version": 1,
          "target_bundle_version": "new",
          "operations": [{{
            "path": "../outside.txt",
            "before": {{"sha256": "{outside_state}", "mode": "0644"}},
            "after": null,
            "backup": null,
            "staged": null
          }}]
        }}"#
    );
    fs::write(&journal, tampered).expect("tampered journal");

    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::Prepare(_))
    ));
    assert_eq!(
        fs::read(temp.path().join("outside.txt")).expect("outside file untouched"),
        b"secret\n"
    );
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed"),
        b"old\n"
    );
}

#[test]
fn resume_rejects_a_tampered_staged_reference_and_mutates_nothing() {
    let (temp, desired, plan) = update_fixture();
    prepare(temp.path(), &plan, &desired).expect("prepare");

    let outside_dir = tempdir().expect("outside dir");
    fs::write(outside_dir.path().join("secret.txt"), "classified\n").expect("outside secret");
    let traversal = format!(
        "../../../../../../../..{}/secret.txt",
        outside_dir.path().display()
    );

    let journal = temp.path().join(".agent/runtime/upgrade/transaction.json");
    let new_state = crate::engine::workspace_upgrade::manifest::sha256(b"new\n");
    let tampered = format!(
        r#"{{
          "schema_version": 1,
          "target_bundle_version": "new",
          "operations": [{{
            "path": "managed.txt",
            "before": {{"sha256": "{}", "mode": "0644"}},
            "after": {{"sha256": "{new_state}", "mode": "0644"}},
            "backup": "backup/0",
            "staged": "{traversal}"
          }}]
        }}"#,
        crate::engine::workspace_upgrade::manifest::sha256(b"old\n"),
    );
    fs::write(&journal, tampered).expect("tampered journal");

    assert!(matches!(
        resume(temp.path()),
        Err(TransactionError::Prepare(_))
    ));
    assert_eq!(
        fs::read(temp.path().join("managed.txt")).expect("managed unchanged"),
        b"old\n"
    );
}
