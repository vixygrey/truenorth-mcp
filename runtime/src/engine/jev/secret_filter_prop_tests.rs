//! Property tests for the secret-filtered state builder.
//!
//! Included from `secret_filter.rs` via `#[path]`, so `super` is the secret_filter module.
//!
//! Feature: jev-integration-eval, Property 31: secret exclusion. A denylisted path rejects
//! the entire set; otherwise content lines matching the denylist are dropped and clean lines
//! survive in the state.

use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;

/// The denylist markers used for the residual and content checks.
const SECRET_MARKERS: [&str; 2] = ["secret", "credentials"];

/// Report whether a string trips any denylist pattern, reusing the shared list.
fn trips_denylist(text: &str) -> bool {
    crate::config::secret_denylist()
        .iter()
        .any(|pattern| pattern.is_match(text))
}

/// Write a file under the repo root, creating parent directories.
fn write_file(repo: &TempDir, rel: &str, content: &str) {
    let path = repo.path().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write file");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Unsafe paths reject the entire set; otherwise clean lines survive and denylisted
    /// content lines are dropped.
    #[test]
    fn state_carries_no_secret(
        clean_line in "[a-z ]{1,20}",
        secret_marker_idx in 0usize..SECRET_MARKERS.len(),
        include_secret_path in any::<bool>(),
        include_secret_content in any::<bool>(),
    ) {
        let repo = TempDir::new().expect("temp repo");
        let mut paths: Vec<PathBuf> = Vec::new();

        // A clean file with a known clean line that must survive.
        let clean_line = clean_line.trim();
        prop_assume!(!clean_line.is_empty());
        prop_assume!(!trips_denylist(clean_line));
        write_file(&repo, "notes.txt", &format!("intro\n{clean_line}\nend"));
        paths.push(PathBuf::from("notes.txt"));

        // A denylisted-path file must reject the entire input before assembly.
        if include_secret_path {
            write_file(&repo, ".env", "API_TOKEN=abcdef123456");
            write_file(&repo, "config/secret.txt", "top secret material");
            write_file(&repo, "key.pem", "-----BEGIN KEY-----");
            write_file(&repo, "credentials.json", "{\"token\": \"xyz\"}");
            paths.push(PathBuf::from(".env"));
            paths.push(PathBuf::from("config/secret.txt"));
            paths.push(PathBuf::from("key.pem"));
            paths.push(PathBuf::from("credentials.json"));
        }

        // A clean-path file with a denylisted content line, mixed with clean lines. The
        // secret line must be dropped and the clean line kept (Requirement 9.2).
        if include_secret_content {
            let marker = SECRET_MARKERS[secret_marker_idx];
            write_file(
                &repo,
                "mixed.txt",
                &format!("keep this\nhere is a {marker} value\nkeep this too"),
            );
            paths.push(PathBuf::from("mixed.txt"));
        }

        let outcome = build_state_with_secret_filter(repo.path(), &paths);
        if include_secret_path {
            prop_assert!(matches!(outcome, Err(super::super::JevError::InvalidGuardPath { .. })), "private input must be rejected");
            return Ok(());
        }
        let state = outcome.expect("safe, redacted input builds a state");
        let serialized = state.to_string();

        // No denylist substring survives anywhere in the returned state (Requirement 9.7).
        prop_assert!(
            !trips_denylist(&serialized),
            "state must carry no denylist substring: {serialized}"
        );

        // The clean line survives.
        prop_assert!(
            serialized.contains(clean_line),
            "the clean line must survive: {serialized}"
        );

    }
}
