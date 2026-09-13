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

/// Seed a complete, valid `.agent/` layout under a fresh temp repository root.
fn seed_valid_layout() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(AGENT_DIR);

    for (entry, kind) in REQUIRED_ENTRIES {
        let path = agent.join(entry);
        match kind {
            EntryKind::Dir => fs::create_dir_all(&path).expect("create dir entry"),
            EntryKind::File => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).expect("create parent");
                }
                fs::write(&path, "seed\n").expect("write file entry");
            }
        }
    }

    repo
}

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

#[test]
fn read_layout_accepts_a_complete_layout() {
    let repo = seed_valid_layout();

    let layout = read_layout(repo.path()).expect("valid layout");
    assert_eq!(layout.agent_root, repo.path().join(AGENT_DIR));
}

#[test]
fn read_layout_names_an_absent_required_entry() {
    let repo = seed_valid_layout();
    // Remove a required file to break the contract.
    fs::remove_file(repo.path().join(AGENT_DIR).join("tasks/state.yml"))
        .expect("remove required file");

    let error = read_layout(repo.path()).expect_err("must reject");
    match error {
        LayoutError::MissingEntry { absent } => {
            assert_eq!(absent, ".agent/tasks/state.yml");
        }
        other => panic!("expected MissingEntry, got {other:?}"),
    }
}

#[test]
fn read_layout_errors_when_the_contract_file_is_absent() {
    let repo = TempDir::new().expect("temp repo");
    // No `.agent/layout.yml` at all.
    let error = read_layout(repo.path()).expect_err("must reject");
    assert!(matches!(error, LayoutError::Io { .. }));
}

#[test]
fn layout_cache_retains_the_last_valid_contract_on_a_later_broken_read() {
    let repo = seed_valid_layout();
    let cache = LayoutCache::new();

    let first = cache.read(repo.path()).expect("first read valid");
    assert_eq!(cache.last_valid(), Some(first.clone()));

    // Break the contract, then read again.
    fs::remove_file(repo.path().join(AGENT_DIR).join("memories/glossary.md"))
        .expect("remove required file");
    let error = cache.read(repo.path()).expect_err("second read invalid");
    assert!(matches!(error, LayoutError::MissingEntry { .. }));

    // The last valid contract is retained (Requirement 1.12).
    assert_eq!(cache.last_valid(), Some(first));
}
