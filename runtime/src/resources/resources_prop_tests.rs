//! Property tests for cockpit state preservation.
//!
//! Property 7: an unknown field in `.agent/tasks/state.json` survives a resource read
//! with its original key and value. A malformed state document yields a read error
//! naming the file, leaves the last good content retained, and does not mutate disk.

use std::collections::BTreeMap;
use std::fs;

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;

/// Generate a set of unknown scalar fields with distinct keys and string values.
///
/// The generated keys cover any unknown field a hand-edited document can carry. The model
/// names no field for any of them; they are all preserved through the same catch-all.
fn unknown_fields() -> impl Strategy<Value = BTreeMap<String, String>> {
    prop::collection::btree_map("[a-z][a-z0-9_]{0,11}", "[a-zA-Z0-9 ._-]{0,23}", 0..8)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A `state.json` read preserves every unknown field, whatever its key.
    #[test]
    fn state_read_preserves_unknown_fields(fields in unknown_fields()) {
        let repo = TempDir::new().expect("temp repo");
        let tasks = repo.path().join(".agent/tasks");
        fs::create_dir_all(&tasks).expect("tasks dir");

        // Build the document as a mapping, so serialization is always valid JSON. Keys
        // the state schema validates as mappings would make a string value genuinely
        // invalid, not a preservation case, so exclude them.
        let reserved = ["git", "handoff", "metrics"];
        let mut map = serde_json::Map::new();
        for (key, value) in &fields {
            if reserved.contains(&key.as_str()) {
                continue;
            }
            map.insert(key.clone(), serde_json::Value::String(value.clone()));
        }
        let doc = crate::engine::cockpit_json::to_string(&map).expect("serialize");
        fs::write(tasks.join("state.json"), &doc).expect("write state");

        let content = ResourceDoc::State.read_current(repo.path()).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse read");

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
    }

    /// A malformed state document yields an Invalid read error and retains the last good.
    #[test]
    fn malformed_state_retains_last_good(good_epic in "[a-z0-9]{1,8}") {
        let repo = TempDir::new().expect("temp repo");
        let tasks = repo.path().join(".agent/tasks");
        fs::create_dir_all(&tasks).expect("tasks dir");
        let cache = ResourceCache::new();

        // A first good read populates the last-good cache.
        fs::write(
            tasks.join("state.json"),
            format!("{{\"active_epic\": \"{good_epic}\"}}"),
        )
        .expect("write good");
        let good = cache.read(ResourceDoc::State, repo.path()).expect("good read");
        prop_assert!(good.contains(&good_epic));

        // Break the state document. The read errors, and the cache keeps the last good.
        fs::write(tasks.join("state.json"), "{\"git\": \"not-a-mapping\"}").expect("write broken");
        let error = cache
            .read(ResourceDoc::State, repo.path())
            .expect_err("broken read");
        prop_assert!(matches!(error, ResourceReadError::Invalid(_)));
        prop_assert_eq!(cache.last_good(ResourceDoc::State), Some(good));
    }
}
