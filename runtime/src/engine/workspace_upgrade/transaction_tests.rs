use std::collections::BTreeMap;
use std::fs;

use tempfile::tempdir;

use super::{cleanup, prepare, resume, rollback, transaction_exists};
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
