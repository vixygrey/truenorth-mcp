//! Unit tests for the ADR resource reader.
//!
//! Included from `adr.rs` via `#[path]`, so `super` is the adr module.
//!
//! Requirements: 9.2, 9.5, 9.6.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::resources::{ResourceCache, ResourceDoc};

/// Seed `specs/adr/<name>` with the given content under a fresh temp repo root.
fn seed_adr(repo: &TempDir, name: &str, content: &str) {
    let dir = repo.path().join("specs").join("adr");
    fs::create_dir_all(&dir).expect("create adr dir");
    fs::write(dir.join(name), content).expect("write adr file");
}

#[test]
fn reads_and_concatenates_in_filename_order() {
    let repo = TempDir::new().expect("temp repo");
    // Seed out of order to confirm the read sorts by name.
    seed_adr(&repo, "0003-third.md", "# Third\n");
    seed_adr(&repo, "0001-first.md", "# First\n");
    seed_adr(&repo, "0002-second.md", "# Second\n");

    let content = read_adr_dir(&repo.path().join("specs").join("adr")).expect("read adr");
    let first = content.find("# First").expect("first present");
    let second = content.find("# Second").expect("second present");
    let third = content.find("# Third").expect("third present");
    assert!(
        first < second && second < third,
        "ADRs concatenate in filename order"
    );
}

#[test]
fn ignores_non_markdown_files() {
    let repo = TempDir::new().expect("temp repo");
    seed_adr(&repo, "0001-first.md", "# First\n");
    seed_adr(&repo, "notes.txt", "not an adr\n");

    let content = read_adr_dir(&repo.path().join("specs").join("adr")).expect("read adr");
    assert!(content.contains("# First"));
    assert!(!content.contains("not an adr"));
}

#[test]
fn absent_directory_is_not_found() {
    // Requirement 9.5: an absent specs/adr/ returns a NotFound naming the directory.
    let repo = TempDir::new().expect("temp repo");
    let error = read_adr_dir(&repo.path().join("specs").join("adr")).expect_err("absent");
    match error {
        ResourceReadError::NotFound(name) => assert_eq!(name, "specs/adr/"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn read_current_routes_adr_through_the_directory_reader() {
    let repo = TempDir::new().expect("temp repo");
    seed_adr(&repo, "0001-first.md", "# Decision\n");

    let content = ResourceDoc::Adr
        .read_current(repo.path())
        .expect("read adr resource");
    assert!(content.contains("# Decision"));
}

#[test]
fn cache_retains_last_good_when_the_directory_disappears() {
    // Requirement 9.6: a later broken read retains the last successfully read content.
    let repo = TempDir::new().expect("temp repo");
    seed_adr(&repo, "0001-first.md", "# Decision\n");
    let cache = ResourceCache::new();

    let good = cache
        .read(ResourceDoc::Adr, repo.path())
        .expect("first read");
    assert!(good.contains("# Decision"));

    // Remove the directory. The read errors, but the cache keeps the last good content.
    fs::remove_dir_all(repo.path().join("specs").join("adr")).expect("remove adr dir");
    let error = cache
        .read(ResourceDoc::Adr, repo.path())
        .expect_err("broken read");
    assert!(matches!(error, ResourceReadError::NotFound(_)));
    assert!(
        cache
            .last_good(ResourceDoc::Adr)
            .expect("retained")
            .contains("# Decision")
    );
}

#[test]
fn adr_resource_is_read_only_and_maps_from_uri() {
    // The ADR resource is listed and resolvable, and it is markdown.
    assert_eq!(
        ResourceDoc::from_uri("truenorth://adr"),
        Some(ResourceDoc::Adr)
    );
    assert_eq!(ResourceDoc::Adr.mime_type(), "text/markdown");
    // There is no write path for the ADR resource; the resource layer exposes only reads.
    // The write guard rejects any specs/ target, asserted in the write-guard property test.
}
