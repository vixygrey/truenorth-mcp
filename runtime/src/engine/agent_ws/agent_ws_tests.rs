//! Unit tests for the agent workspace guard.
//!
//! Included from `agent_ws.rs` via `#[path]`, so `super` is the agent_ws module.
//!
//! These cover the write guard's accept and reject paths, the telemetry read exclusion,
//! and the layout-contract read (present, absent-entry, and last-valid retention). The
//! exhaustive generated coverage of the guard lives in the Property 6 proptest.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;

#[test]
fn write_under_agent_writes_a_path_under_agent() {
    let repo = TempDir::new().expect("temp repo");
    let rel = Path::new("tasks/state.yml");

    write_under_agent(repo.path(), rel, "phase: execute\n").expect("write under .agent");

    let written = repo.path().join(AGENT_DIR).join(rel);
    let contents = fs::read_to_string(&written).expect("read back");
    assert_eq!(contents, "phase: execute\n");
}

#[test]
fn write_under_agent_creates_missing_parent_dirs() {
    let repo = TempDir::new().expect("temp repo");
    let rel = Path::new("tasks/nested/deep/file.yml");

    write_under_agent(repo.path(), rel, "x: 1\n").expect("write nested");

    assert!(repo.path().join(AGENT_DIR).join(rel).is_file());
}

#[test]
fn write_under_agent_rejects_parent_traversal_and_writes_nothing() {
    let repo = TempDir::new().expect("temp repo");
    // Target `specs/leak.yml`, escaping `.agent/` via `..`.
    let rel = Path::new("../specs/leak.yml");

    let error = write_under_agent(repo.path(), rel, "leak\n").expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));

    // No file was written anywhere under the repo root.
    assert!(!repo.path().join("specs").join("leak.yml").exists());
}

#[test]
fn write_under_agent_rejects_an_absolute_path() {
    let repo = TempDir::new().expect("temp repo");
    let outside = TempDir::new().expect("outside dir");
    let abs = outside.path().join("leak.yml");

    let error = write_under_agent(repo.path(), &abs, "leak\n").expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));
    assert!(!abs.exists());
}

#[test]
fn write_repo_seed_writes_a_root_doc_when_allowed() {
    let repo = TempDir::new().expect("temp repo");
    let rel = Path::new(".githooks/commit-msg");

    write_repo_seed(repo.path(), rel, "#!/bin/sh\n", true).expect("seed allowed");

    let written = repo.path().join(rel);
    assert_eq!(
        fs::read_to_string(&written).expect("read back"),
        "#!/bin/sh\n"
    );
}

#[test]
fn write_repo_seed_rejects_when_not_allowed_and_writes_nothing() {
    let repo = TempDir::new().expect("temp repo");
    let rel = Path::new("AGENTS.md");

    let error = write_repo_seed(repo.path(), rel, "seed\n", false).expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));
    assert!(!repo.path().join(rel).exists());
}

#[test]
fn write_repo_seed_rejects_traversal_outside_the_repo() {
    let repo = TempDir::new().expect("temp repo");
    let rel = Path::new("../escape.txt");

    let error = write_repo_seed(repo.path(), rel, "x\n", true).expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));
}

#[test]
fn write_under_agent_rejects_a_specs_adr_target_and_leaves_bytes_unchanged() {
    // Property 6 for the ADR resource: a write aimed at specs/adr/ is rejected, and the
    // existing ADR bytes stay unchanged. This is the write-guard half of the read-only
    // ADR guarantee (Requirement 9.4).
    let repo = TempDir::new().expect("temp repo");
    let adr = repo.path().join("specs").join("adr");
    fs::create_dir_all(&adr).expect("create adr dir");
    fs::write(adr.join("0001-first.md"), "# Decision\n").expect("seed adr");

    // A relative target that escapes .agent/ into specs/adr/.
    let rel = Path::new("../specs/adr/0001-first.md");
    let error = write_under_agent(repo.path(), rel, "tampered\n").expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));

    // The ADR file is unchanged.
    assert_eq!(
        fs::read_to_string(adr.join("0001-first.md")).expect("read adr"),
        "# Decision\n"
    );
}

#[cfg(unix)]
#[test]
fn write_under_agent_rejects_a_symlinked_directory_escape() {
    // #186: a symlinked directory inside `.agent/` that points outside must not let a
    // lexically-valid target escape on the write. The lexical guard passes `link/leak.yml`
    // (no `..`, not absolute), so the symlink resolution is what rejects it.
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("temp repo");
    let outside = TempDir::new().expect("outside dir");
    let agent = repo.path().join(AGENT_DIR);
    fs::create_dir_all(&agent).expect("create .agent");
    // `.agent/link` -> the outside directory.
    symlink(outside.path(), agent.join("link")).expect("create symlink");

    let rel = Path::new("link/leak.yml");
    let error = write_under_agent(repo.path(), rel, "leak\n").expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));

    // Nothing was written through the symlink into the outside directory.
    assert!(!outside.path().join("leak.yml").exists());
}

#[cfg(unix)]
#[test]
fn write_repo_seed_rejects_a_symlinked_directory_escape() {
    // #186: the audited seed path applies the same symlink check against the repo root.
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().expect("temp repo");
    let outside = TempDir::new().expect("outside dir");
    // `<repo>/link` -> the outside directory.
    symlink(outside.path(), repo.path().join("link")).expect("create symlink");

    let rel = Path::new("link/leak.txt");
    let error = write_repo_seed(repo.path(), rel, "leak\n", true).expect_err("must reject");
    assert!(matches!(error, WriteGuardError::OutsideAgent { .. }));
    assert!(!outside.path().join("leak.txt").exists());
}

#[cfg(unix)]
#[test]
fn write_under_agent_allows_a_normal_write_after_the_symlink_check() {
    // The symlink check must not break an ordinary write into a fresh `.agent/` tree,
    // including a not-yet-created nested target.
    let repo = TempDir::new().expect("temp repo");
    write_under_agent(repo.path(), Path::new("tasks/nested/x.yml"), "x: 1\n")
        .expect("normal nested write");
    assert!(
        repo.path()
            .join(AGENT_DIR)
            .join("tasks/nested/x.yml")
            .is_file()
    );
}

#[test]
fn is_excluded_read_is_true_only_for_telemetry() {
    // Repo-relative and agent-relative telemetry paths are excluded.
    assert!(is_excluded_read(Path::new(".agent/telemetry/runs.yml")));
    assert!(is_excluded_read(Path::new("telemetry/runs.yml")));
    assert!(is_excluded_read(Path::new(".agent/telemetry")));

    // Other areas are not excluded.
    assert!(!is_excluded_read(Path::new(".agent/tasks/state.yml")));
    assert!(!is_excluded_read(Path::new("tasks/state.yml")));
    // A name that merely contains "telemetry" as a substring is not the telemetry area.
    assert!(!is_excluded_read(Path::new(
        ".agent/tasks/telemetry-notes.md"
    )));
}
