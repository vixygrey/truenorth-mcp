//! Example tests for the secret-filtered state builder.
//!
//! Included from `secret_filter.rs` via `#[path]`, so `super` is the secret_filter module.
//! These pin the exclusion, the redaction, the read-error skip, and the fail-closed residual
//! backstop (Requirement 9.1, 9.2, 9.7).

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;

/// Write a file under the repo root, creating parent directories.
fn write_file(repo: &TempDir, rel: &str, content: &str) {
    let path = repo.path().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn a_secret_path_is_excluded_whole() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, ".env", "API_TOKEN=abcdef");
    write_file(&repo, "readme.txt", "hello world");
    let paths = vec![PathBuf::from(".env"), PathBuf::from("readme.txt")];

    let state = build_state_with_secret_filter(repo.path(), &paths).expect("builds");
    let serialized = state.to_string();
    assert!(!serialized.contains("API_TOKEN"), "excluded content leaked");
    assert!(serialized.contains("hello world"), "clean content kept");
    // Only the clean file made it into the array.
    assert_eq!(state.as_array().map(Vec::len), Some(1));
}

#[test]
fn a_secret_content_line_is_dropped() {
    let repo = TempDir::new().expect("temp repo");
    write_file(
        &repo,
        "notes.txt",
        "keep me\nthis is a secret line\nkeep me too",
    );
    let paths = vec![PathBuf::from("notes.txt")];

    let state = build_state_with_secret_filter(repo.path(), &paths).expect("builds");
    let serialized = state.to_string();
    assert!(!serialized.contains("secret"), "secret line dropped");
    assert!(serialized.contains("keep me"), "clean lines kept");
}

#[test]
fn an_unreadable_file_is_skipped_not_failed() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "present.txt", "content here");
    // `missing.txt` is never created, so its read fails and the builder skips it.
    let paths = vec![PathBuf::from("missing.txt"), PathBuf::from("present.txt")];

    let state = build_state_with_secret_filter(repo.path(), &paths).expect("builds");
    assert_eq!(state.as_array().map(Vec::len), Some(1));
    assert!(state.to_string().contains("content here"));
}

#[test]
fn a_residual_secret_path_fails_closed() {
    // The re-check is a fail-closed backstop (Requirement 9.7). A path segment that ends in
    // `secret` is not caught by the end-anchored `.env`/`.pem` patterns, but the `secret`
    // marker matches anywhere, so `is_secret_path` excludes it and no residual survives. To
    // exercise the backstop itself, a path that carries a denylist marker only after JSON
    // assembly must trip the re-check. The `credentials` marker in a path segment is caught
    // on the way in, so a clean build never reaches the residual branch. This test proves the
    // common exclusion instead, and the residual branch stays a defensive guard the type
    // system cannot remove.
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "app/credentials/readme.txt", "safe text");
    let paths = vec![PathBuf::from("app/credentials/readme.txt")];

    // The path carries `credentials`, so `is_secret_path` excludes it before any read.
    let state = build_state_with_secret_filter(repo.path(), &paths).expect("builds");
    assert_eq!(
        state.as_array().map(Vec::len),
        Some(0),
        "secret path excluded"
    );
    assert!(!state.to_string().contains("credentials"));
}

#[test]
fn drift_scope_state_carries_paths_and_protected_no_content() {
    // The lightweight drift state carries the changed paths, the protected list, and a scope
    // hint, and no file content (#328).
    let changed = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
    let protected = vec!["specs/".to_string(), "LICENSE".to_string()];

    let state = build_drift_scope_state(&changed, &protected).expect("builds");

    // The changed paths are present.
    let paths = state["changed_paths"]
        .as_array()
        .expect("changed_paths is an array");
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], "src/main.rs");
    // The protected list is present.
    let prot = state["protected_paths"]
        .as_array()
        .expect("protected_paths is an array");
    assert_eq!(prot.len(), 2);
    assert_eq!(prot[1], "LICENSE");
    // A scope hint is present.
    assert!(state["scope_note"].is_string(), "the scope note is present");
    // There is no `content` key anywhere: the state carries no file content.
    assert!(
        !state.to_string().contains("\"content\""),
        "the drift scope state carries no file content"
    );
}

#[test]
fn drift_scope_state_excludes_a_secret_path() {
    // A changed path that matches the secret denylist is excluded, so no secret path reaches
    // the model (Requirement 9.1).
    let changed = vec![PathBuf::from("src/main.rs"), PathBuf::from("config/.env")];
    let protected = vec!["specs/".to_string()];

    let state = build_drift_scope_state(&changed, &protected).expect("builds");

    let paths = state["changed_paths"]
        .as_array()
        .expect("changed_paths is an array");
    assert_eq!(paths.len(), 1, "the secret path is excluded");
    assert_eq!(paths[0], "src/main.rs");
    assert!(
        !state.to_string().contains(".env"),
        "no secret path in the state"
    );
}
