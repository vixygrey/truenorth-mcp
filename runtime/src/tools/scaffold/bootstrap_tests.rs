//! Focused coverage for the fresh-project bootstrap entry point.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::bootstrap_project;
use crate::engine::workspace_upgrade::manifest::{
    BUNDLE_SCHEMA_VERSION, BundleFile, BundleManifest, SkillSet, sha256,
};

fn skill_bundle() -> TempDir {
    let bundle = TempDir::new().expect("skill bundle");
    let root = bundle.path();
    let skills = root.join("skills");
    fs::create_dir_all(skills.join("using-truenorth/references")).expect("using-truenorth dirs");
    fs::write(
        skills.join("using-truenorth/SKILL.md"),
        "---\nname: using-truenorth\ndescription: bootstrap test\n---\n",
    )
    .expect("required skill");
    fs::write(
        skills.join("using-truenorth/references/guide.txt"),
        "nested asset\n",
    )
    .expect("nested asset");
    fs::write(
        skills.join("using-truenorth/references/icon.bin"),
        [0, 159, 146, 150],
    )
    .expect("binary asset");
    fs::create_dir_all(skills.join("guard/scripts")).expect("script dirs");
    let script = skills.join("guard/scripts/check.sh");
    fs::write(&script, "#!/bin/sh\nexit 0\n").expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("script mode");
    }

    let paths = [
        "skills/guard/scripts/check.sh",
        "skills/using-truenorth/SKILL.md",
        "skills/using-truenorth/references/guide.txt",
        "skills/using-truenorth/references/icon.bin",
    ];
    let files = paths
        .into_iter()
        .map(|path| BundleFile {
            path: path.to_string(),
            sha256: sha256(&fs::read(root.join(path)).expect("bundle file")),
            mode: if path.ends_with("check.sh") {
                "0755".to_string()
            } else {
                "0644".to_string()
            },
        })
        .collect();
    let manifest = BundleManifest {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_schema_version: "2".to_string(),
        supported_from: Vec::new(),
        files,
        skill_sets: vec![
            SkillSet {
                name: "core".to_string(),
                support: "supported".to_string(),
                description: "Core test skills.".to_string(),
                prerequisites: "none".to_string(),
                skills: vec!["using-truenorth".to_string()],
            },
            SkillSet {
                name: "visual".to_string(),
                support: "optional".to_string(),
                description: "Visual test skills.".to_string(),
                prerequisites: "none".to_string(),
                skills: vec!["guard".to_string()],
            },
        ],
    };
    fs::create_dir_all(root.join("bundle")).expect("bundle dir");
    fs::write(
        root.join("bundle/current.json"),
        serde_json::to_vec(&manifest).expect("manifest"),
    )
    .expect("write manifest");
    bundle
}

#[test]
fn bootstrap_creates_language_agnostic_workspace_and_copies_assets() {
    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();

    let result = bootstrap_project(
        repo.path(),
        Some("generic"),
        bundle.path(),
        &["visual".to_string()],
    )
    .expect("bootstrap");

    assert_eq!(result["profile"], "generic");
    assert!(repo.path().join(".agent/layout.yml").is_file());
    assert!(repo.path().join(".agent/workspace-manifest.yml").is_file());
    assert!(repo.path().join("specs/adr/.gitkeep").is_file());
    assert_eq!(
        fs::read_to_string(
            repo.path()
                .join("skills/using-truenorth/references/guide.txt")
        )
        .expect("copied asset"),
        "nested asset\n"
    );
    assert_eq!(
        fs::read(
            repo.path()
                .join("skills/using-truenorth/references/icon.bin")
        )
        .expect("copied binary asset"),
        [0, 159, 146, 150]
    );
    let copied_script = repo.path().join("skills/guard/scripts/check.sh");
    assert!(copied_script.is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(copied_script)
                .expect("script metadata")
                .permissions()
                .mode()
                & 0o111,
            0o111
        );
    }
    for language_file in ["Cargo.toml", "package.json", "src"] {
        assert!(
            !repo.path().join(language_file).exists(),
            "bootstrap must not create {language_file}"
        );
    }
}

#[test]
fn bootstrap_defaults_to_core_skills_only() {
    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();

    bootstrap_project(repo.path(), Some("generic"), bundle.path(), &[]).expect("bootstrap");

    assert!(
        repo.path()
            .join("skills/using-truenorth/SKILL.md")
            .is_file()
    );
    assert!(!repo.path().join("skills/guard").exists());
    let manifest = fs::read_to_string(repo.path().join(".agent/workspace-manifest.yml"))
        .expect("workspace manifest");
    assert!(manifest.contains("skill_sets:\n- core\n"));
}

#[test]
fn bootstrap_rejects_an_owned_target_without_writing() {
    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();
    fs::write(repo.path().join("AGENTS.md"), "human owned\n").expect("existing target");

    let error =
        bootstrap_project(repo.path(), None, bundle.path(), &[]).expect_err("must reject conflict");

    assert!(error.contains("AGENTS.md"));
    assert_eq!(
        fs::read_to_string(repo.path().join("AGENTS.md")).unwrap(),
        "human owned\n"
    );
    assert!(!repo.path().join(".agent").exists());
    assert!(!repo.path().join("skills").exists());
}

#[test]
fn bootstrap_rejects_invalid_skill_source_without_writing() {
    let repo = TempDir::new().expect("project");
    let source = TempDir::new().expect("source");
    fs::create_dir(source.path().join("other")).expect("source dir");

    let error =
        bootstrap_project(repo.path(), None, source.path(), &[]).expect_err("must reject source");

    assert!(error.contains("bundle/current.json"));
    assert!(!repo.path().join(".agent").exists());
}

#[cfg(unix)]
#[test]
fn bootstrap_rejects_symlinked_skill_source_without_writing() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();
    let linked = bundle
        .path()
        .join("skills/using-truenorth/references/guide.txt");
    fs::remove_file(&linked).expect("remove listed source");
    symlink(Path::new("/tmp"), &linked).expect("symlink");

    let error =
        bootstrap_project(repo.path(), None, bundle.path(), &[]).expect_err("must reject symlink");

    assert!(error.contains("regular file"));
    assert!(!repo.path().join(".agent").exists());
}
