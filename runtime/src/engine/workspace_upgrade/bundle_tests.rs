use std::fs;

use tempfile::tempdir;

use super::*;
use crate::engine::workspace_upgrade::manifest::{BUNDLE_SCHEMA_VERSION, BundleFile};

#[test]
fn package_bundle_loads_only_verified_manifest_files() {
    let dir = tempdir().expect("bundle root");
    fs::create_dir_all(dir.path().join("bundle")).expect("bundle metadata");
    fs::create_dir_all(dir.path().join("skills/a")).expect("skill dir");
    let body = b"skill body\n";
    fs::write(dir.path().join("skills/a/SKILL.md"), body).expect("skill");
    let manifest = BundleManifest {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_schema_version: "1".to_string(),
        supported_from: Vec::new(),
        files: vec![BundleFile {
            path: "skills/a/SKILL.md".to_string(),
            sha256: sha256(body),
            mode: "0644".to_string(),
        }],
    };
    fs::write(
        dir.path().join("bundle/current.json"),
        serde_json::to_vec(&manifest).expect("serialize"),
    )
    .expect("manifest");

    let bundle = PackageBundle::load(dir.path()).expect("load bundle");
    assert_eq!(bundle.files.len(), 1);
    assert_eq!(bundle.files["skills/a/SKILL.md"].bytes, body);
}

#[test]
fn package_bundle_rejects_source_drift() {
    let dir = tempdir().expect("bundle root");
    fs::create_dir_all(dir.path().join("bundle")).expect("bundle metadata");
    fs::create_dir_all(dir.path().join("skills/a")).expect("skill dir");
    fs::write(dir.path().join("skills/a/SKILL.md"), "changed\n").expect("skill");
    let manifest = BundleManifest {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_schema_version: "1".to_string(),
        supported_from: Vec::new(),
        files: vec![BundleFile {
            path: "skills/a/SKILL.md".to_string(),
            sha256: sha256(b"expected\n"),
            mode: "0644".to_string(),
        }],
    };
    fs::write(
        dir.path().join("bundle/current.json"),
        serde_json::to_vec(&manifest).expect("serialize"),
    )
    .expect("manifest");

    assert!(matches!(
        PackageBundle::load(dir.path()),
        Err(BundleError::InvalidFile { .. })
    ));
}
