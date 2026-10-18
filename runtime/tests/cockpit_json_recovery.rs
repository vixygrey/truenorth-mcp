use std::fs;
use std::process::Command;

use tempfile::tempdir;
use truenorth_mcp::engine::cockpit_json_transaction::{self, Change};
use truenorth_mcp::engine::mutation::acquire_worktree_lease;

#[test]
fn resume_and_rollback_refuse_a_contended_worktree_before_mutating() {
    let repo = tempdir().expect("repo");
    let tasks = repo.path().join(".agent/tasks");
    fs::create_dir_all(&tasks).expect("tasks");
    let target = tasks.join("state.yml");
    fs::write(&target, b"old").expect("state");
    cockpit_json_transaction::prepare(
        repo.path(),
        vec![Change {
            path: ".agent/tasks/state.yml".to_string(),
            after: Some(b"new".to_vec()),
        }],
    )
    .expect("prepare journal");
    let journal = repo
        .path()
        .join(".agent/runtime/cockpit-json-migration/transaction.json");
    let original_journal = fs::read(&journal).expect("journal");
    let _lease = acquire_worktree_lease(repo.path()).expect("hold writer lease");

    for option in ["--resume", "--rollback"] {
        let output = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
            .env_clear()
            .current_dir(repo.path())
            .args(["migrate-cockpit-json", option])
            .output()
            .expect("invoke CLI");
        assert!(!output.status.success(), "{option} must contend");
        assert!(String::from_utf8_lossy(&output.stderr).contains("writer lease"));
        assert_eq!(fs::read(&target).expect("unchanged state"), b"old");
        assert_eq!(
            fs::read(&journal).expect("unchanged journal"),
            original_journal
        );
    }
}

#[cfg(unix)]
#[test]
fn recovery_does_not_create_directories_through_symlinked_lease_parents() {
    use std::os::unix::fs::symlink;

    for component in ["agent", "runtime"] {
        let repo = tempdir().expect("repo");
        let outside = tempdir().expect("outside");
        if component == "agent" {
            symlink(outside.path(), repo.path().join(".agent")).expect("agent link");
        } else {
            fs::create_dir(repo.path().join(".agent")).expect("agent");
            symlink(outside.path(), repo.path().join(".agent/runtime")).expect("runtime link");
        }
        for option in ["--resume", "--rollback"] {
            let output = Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
                .env_clear()
                .current_dir(repo.path())
                .args(["migrate-cockpit-json", option])
                .output()
                .expect("invoke CLI");
            assert!(
                !output.status.success(),
                "{option} must reject symlinked {component}"
            );
            assert!(!outside.path().join("runtime").exists());
            assert!(!outside.path().join("writer.lock").exists());
        }
    }
}
