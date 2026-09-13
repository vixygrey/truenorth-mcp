//! Property tests for the emitted git hooks.
//!
//! Included from `hooks.rs` via `#[path]`, so `super` is the hooks module. These write
//! the rendered scripts to disk and run them with `/bin/sh`, so they exercise the real
//! emitted behavior, not a reimplementation.
//!
//! Feature: agent-workspace-profiles, Property 12: commit-msg hook decision. The hook
//! exits 0 if and only if the subject uses an approved type and matches the
//! `type(scope): description` format, and either an id reference is present or the profile
//! does not require one. A merge or revert subject always exits 0.
//!
//! Feature: agent-workspace-profiles, Property 11: post-merge sweep safety. The hook never
//! deletes the current or trunk branch, deletes a topic branch only when it matches the
//! profile pattern and is provably present on the trunk, and deletes nothing on a detached
//! HEAD.

use std::fs;
use std::path::Path;
use std::process::Command;

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;
use crate::engine::profile::{EPIC_BASED, GENERIC, ISSUE_PER_TASK, KANBAN, MILESTONE_BASED};

/// Write a script to `path` and mark it executable.
fn write_script(path: &Path, body: &str) {
    fs::write(path, body).expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

/// Run the commit-msg hook against a message, returning true when it exits 0.
fn run_commit_msg(profile: crate::engine::profile::Profile, message: &str) -> bool {
    let dir = TempDir::new().expect("temp dir");
    let hook = dir.path().join("commit-msg");
    write_script(&hook, &commit_msg_hook(profile));
    let msg_file = dir.path().join("COMMIT_EDITMSG");
    fs::write(&msg_file, message).expect("write message");

    Command::new("/bin/sh")
        .arg(&hook)
        .arg(&msg_file)
        .status()
        .expect("run commit-msg")
        .success()
}

/// The five profiles, indexed for generation.
const PROFILES: [crate::engine::profile::Profile; 5] =
    [EPIC_BASED, ISSUE_PER_TASK, KANBAN, MILESTONE_BASED, GENERIC];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Property 12: the commit-msg hook exits 0 iff the type and format are valid and the
    /// id requirement is met.
    #[test]
    fn commit_msg_decision_matches_the_rule(
        profile_idx in 0usize..5,
        type_idx in 0usize..12,
        with_scope in any::<bool>(),
        with_id in any::<bool>(),
    ) {
        let profile = PROFILES[profile_idx];
        // Index 0..11 are approved types; index 11 is a bogus type.
        let types = [
            "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci",
            "chore", "revert",
        ];
        let (ty, type_ok) = if type_idx < types.len() {
            (types[type_idx], true)
        } else {
            ("wip", false)
        };
        let scope = if with_scope { "(core)" } else { "" };
        let id = if with_id { " (#12)" } else { "" };
        let subject = format!("{ty}{scope}: do the thing{id}");

        let accepted = run_commit_msg(profile, &subject);

        // Oracle: valid type and format, and the id requirement is met.
        let id_ok = with_id || !profile.require_issue_id;
        let expected = type_ok && id_ok;
        prop_assert_eq!(accepted, expected,
            "profile={} subject={:?} require_id={}", profile.name, subject, profile.require_issue_id);
    }

    /// Property 12: a generated merge or revert subject always exits 0.
    #[test]
    fn commit_msg_accepts_merge_and_revert(profile_idx in 0usize..5, revert in any::<bool>()) {
        let profile = PROFILES[profile_idx];
        let subject = if revert { "Revert \"feat: x\"" } else { "Merge branch 'topic'" };
        prop_assert!(run_commit_msg(profile, subject));
    }
}

/// Run a git command in `repo`, asserting success.
fn git(repo: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("run git")
        .success();
    assert!(ok, "git {args:?} failed");
}

/// Whether a local branch exists in `repo`.
fn branch_exists(repo: &Path, name: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{name}")])
        .current_dir(repo)
        .status()
        .expect("run git rev-parse")
        .success()
}

/// Initialize a git repo on `main` with one commit and a configured identity.
fn init_repo() -> TempDir {
    let dir = TempDir::new().expect("temp repo");
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    fs::write(p.join("README.md"), "root\n").expect("seed file");
    git(p, &["add", "-A"]);
    git(p, &["commit", "-q", "-m", "chore: init"]);
    dir
}

/// Install and run the post-merge hook in `repo` for `profile`.
fn run_post_merge(repo: &Path, profile: crate::engine::profile::Profile) {
    let hooks_dir = repo.join(".githooks");
    fs::create_dir_all(&hooks_dir).expect("hooks dir");
    let hook = hooks_dir.join("post-merge");
    write_script(&hook, &post_merge_hook(profile));
    let ok = Command::new("/bin/sh")
        .arg(&hook)
        .current_dir(repo)
        .status()
        .expect("run post-merge")
        .success();
    assert!(ok, "post-merge hook exited non-zero");
}

#[test]
fn post_merge_deletes_a_merged_topic_branch_only() {
    // Property 11: a merged topic branch that matches the pattern is deleted, an unmerged
    // one is retained, and the trunk survives.
    let repo = init_repo();
    let p = repo.path();

    // A merged feature branch: create, commit, merge into main.
    git(p, &["checkout", "-q", "-b", "feat/e1-done"]);
    fs::write(p.join("a.txt"), "a\n").expect("write");
    git(p, &["add", "-A"]);
    git(p, &["commit", "-q", "-m", "feat: a (#1)"]);
    git(p, &["checkout", "-q", "main"]);
    git(
        p,
        &["merge", "-q", "--no-ff", "feat/e1-done", "-m", "merge"],
    );

    // An unmerged feature branch with a divergent commit.
    git(p, &["checkout", "-q", "-b", "feat/e2-open"]);
    fs::write(p.join("b.txt"), "b\n").expect("write");
    git(p, &["add", "-A"]);
    git(p, &["commit", "-q", "-m", "feat: b (#2)"]);
    git(p, &["checkout", "-q", "main"]);

    run_post_merge(p, EPIC_BASED);

    assert!(
        !branch_exists(p, "feat/e1-done"),
        "the merged branch is swept"
    );
    assert!(
        branch_exists(p, "feat/e2-open"),
        "the unmerged branch is retained"
    );
    assert!(branch_exists(p, "main"), "the trunk survives");
}

#[test]
fn post_merge_does_nothing_off_the_trunk() {
    // Property 11: the sweep runs only on the trunk.
    let repo = init_repo();
    let p = repo.path();

    // A merged branch that would be swept if the hook ran on the trunk.
    git(p, &["checkout", "-q", "-b", "feat/e1-done"]);
    fs::write(p.join("a.txt"), "a\n").expect("write");
    git(p, &["add", "-A"]);
    git(p, &["commit", "-q", "-m", "feat: a (#1)"]);
    git(p, &["checkout", "-q", "main"]);
    git(
        p,
        &["merge", "-q", "--no-ff", "feat/e1-done", "-m", "merge"],
    );

    // Move onto a topic branch, then run the hook. It must delete nothing.
    git(p, &["checkout", "-q", "-b", "feat/e9-working"]);
    run_post_merge(p, EPIC_BASED);

    assert!(
        branch_exists(p, "feat/e1-done"),
        "no sweep happens off the trunk"
    );
    assert!(branch_exists(p, "feat/e9-working"));
}

#[test]
fn post_merge_retains_a_branch_off_the_profile_pattern() {
    // Property 11: a merged branch whose name does not match the profile pattern is kept.
    let repo = init_repo();
    let p = repo.path();

    // The epic-based pattern is ^(feat|fix)/e[0-9]+, so `chore/cleanup` does not match.
    git(p, &["checkout", "-q", "-b", "chore/cleanup"]);
    fs::write(p.join("c.txt"), "c\n").expect("write");
    git(p, &["add", "-A"]);
    git(p, &["commit", "-q", "-m", "chore: c"]);
    git(p, &["checkout", "-q", "main"]);
    git(
        p,
        &["merge", "-q", "--no-ff", "chore/cleanup", "-m", "merge"],
    );

    run_post_merge(p, EPIC_BASED);

    assert!(
        branch_exists(p, "chore/cleanup"),
        "a merged branch off the profile pattern is retained"
    );
}
