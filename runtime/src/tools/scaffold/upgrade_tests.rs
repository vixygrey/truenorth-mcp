use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;

use super::{
    adopt_exact, apply_workspace_upgrade, desired_files, plan_workspace_upgrade, target_skill_sets,
};
use crate::engine::profile;
use crate::engine::workspace_upgrade::bundle::PackageBundle;
use crate::engine::workspace_upgrade::manifest::{
    BUNDLE_SCHEMA_VERSION, BundleFile, BundleManifest, MANIFEST_VERSION, ManagedFile, SkillSet,
    WorkspaceManifest, sha256,
};

fn bundle_with_supported_from(supported_from: Vec<String>) -> TempDir {
    let bundle = TempDir::new().expect("bundle");
    let root = bundle.path();
    let files = [
        ("skills/core-skill/SKILL.md", "core\n"),
        ("skills/visual-skill/SKILL.md", "visual\n"),
    ];
    for (path, body) in files {
        let full = root.join(path);
        fs::create_dir_all(full.parent().expect("file parent")).expect("bundle dirs");
        fs::write(full, body).expect("bundle file");
    }
    let manifest = BundleManifest {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_schema_version: "2".to_string(),
        supported_from,
        files: files
            .into_iter()
            .map(|(path, body)| BundleFile {
                path: path.to_string(),
                sha256: sha256(body.as_bytes()),
                mode: "0644".to_string(),
            })
            .collect(),
        skill_sets: vec![
            SkillSet {
                name: "core".to_string(),
                support: "supported".to_string(),
                description: "Core test skills".to_string(),
                prerequisites: "none".to_string(),
                skills: vec!["core-skill".to_string()],
            },
            SkillSet {
                name: "visual".to_string(),
                support: "optional".to_string(),
                description: "Visual test skills".to_string(),
                prerequisites: "none".to_string(),
                skills: vec!["visual-skill".to_string()],
            },
        ],
    };
    fs::create_dir_all(root.join("bundle")).expect("bundle manifest dir");
    fs::write(
        root.join("bundle/current.json"),
        serde_json::to_vec(&manifest).expect("serialize manifest"),
    )
    .expect("manifest");
    bundle
}

fn installed(version: &str, manifest_version: &str, skill_sets: Vec<String>) -> WorkspaceManifest {
    WorkspaceManifest {
        manifest_version: manifest_version.to_string(),
        bundle_version: version.to_string(),
        workspace_schema_version: "2".to_string(),
        profile: "generic".to_string(),
        skill_sets,
        managed: BTreeMap::new(),
    }
}

#[test]
fn target_skill_sets_validates_names_and_core_invariant() {
    let fixture = bundle_with_supported_from(Vec::new());
    let bundle = PackageBundle::load(fixture.path()).expect("load bundle");
    let current = installed(
        env!("CARGO_PKG_VERSION"),
        MANIFEST_VERSION,
        vec!["core".to_string()],
    );

    assert_eq!(
        target_skill_sets(&bundle, &current, &["visual".to_string()], &[]).expect("add visual"),
        vec!["core".to_string(), "visual".to_string()]
    );
    assert!(
        target_skill_sets(&bundle, &current, &["missing".to_string()], &[])
            .expect_err("unknown add")
            .contains("unknown skill set `missing`")
    );
    assert!(
        target_skill_sets(&bundle, &current, &[], &["core".to_string()])
            .expect_err("core removal")
            .contains("cannot be removed")
    );
}

#[test]
fn legacy_or_empty_selection_adopts_all_bundle_skill_sets() {
    let fixture = bundle_with_supported_from(Vec::new());
    let bundle = PackageBundle::load(fixture.path()).expect("load bundle");
    for current in [
        installed("old", "1", vec!["core".to_string()]),
        installed("old", MANIFEST_VERSION, Vec::new()),
    ] {
        assert_eq!(
            target_skill_sets(&bundle, &current, &[], &[]).expect("default sets"),
            vec!["core".to_string(), "visual".to_string()]
        );
    }
}

#[test]
fn desired_files_combines_selected_bundle_files_with_profile_scaffold() {
    let fixture = bundle_with_supported_from(Vec::new());
    let bundle = PackageBundle::load(fixture.path()).expect("load bundle");
    let desired =
        desired_files(&bundle, profile::GENERIC, &["core".to_string()]).expect("desired files");

    assert_eq!(desired["skills/core-skill/SKILL.md"].bytes, b"core\n");
    assert!(!desired.contains_key("skills/visual-skill/SKILL.md"));
    assert_eq!(desired[".agent/profile.yml"].bytes, b"profile: generic\n");
    assert!(desired.contains_key("AGENTS.md"));
}

#[test]
fn planning_rejects_unsupported_installed_version_without_changes() {
    let repo = TempDir::new().expect("repo");
    let fixture = bundle_with_supported_from(Vec::new());
    let manifest = installed("ancient", MANIFEST_VERSION, vec!["core".to_string()]);
    let path = repo.path().join(".agent/workspace-manifest.yml");
    fs::create_dir_all(path.parent().expect("manifest parent")).expect("agent dir");
    let original = manifest.to_yaml().expect("manifest yaml");
    fs::write(&path, &original).expect("installed manifest");

    let error = plan_workspace_upgrade(repo.path(), fixture.path(), &[], &[])
        .expect_err("unsupported installed version");

    assert!(error.contains("ancient"));
    assert!(error.contains(env!("CARGO_PKG_VERSION")));
    assert_eq!(
        fs::read_to_string(path).expect("unchanged manifest"),
        original
    );
}

#[test]
fn planning_legacy_workspace_requires_a_migration_baseline() {
    let repo = TempDir::new().expect("repo");
    let fixture = bundle_with_supported_from(Vec::new());

    let error = plan_workspace_upgrade(repo.path(), fixture.path(), &[], &[])
        .expect_err("missing migration baseline");

    assert!(error.contains("no supported migration baseline"));
    assert!(!repo.path().join(".agent/workspace-manifest.yml").exists());
}

#[test]
fn adopt_exact_manages_only_exact_regular_files() {
    let repo = TempDir::new().expect("repo");
    fs::write(repo.path().join("exact.txt"), "same").expect("exact file");
    fs::write(repo.path().join("changed.txt"), "local").expect("changed file");
    fs::create_dir(repo.path().join("directory")).expect("directory");
    let record = ManagedFile {
        source_sha256: sha256(b"same"),
        mode: "0644".to_string(),
    };
    let mut managed = BTreeMap::new();

    adopt_exact(repo.path(), "exact.txt", record.clone(), &mut managed).expect("exact");
    adopt_exact(repo.path(), "changed.txt", record.clone(), &mut managed).expect("changed");
    adopt_exact(repo.path(), "directory", record.clone(), &mut managed).expect("directory");
    adopt_exact(repo.path(), "missing.txt", record, &mut managed).expect("missing");

    assert_eq!(managed.len(), 1);
    assert!(managed.contains_key("exact.txt"));
}

#[test]
fn apply_rejects_dirty_git_workspace_without_overwriting_scaffold() {
    if !crate::isolated_process_tests::in_child(
        "tools::scaffold::upgrade::tests::apply_rejects_dirty_git_workspace_without_overwriting_scaffold",
        &[],
    ) {
        return;
    }
    let repo = TempDir::new().expect("repo");
    let fixture = bundle_with_supported_from(Vec::new());
    super::super::scaffold_project(repo.path(), profile::GENERIC).expect("valid scaffold");
    let state = repo.path().join(".agent/tasks/state.yml");
    fs::write(&state, "phase: discover\nlocal: keep\n").expect("local state");
    let before = fs::read(&state).expect("state before");
    let status = Command::new("git")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").expect("git PATH"))
        .arg("init")
        .arg("--quiet")
        .current_dir(repo.path())
        .status()
        .expect("run git init");
    assert!(status.success());

    let error =
        apply_workspace_upgrade(repo.path(), fixture.path(), &[], &[]).expect_err("dirty worktree");

    assert!(error.contains("clean Git worktree"));
    assert_eq!(fs::read(&state).expect("state unchanged"), before);
    assert!(!repo.path().join("skills/core-skill").exists());
}
