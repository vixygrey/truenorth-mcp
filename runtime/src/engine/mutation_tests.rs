//! Behavioral tests for mutation serialization and repository writer leases.

use std::fs;

use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn local_coordinator_holds_the_gate_for_the_complete_transaction() {
    let repo = TempDir::new().expect("temp repo");
    let coordinator = MutationCoordinator::local_only(repo.path().to_path_buf());

    assert_eq!(coordinator.initial_status(), "local-only");
    let permit = coordinator.begin().await.expect("first transaction");
    assert_eq!(coordinator.initial_status(), "busy");

    drop(permit);
    assert_eq!(coordinator.initial_status(), "local-only");
    coordinator.begin().await.expect("gate released");
}

#[tokio::test]
async fn default_coordinator_is_a_usable_local_serializer() {
    let coordinator = MutationCoordinator::default();
    assert_eq!(coordinator.initial_status(), "local-only");
    coordinator.begin().await.expect("default transaction");
}

#[cfg(unix)]
#[tokio::test]
async fn repository_coordinator_owns_its_lease_for_its_lifetime() {
    let repo = TempDir::new().expect("temp repo");
    let coordinator = MutationCoordinator::for_repo(repo.path().to_path_buf());
    assert_eq!(coordinator.initial_status(), "held");
    coordinator
        .begin()
        .await
        .expect("held lease permits mutation");

    let contender = acquire_worktree_lease(repo.path()).expect_err("coordinator retains lease");
    assert!(matches!(contender, MutationError::Contended { .. }));
}

#[cfg(unix)]
#[test]
fn acquired_lease_records_owner_and_rejects_a_second_owner() {
    let repo = TempDir::new().expect("temp repo");
    let lease = acquire_worktree_lease(repo.path()).expect("first lease");
    let metadata: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo.path().join(WRITER_LOCK_REL_PATH)).expect("lease metadata"),
    )
    .expect("valid lease metadata");
    assert_eq!(
        metadata.get("pid").and_then(serde_json::Value::as_u64),
        Some(u64::from(std::process::id()))
    );
    assert!(
        metadata
            .get("acquired_at_unix")
            .and_then(serde_json::Value::as_u64)
            .is_some()
    );

    let error = acquire_worktree_lease(repo.path()).expect_err("second owner must contend");
    match error {
        MutationError::Contended {
            lock_path,
            holder_pid,
            holder,
        } => {
            assert_eq!(lock_path, WRITER_LOCK_REL_PATH);
            assert_eq!(holder_pid, Some(std::process::id()));
            assert_eq!(holder, format!(" (process {})", std::process::id()));
        }
        other => panic!("expected contention, got {other}"),
    }
    fs::write(repo.path().join(WRITER_LOCK_REL_PATH), "invalid metadata")
        .expect("replace metadata while lease remains held");
    let unknown = acquire_worktree_lease(repo.path()).expect_err("lease still contended");
    assert!(matches!(
        unknown,
        MutationError::Contended {
            holder_pid: None,
            ref holder,
            ..
        } if holder.is_empty()
    ));
    drop(lease);
    acquire_worktree_lease(repo.path()).expect("released lease is acquirable");
}

#[cfg(unix)]
#[tokio::test]
async fn contended_coordinator_retries_and_becomes_owner_after_release() {
    let repo = TempDir::new().expect("temp repo");
    let owner = acquire_worktree_lease(repo.path()).expect("owner lease");
    let contender = MutationCoordinator::for_repo(repo.path().to_path_buf());
    assert_eq!(contender.initial_status(), "contended");

    let error = contender
        .begin()
        .await
        .expect_err("owner still holds lease");
    assert!(matches!(
        error,
        MutationError::Contended {
            holder_pid: Some(pid),
            ..
        } if pid == std::process::id()
    ));

    drop(owner);
    let permit = contender
        .begin()
        .await
        .expect("retry acquires released lease");
    drop(permit);
    assert_eq!(contender.initial_status(), "held");
}

#[cfg(unix)]
#[tokio::test]
async fn unavailable_coordinator_retries_without_reading_repository_state() {
    let repo = TempDir::new().expect("temp repo");
    fs::write(repo.path().join(AGENT_DIR), "not a directory").expect("block agent directory");
    let direct = acquire_worktree_lease(repo.path()).expect_err("direct lease unavailable");
    assert!(matches!(
        direct,
        MutationError::Unavailable {
            lock_path: WRITER_LOCK_REL_PATH,
            ref detail,
        } if !detail.is_empty()
    ));
    let coordinator = MutationCoordinator::for_repo(repo.path().to_path_buf());
    assert_eq!(coordinator.initial_status(), "unavailable");

    let error = coordinator
        .begin()
        .await
        .expect_err("lease remains unavailable");
    match error {
        MutationError::Unavailable { lock_path, detail } => {
            assert_eq!(lock_path, WRITER_LOCK_REL_PATH);
            assert!(!detail.is_empty());
        }
        other => panic!("expected unavailable lease, got {other}"),
    }
}

#[cfg(unix)]
#[test]
fn lease_rejects_runtime_directory_that_escapes_agent_root() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("temp repo");
    let outside = TempDir::new().expect("outside dir");
    fs::create_dir(repo.path().join(AGENT_DIR)).expect("agent dir");
    symlink(outside.path(), repo.path().join(AGENT_DIR).join("runtime"))
        .expect("escaping runtime link");

    let error = acquire_worktree_lease(repo.path()).expect_err("escaped runtime rejected");
    assert!(matches!(error, MutationError::Unavailable { .. }));
    assert!(error.to_string().contains("escapes `.agent/`"));
    assert!(!outside.path().join("writer.lock").exists());
}

#[cfg(unix)]
#[test]
fn lease_rejects_a_symlinked_lock_file_without_changing_its_target() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("temp repo");
    let outside = TempDir::new().expect("outside dir");
    let runtime = repo.path().join(AGENT_DIR).join("runtime");
    fs::create_dir_all(&runtime).expect("runtime dir");
    let outside_lock = outside.path().join("lock");
    fs::write(&outside_lock, "external bytes").expect("outside lock");
    symlink(&outside_lock, runtime.join("writer.lock")).expect("lock symlink");

    let error = acquire_worktree_lease(repo.path()).expect_err("symlink rejected");
    assert!(matches!(error, MutationError::Unavailable { .. }));
    assert!(error.to_string().contains("writer lock path is a symlink"));
    assert_eq!(
        fs::read_to_string(outside_lock).expect("outside bytes"),
        "external bytes"
    );
}

#[test]
fn holder_metadata_without_a_usable_pid_is_reported_as_unknown() {
    let repo = TempDir::new().expect("temp repo");
    let path = repo.path().join("holder.json");

    for contents in ["not json", r#"{"pid":"text"}"#, r#"{"pid":4294967296}"#] {
        fs::write(&path, contents).expect("holder metadata");
        assert_eq!(read_holder(&path).holder_pid, None);
    }
    assert_eq!(read_holder(&repo.path().join("missing")).holder_pid, None);
}
