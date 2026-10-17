use std::collections::BTreeMap;
use std::fs;
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
        skill_sets: Vec::new(),
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
        "specs",
        "specs/adr/.gitkeep",
        "specs/authority-probe.md",
        "Specs/evil.md",
        "SPECS",
        "SpEcS/x",
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
            skill_sets: Vec::new(),
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
        skill_sets: Vec::new(),
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
        skill_sets: Vec::new(),
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

fn valid_v2_bundle() -> BundleManifest {
    BundleManifest {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_version: "2.0.0".to_string(),
        workspace_schema_version: "2".to_string(),
        supported_from: vec!["1.0.0".to_string()],
        files: vec![BundleFile {
            path: "skills/a/SKILL.md".to_string(),
            sha256: hash(),
            mode: "0644".to_string(),
        }],
        skill_sets: vec![SkillSet {
            name: "core".to_string(),
            support: "supported".to_string(),
            description: "Core skills".to_string(),
            prerequisites: "none".to_string(),
            skills: vec!["a".to_string()],
        }],
    }
}

fn valid_workspace() -> WorkspaceManifest {
    WorkspaceManifest {
        manifest_version: MANIFEST_VERSION.to_string(),
        bundle_version: "2.0.0".to_string(),
        workspace_schema_version: "3".to_string(),
        profile: "generic".to_string(),
        skill_sets: vec!["core".to_string()],
        managed: BTreeMap::from([(
            "skills/a/SKILL.md".to_string(),
            ManagedFile {
                source_sha256: hash(),
                mode: "0644".to_string(),
            },
        )]),
    }
}

#[test]
fn bundle_manifest_read_distinguishes_parse_and_validation_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bundle.json");
    fs::write(&path, "{").expect("malformed manifest");
    assert!(matches!(
        BundleManifest::read(&path),
        Err(ManifestError::Parse { .. })
    ));

    let mut manifest = valid_v2_bundle();
    manifest.schema_version = 99;
    fs::write(&path, serde_json::to_vec(&manifest).expect("serialize")).expect("manifest");
    assert!(matches!(
        BundleManifest::read(&path),
        Err(ManifestError::Invalid { .. })
    ));
    assert!(matches!(
        BundleManifest::read(&dir.path().join("missing.json")),
        Err(ManifestError::Read { .. })
    ));
}

#[test]
fn bundle_manifest_rejects_version_and_v1_skill_set_mismatches() {
    let path = Path::new("bundle.json");
    let mut manifest = valid_v2_bundle();
    manifest.bundle_version.clear();
    assert!(manifest.validate(path).is_err());

    let mut manifest = valid_v2_bundle();
    manifest.workspace_schema_version = "1".to_string();
    assert!(manifest.validate(path).is_err());

    let mut manifest = valid_v2_bundle();
    manifest.schema_version = 1;
    manifest.workspace_schema_version = "1".to_string();
    assert!(manifest.validate(path).is_err());
}

#[test]
fn bundle_manifest_enforces_complete_sorted_skill_partition() {
    let path = Path::new("bundle.json");

    let mut manifest = valid_v2_bundle();
    manifest.skill_sets[0].name = "Core".to_string();
    assert!(manifest.validate(path).is_err());

    let mut manifest = valid_v2_bundle();
    manifest.skill_sets[0].support.clear();
    assert!(manifest.validate(path).is_err());

    let mut manifest = valid_v2_bundle();
    manifest.skill_sets.push(SkillSet {
        name: "addon".to_string(),
        support: "supported".to_string(),
        description: "Addon".to_string(),
        prerequisites: "none".to_string(),
        skills: vec!["b".to_string()],
    });
    assert!(manifest.validate(path).is_err(), "sets are not sorted");

    let mut manifest = valid_v2_bundle();
    manifest.skill_sets[0].skills = vec!["b".to_string(), "a".to_string()];
    assert!(manifest.validate(path).is_err(), "skills are not sorted");

    let mut manifest = valid_v2_bundle();
    manifest.skill_sets[0].name = "other".to_string();
    assert!(manifest.validate(path).is_err(), "core is required");

    let mut manifest = valid_v2_bundle();
    manifest.skill_sets[0].skills = vec!["b".to_string()];
    assert!(
        manifest.validate(path).is_err(),
        "membership must match files"
    );

    let manifest = valid_v2_bundle();
    assert_eq!(manifest.all_skill_sets(), vec!["core"]);
    assert_eq!(manifest.skill_set("core"), Some(&manifest.skill_sets[0]));
    assert_eq!(manifest.set_for_skill("a"), Some("core"));
    assert_eq!(manifest.set_for_skill("missing"), None);
}

#[test]
fn workspace_manifest_optional_read_and_validation_cover_boundaries() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        WorkspaceManifest::read_optional(dir.path()).expect("optional"),
        None
    );
    fs::create_dir_all(dir.path().join(".agent")).expect("agent");
    let path = dir.path().join(MANIFEST_REL_PATH);
    fs::write(&path, "not: [valid").expect("malformed");
    assert!(matches!(
        WorkspaceManifest::read_optional(dir.path()),
        Err(ManifestError::Parse { .. })
    ));

    let manifest = valid_workspace();
    fs::write(&path, manifest.to_yaml().expect("yaml")).expect("manifest");
    assert_eq!(
        WorkspaceManifest::read_optional(dir.path()).expect("read"),
        Some(manifest)
    );
}

#[test]
fn workspace_manifest_reports_filesystem_read_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join(MANIFEST_REL_PATH)).expect("manifest directory");

    assert!(matches!(
        WorkspaceManifest::read_optional(dir.path()),
        Err(ManifestError::Read { .. })
    ));
}

#[test]
fn workspace_manifest_rejects_invalid_versions_and_skill_selection() {
    let path = Path::new(MANIFEST_REL_PATH);

    let mut manifest = valid_workspace();
    manifest.manifest_version = "9".to_string();
    assert!(manifest.validate(path).is_err());

    let mut manifest = valid_workspace();
    manifest.profile.clear();
    assert!(manifest.validate(path).is_err());

    let mut manifest = valid_workspace();
    manifest.manifest_version = "1".to_string();
    manifest.workspace_schema_version = "1".to_string();
    assert!(manifest.validate(path).is_err(), "v1 cannot select sets");

    let mut manifest = valid_workspace();
    manifest.skill_sets = vec!["core".to_string(), "core".to_string()];
    assert!(manifest.validate(path).is_err(), "sets must be unique");

    let mut manifest = valid_workspace();
    manifest.skill_sets = vec!["addon".to_string()];
    assert!(manifest.validate(path).is_err(), "core is required");

    let mut manifest = valid_workspace();
    manifest.skill_sets = vec!["-bad".to_string(), "core".to_string()];
    assert!(manifest.validate(path).is_err(), "names must be safe");
}
