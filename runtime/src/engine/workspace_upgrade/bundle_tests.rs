use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::*;
use crate::engine::workspace_upgrade::manifest::{BUNDLE_SCHEMA_VERSION, BundleFile, SkillSet};

fn core_set() -> SkillSet {
    SkillSet {
        name: "core".to_string(),
        support: "supported".to_string(),
        description: "Core test skills.".to_string(),
        prerequisites: "none".to_string(),
        skills: vec!["a".to_string()],
    }
}
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
        workspace_schema_version: "3".to_string(),
        supported_from: Vec::new(),
        files: vec![BundleFile {
            path: "skills/a/SKILL.md".to_string(),
            sha256: sha256(body),
            mode: "0644".to_string(),
        }],
        skill_sets: vec![core_set()],
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
        workspace_schema_version: "3".to_string(),
        supported_from: Vec::new(),
        files: vec![BundleFile {
            path: "skills/a/SKILL.md".to_string(),
            sha256: sha256(b"expected\n"),
            mode: "0644".to_string(),
        }],
        skill_sets: vec![core_set()],
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

fn write_bundle(root: &Path, manifest: &BundleManifest, files: &[(&str, &[u8])]) {
    fs::create_dir_all(root.join("bundle")).expect("bundle metadata");
    for (path, bytes) in files {
        let target = root.join(path);
        fs::create_dir_all(target.parent().expect("file parent")).expect("parent");
        fs::write(target, bytes).expect("bundle file");
    }
    fs::write(
        root.join("bundle/current.json"),
        serde_json::to_vec(manifest).expect("serialize"),
    )
    .expect("manifest");
}

fn current_manifest(files: Vec<BundleFile>) -> BundleManifest {
    BundleManifest {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_schema_version: "3".to_string(),
        supported_from: vec!["1.0.0".to_string()],
        files,
        skill_sets: vec![core_set()],
    }
}

#[test]
fn package_bundle_rejects_noncurrent_schema_and_runtime_version() {
    let dir = tempdir().expect("bundle root");
    let manifest = BundleManifest {
        schema_version: 1,
        bundle_version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_schema_version: "1".to_string(),
        supported_from: Vec::new(),
        files: Vec::new(),
        skill_sets: Vec::new(),
    };
    write_bundle(dir.path(), &manifest, &[]);
    assert!(matches!(
        PackageBundle::load(dir.path()),
        Err(BundleError::InvalidFile { .. })
    ));

    let mut manifest = current_manifest(vec![BundleFile {
        path: "skills/a/SKILL.md".to_string(),
        sha256: sha256(b"a"),
        mode: "0644".to_string(),
    }]);
    manifest.bundle_version = "different".to_string();
    write_bundle(dir.path(), &manifest, &[("skills/a/SKILL.md", b"a")]);
    assert!(matches!(
        PackageBundle::load(dir.path()),
        Err(BundleError::InvalidFile { .. })
    ));
}

#[test]
fn package_bundle_reports_missing_and_nonregular_sources() {
    let dir = tempdir().expect("bundle root");
    let manifest = current_manifest(vec![BundleFile {
        path: "skills/a/SKILL.md".to_string(),
        sha256: sha256(b"a"),
        mode: "0644".to_string(),
    }]);
    write_bundle(dir.path(), &manifest, &[]);
    assert!(matches!(
        PackageBundle::load(dir.path()),
        Err(BundleError::Read { .. })
    ));

    fs::create_dir_all(dir.path().join("skills/a/SKILL.md")).expect("directory collision");
    assert!(matches!(
        PackageBundle::load(dir.path()),
        Err(BundleError::InvalidFile { .. })
    ));
}

#[test]
fn package_bundle_rejects_mode_drift() {
    let dir = tempdir().expect("bundle root");
    let manifest = current_manifest(vec![BundleFile {
        path: "skills/a/SKILL.md".to_string(),
        sha256: sha256(b"a"),
        mode: "0755".to_string(),
    }]);
    write_bundle(dir.path(), &manifest, &[("skills/a/SKILL.md", b"a")]);
    assert!(matches!(
        PackageBundle::load(dir.path()),
        Err(BundleError::InvalidFile { .. })
    ));
}

#[test]
fn history_and_skill_set_selection_are_explicit() {
    let dir = tempdir().expect("bundle root");
    let body = b"a";
    let mut manifest = current_manifest(vec![
        BundleFile {
            path: "skills/a/SKILL.md".to_string(),
            sha256: sha256(body),
            mode: "0644".to_string(),
        },
        BundleFile {
            path: ".agent/state.yml".to_string(),
            sha256: sha256(b"phase: discover\n"),
            mode: "0644".to_string(),
        },
    ]);
    manifest.skill_sets.push(SkillSet {
        name: "extra".to_string(),
        support: "supported".to_string(),
        description: "Extra".to_string(),
        prerequisites: "none".to_string(),
        skills: vec!["b".to_string()],
    });
    manifest.files.push(BundleFile {
        path: "skills/b/SKILL.md".to_string(),
        sha256: sha256(b"b"),
        mode: "0644".to_string(),
    });
    write_bundle(
        dir.path(),
        &manifest,
        &[
            ("skills/a/SKILL.md", body),
            ("skills/b/SKILL.md", b"b"),
            (".agent/state.yml", b"phase: discover\n"),
        ],
    );
    let bundle = PackageBundle::load(dir.path()).expect("bundle");

    assert_eq!(bundle.load_history("unlisted").expect("history"), None);
    let history = BundleManifest {
        schema_version: 1,
        bundle_version: "1.0.0".to_string(),
        workspace_schema_version: "1".to_string(),
        supported_from: Vec::new(),
        files: Vec::new(),
        skill_sets: Vec::new(),
    };
    fs::create_dir_all(dir.path().join("bundle/history")).expect("history directory");
    fs::write(
        dir.path().join("bundle/history/1.0.0.json"),
        serde_json::to_vec(&history).expect("serialize history"),
    )
    .expect("history");
    assert_eq!(
        bundle
            .load_history("1.0.0")
            .expect("supported history")
            .expect("history manifest"),
        history
    );

    assert!(matches!(
        bundle.resolve_skill_sets(&["missing".to_string()]),
        Err(BundleError::UnknownSkillSet(name)) if name == "missing"
    ));
    assert_eq!(
        bundle
            .resolve_skill_sets(&["extra".to_string()])
            .expect("sets"),
        vec!["core".to_string(), "extra".to_string()]
    );
    let core_files = bundle.files_for_skill_sets(&[]).expect("core files");
    assert!(core_files.contains_key("skills/a/SKILL.md"));
    assert!(core_files.contains_key(".agent/state.yml"));
    assert!(!core_files.contains_key("skills/b/SKILL.md"));
}
