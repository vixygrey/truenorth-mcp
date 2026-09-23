//! Focused coverage for the fresh-project bootstrap entry point.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::bootstrap_project;

fn skill_bundle() -> TempDir {
    let bundle = TempDir::new().expect("skill bundle");
    let root = bundle.path();
    fs::create_dir_all(root.join("using-truenorth/references")).expect("using-truenorth dirs");
    fs::write(
        root.join("using-truenorth/SKILL.md"),
        "---\nname: using-truenorth\ndescription: bootstrap test\n---\n",
    )
    .expect("required skill");
    fs::write(
        root.join("using-truenorth/references/guide.txt"),
        "nested asset\n",
    )
    .expect("nested asset");
    fs::write(
        root.join("using-truenorth/references/icon.bin"),
        [0, 159, 146, 150],
    )
    .expect("binary asset");
    fs::create_dir_all(root.join("guard/scripts")).expect("script dirs");
    let script = root.join("guard/scripts/check.sh");
    fs::write(&script, "#!/bin/sh\nexit 0\n").expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("script mode");
    }
    bundle
}

#[test]
fn bootstrap_creates_language_agnostic_workspace_and_copies_assets() {
    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();

    let result = bootstrap_project(repo.path(), Some("generic"), bundle.path()).expect("bootstrap");

    assert_eq!(result["profile"], "generic");
    assert!(repo.path().join(".agent/layout.yml").is_file());
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
fn bootstrap_rejects_an_owned_target_without_writing() {
    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();
    fs::write(repo.path().join("AGENTS.md"), "human owned\n").expect("existing target");

    let error =
        bootstrap_project(repo.path(), None, bundle.path()).expect_err("must reject conflict");

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
        bootstrap_project(repo.path(), None, source.path()).expect_err("must reject source");

    assert!(error.contains("using-truenorth/SKILL.md"));
    assert!(!repo.path().join(".agent").exists());
}

#[cfg(unix)]
#[test]
fn bootstrap_rejects_symlinked_skill_source_without_writing() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("project");
    let bundle = skill_bundle();
    symlink(Path::new("/tmp"), bundle.path().join("linked")).expect("symlink");

    let error =
        bootstrap_project(repo.path(), None, bundle.path()).expect_err("must reject symlink");

    assert!(error.contains("unsupported symlink"));
    assert!(!repo.path().join(".agent").exists());
}
