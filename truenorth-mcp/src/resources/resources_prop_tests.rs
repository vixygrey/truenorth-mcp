//! Property tests for cockpit relocation backward-compat.
//!
//! Included from `resources/mod.rs` via `#[path]`, so `super` is the resources module.
//!
//! Feature: agent-workspace-profiles, Property 7: cockpit relocation preserves unknown
//! fields and version. For every legacy cockpit read under `specs/` with an arbitrary set
//! of unknown fields and a `bigpowers_version` value, mapping that content onto the
//! `.agent/` model preserves every unknown field with its original key and value, and
//! preserves the `bigpowers_version` value. A malformed legacy file yields a read error
//! naming the file, leaves `.agent/` unchanged, and retains the last good content.

use std::collections::BTreeMap;
use std::fs;

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;

/// Generate a set of unknown scalar fields with distinct keys and string values.
fn unknown_fields() -> impl Strategy<Value = BTreeMap<String, String>> {
    prop::collection::btree_map("[a-z][a-z0-9_]{0,11}", "[a-zA-Z0-9 ._-]{0,23}", 0..8)
}

/// A `bigpowers_version` value, drawn from a small set of realistic version strings.
fn version() -> impl Strategy<Value = String> {
    "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}".prop_map(String::from)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A legacy specs/state.yaml read preserves every unknown field and bigpowers_version.
    #[test]
    fn legacy_state_read_preserves_fields(fields in unknown_fields(), ver in version()) {
        let repo = TempDir::new().expect("temp repo");
        let specs = repo.path().join("specs");
        fs::create_dir_all(&specs).expect("specs dir");

        // Build a legacy document as a mapping, so serialization is always valid YAML.
        // Keys the state schema validates as mappings. A string value under these would
        // be a genuinely invalid legacy file, not a preservation case, so exclude them.
        let reserved = ["bigpowers_version", "git", "handoff", "metrics"];
        let mut map = serde_yaml::Mapping::new();
        map.insert("bigpowers_version".into(), ver.clone().into());
        for (key, value) in &fields {
            if reserved.contains(&key.as_str()) {
                continue;
            }
            map.insert(key.clone().into(), value.clone().into());
        }
        let doc = serde_yaml::to_string(&serde_yaml::Value::Mapping(map)).expect("serialize");
        fs::write(specs.join("state.yaml"), &doc).expect("write legacy state");

        // Read through the resource, which falls back to the legacy file.
        let content = ResourceDoc::State
            .read_current(repo.path())
            .expect("legacy read");
        let parsed: serde_yaml::Value = serde_yaml::from_str(&content).expect("parse read");

        // The version value survives.
        prop_assert_eq!(
            parsed.get("bigpowers_version").and_then(|v| v.as_str()),
            Some(ver.as_str())
        );

        // Every unknown field survives with its original key and value.
        for (key, value) in &fields {
            if reserved.contains(&key.as_str()) {
                continue;
            }
            prop_assert_eq!(
                parsed.get(key.as_str()).and_then(|v| v.as_str()),
                Some(value.as_str()),
                "field `{}` was not preserved",
                key
            );
        }

        // The read never wrote a .agent/ state file; the legacy read does not mutate.
        prop_assert!(!repo.path().join(".agent/tasks/state.yml").exists());
    }

    /// A malformed legacy file yields an Invalid read error and retains the last good.
    #[test]
    fn malformed_legacy_retains_last_good(good_epic in "[a-z0-9]{1,8}") {
        let repo = TempDir::new().expect("temp repo");
        let specs = repo.path().join("specs");
        fs::create_dir_all(&specs).expect("specs dir");
        let cache = ResourceCache::new();

        // A first good read populates the last-good cache.
        fs::write(specs.join("state.yaml"), format!("active_epic: {good_epic}\n"))
            .expect("write good");
        let good = cache.read(ResourceDoc::State, repo.path()).expect("good read");
        prop_assert!(good.contains(&good_epic));

        // Break the legacy file. The read errors, and the cache keeps the last good.
        fs::write(specs.join("state.yaml"), "git: not-a-mapping\n").expect("write broken");
        let error = cache
            .read(ResourceDoc::State, repo.path())
            .expect_err("broken read");
        prop_assert!(matches!(error, ResourceReadError::Invalid(_)));
        prop_assert_eq!(cache.last_good(ResourceDoc::State), Some(good));
    }
}
