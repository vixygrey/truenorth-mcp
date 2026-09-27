use std::collections::BTreeMap;
use std::path::Path;

use super::*;

fn hash() -> String {
    "a".repeat(64)
}

#[test]
fn bundle_manifest_accepts_sorted_safe_files() {
    let manifest = BundleManifest {
        schema_version: 1,
        bundle_version: "1.0.3".to_string(),
        workspace_schema_version: "1".to_string(),
        supported_from: vec!["1.0.2".to_string()],
        files: vec![BundleFile {
            path: "skills/using-truenorth/SKILL.md".to_string(),
            sha256: hash(),
            mode: "0644".to_string(),
        }],
    };
    manifest.validate(Path::new("bundle.json")).expect("valid");
}

#[test]
fn manifests_reject_escape_duplicate_hash_and_mode() {
    for path in [
        "",
        "/tmp/x",
        "../x",
        "skills/../x",
        ".agent/runtime/x",
        "skills\\x",
    ] {
        let manifest = BundleManifest {
            schema_version: 1,
            bundle_version: "1".to_string(),
            workspace_schema_version: "1".to_string(),
            supported_from: Vec::new(),
            files: vec![BundleFile {
                path: path.to_string(),
                sha256: hash(),
                mode: "0644".to_string(),
            }],
        };
        assert!(
            manifest.validate(Path::new("bundle.json")).is_err(),
            "accepted {path}"
        );
    }

    let duplicate = BundleFile {
        path: "skills/a/SKILL.md".to_string(),
        sha256: hash(),
        mode: "0644".to_string(),
    };
    let manifest = BundleManifest {
        schema_version: 1,
        bundle_version: "1".to_string(),
        workspace_schema_version: "1".to_string(),
        supported_from: Vec::new(),
        files: vec![duplicate.clone(), duplicate],
    };
    assert!(manifest.validate(Path::new("bundle.json")).is_err());

    let mut managed = BTreeMap::new();
    managed.insert(
        "skills/a/SKILL.md".to_string(),
        ManagedFile {
            source_sha256: "xyz".to_string(),
            mode: "0777".to_string(),
        },
    );
    let workspace = WorkspaceManifest {
        manifest_version: "1".to_string(),
        bundle_version: "1".to_string(),
        workspace_schema_version: "1".to_string(),
        profile: "generic".to_string(),
        managed,
    };
    assert!(workspace.validate(Path::new(MANIFEST_REL_PATH)).is_err());
}

#[test]
fn sha256_matches_known_vector() {
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
