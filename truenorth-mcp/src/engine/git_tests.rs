//! Tests for git context (task 7.3).
//!
//! Included from `git.rs` via `#[path]`, so `super` is the git module. The tests build a
//! real temp git repo, so they exercise the scoping through the pathspec.
//!
//! Requirements: 1.8, 4.7.

use super::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// Run a git command in `dir`, failing the test loudly on error.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        status.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// Initialize a repo with a deterministic identity and an initial commit.
fn init_repo(dir: &Path) {
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    fs::create_dir_all(dir.join("skills")).expect("skills dir");
    fs::create_dir_all(dir.join("specs")).expect("specs dir");
    fs::write(dir.join("README.md"), "root\n").expect("write readme");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

#[test]
fn status_excludes_paths_outside_scope() {
    // Requirement 1.8: a change outside skills/ and specs/ is excluded from status.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    init_repo(root);

    fs::write(root.join("specs/state.yaml"), "active_epic: e01\n").expect("write in scope");
    fs::write(root.join("outside.txt"), "noise\n").expect("write out of scope");

    let status = status(root).expect("git status");
    assert!(
        status.contains("specs/state.yaml"),
        "in-scope change must appear"
    );
    assert!(
        !status.contains("outside.txt"),
        "out-of-scope change must be excluded"
    );
}

#[test]
fn changed_files_in_scope_lists_only_scoped_paths() {
    // Requirement 4.7: the ontology default scope covers only scoped changes.
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    init_repo(root);

    fs::write(root.join("skills/a.txt"), "skill change\n").expect("write skill");
    fs::write(root.join("specs/b.yaml"), "spec change\n").expect("write spec");
    fs::write(root.join("elsewhere.rs"), "code change\n").expect("write outside");

    let mut changed = changed_files_in_scope(root).expect("changed files");
    changed.sort();

    assert_eq!(
        changed,
        vec![PathBuf::from("skills/a.txt"), PathBuf::from("specs/b.yaml")]
    );
}

#[test]
fn diff_excludes_out_of_scope_edits() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    init_repo(root);

    // Modify a tracked in-scope file and a tracked out-of-scope file.
    fs::write(root.join("specs/tracked.yaml"), "v: 1\n").expect("write scoped");
    git(root, &["add", "specs/tracked.yaml"]);
    git(root, &["commit", "-q", "-m", "add scoped"]);
    fs::write(root.join("specs/tracked.yaml"), "v: 2\n").expect("edit scoped");
    fs::write(root.join("README.md"), "changed root\n").expect("edit root");

    let diff = diff(root).expect("git diff");
    assert!(
        diff.contains("specs/tracked.yaml"),
        "scoped diff must appear"
    );
    assert!(
        !diff.contains("README.md"),
        "out-of-scope diff must be excluded"
    );
}

#[test]
fn default_action_is_status() {
    assert_eq!(GitAction::default(), GitAction::Status);
}

#[test]
fn parse_porcelain_takes_rename_target() {
    // A rename line carries `old -> new`; the new path is taken.
    let porcelain = "R  specs/old.yaml -> specs/new.yaml\n M skills/a.txt\n?? specs/c.yaml\n";
    let paths = parse_porcelain_paths(porcelain);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("specs/new.yaml"),
            PathBuf::from("skills/a.txt"),
            PathBuf::from("specs/c.yaml"),
        ]
    );
}
