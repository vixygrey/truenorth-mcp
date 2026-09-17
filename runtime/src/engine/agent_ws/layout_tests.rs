//! Tests for the `.agent/` layout contract.
//!
//! Included from `layout.rs` via `#[path]`, so `super` is the layout module. These reach
//! the private `REQUIRED_ENTRIES` and `EntryKind` through `use super::*`.
//!
//! Requirements: 1.11, 1.12.

use std::fs;

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
