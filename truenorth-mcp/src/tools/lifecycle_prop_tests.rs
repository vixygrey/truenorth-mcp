//! Property tests for the neutral grouping key in `record_task`.
//!
//! Included from `lifecycle.rs` via `#[path]`, so `super` is the lifecycle module.
//!
//! Feature: agent-workspace-profiles, Property 8: neutral grouping preserves legacy
//! mapping and fields. For every legacy `epic_id`, the resolved grouping carries
//! `group_kind = epic` and `group_id = epic_id`. A cockpit read preserves a legacy
//! `active_epic` value and every unknown field.
//!
//! Feature: agent-workspace-profiles, Property 9: grouping-key validation rejects invalid
//! input and preserves state. Across the five profiles, a call is accepted or rejected per
//! the rule, and the release-plan file is unchanged on reject.

use std::fs;

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;
use crate::engine::cockpit::{record_task, release_plan_path};
use crate::engine::profile::ALL_PROFILES;

/// Build RecordTaskArgs with the given grouping fields.
fn make_args(
    group_id: Option<String>,
    group_kind: Option<String>,
    epic_id: Option<String>,
) -> RecordTaskArgs {
    RecordTaskArgs {
        group_id,
        group_kind,
        epic_id,
        task_name: "A task".to_string(),
        verify_command: "cargo test".to_string(),
    }
}

/// A repo whose `.agent/profile.yml` names the given profile.
fn repo_with_profile(name: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(".agent");
    fs::create_dir_all(&agent).expect("create .agent");
    fs::write(agent.join("profile.yml"), format!("profile: {name}\n")).expect("write profile");
    repo
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Property 8: a legacy epic_id maps to group_kind=epic and group_id=epic_id.
    #[test]
    fn legacy_epic_id_maps_to_epic(suffix in "[a-z0-9-]{0,8}", n in 0u32..9999) {
        let epic_id = format!("e{n}{suffix}");
        // The generated id always matches the legacy pattern ^e[0-9]+([a-z0-9-]*)?$.
        let repo = repo_with_profile("epic-based");
        let grouping = resolve_grouping(repo.path(), &make_args(None, None, Some(epic_id.clone())))
            .expect("legacy maps");
        prop_assert_eq!(grouping.id.as_deref(), Some(epic_id.as_str()));
        prop_assert_eq!(grouping.kind.as_deref(), Some("epic"));
    }

    /// Property 8: a cockpit read preserves a legacy active_epic and unknown fields.
    #[test]
    fn record_task_preserves_active_epic_and_unknown_fields(
        active_epic in "[a-z0-9]{1,8}",
        extra in "[a-z][a-z0-9_]{0,10}",
        extra_val in "[a-zA-Z0-9 ._-]{0,16}",
    ) {
        // Skip a generated key that collides with a written or validated field.
        prop_assume!(!["group_id", "group_kind", "task_name", "verify_command",
                       "tasks", "build_order", "release"].contains(&extra.as_str()));

        let repo = TempDir::new().expect("temp repo");
        let plan = release_plan_path(repo.path());
        fs::create_dir_all(plan.parent().unwrap()).expect("tasks dir");

        // Build the legacy plan through the serializer, so any generated value is a valid
        // scalar. This isolates the property to field preservation, not YAML quoting.
        let mut seed_map = serde_yaml::Mapping::new();
        seed_map.insert("active_epic".into(), active_epic.clone().into());
        seed_map.insert(extra.clone().into(), extra_val.clone().into());
        let seeded = serde_yaml::Value::Mapping(seed_map);
        fs::write(&plan, serde_yaml::to_string(&seeded).expect("serialize seed")).expect("seed");

        record_task(repo.path(), Some("e80"), Some("epic"), "A task", "cargo test")
            .expect("record");

        let written = fs::read_to_string(&plan).expect("read plan");
        let value: serde_yaml::Value = serde_yaml::from_str(&written).expect("parse");
        prop_assert_eq!(value.get("active_epic"), seeded.get("active_epic"));
        prop_assert_eq!(value.get(extra.as_str()), seeded.get(extra.as_str()));
        prop_assert!(value.get("tasks").and_then(|v| v.as_sequence()).is_some());
    }

    /// Property 9: validation accepts or rejects per the rule, and a reject leaves the
    /// release-plan file unchanged.
    #[test]
    fn grouping_validation_matches_rule_and_preserves_state(
        profile_idx in 0usize..ALL_PROFILES.len(),
        group_id in prop::option::of("[a-z0-9-]{1,12}"),
        kind_choice in 0usize..6,
    ) {
        let profile = ALL_PROFILES[profile_idx];
        let repo = repo_with_profile(profile.name);

        // Seed a release plan so a reject can be checked against unchanged bytes.
        let plan = release_plan_path(repo.path());
        fs::create_dir_all(plan.parent().unwrap()).expect("tasks dir");
        let seed = "build_order:\n- e01\n";
        fs::write(&plan, seed).expect("seed plan");

        // Choose a group_kind: indices 0..4 are valid kinds, 4 is a bogus kind, 5 is None.
        let all_kinds = ["epic", "sprint", "milestone", "ticket"];
        let group_kind = match kind_choice {
            0..=3 => Some(all_kinds[kind_choice].to_string()),
            4 => Some("saga".to_string()),
            _ => None,
        };

        let args = make_args(group_id.clone(), group_kind.clone(), None);
        let outcome = resolve_grouping(repo.path(), &args);

        // Independently compute the expected accept/reject per the requirement rules.
        let kind_ok = match &group_kind {
            None => true,
            Some(k) => all_kinds.contains(&k.as_str())
                && profile.vocab.as_kind_str() == Some(k.as_str()),
        };
        let id_ok = group_id.as_ref().map(|id| !id.is_empty() && id.chars().count() <= 200)
            .unwrap_or(true);
        let required_ok = !(matches!(profile.rule, crate::engine::profile::GroupingRule::Required)
            && group_id.is_none());
        let expected_ok = kind_ok && id_ok && required_ok;

        prop_assert_eq!(outcome.is_ok(), expected_ok,
            "profile={} group_id={:?} group_kind={:?}", profile.name, group_id, group_kind);

        // On a reject, the resolver never writes, so the plan file is unchanged.
        if outcome.is_err() {
            let after = fs::read_to_string(&plan).expect("read plan");
            prop_assert_eq!(after, seed);
        }
    }
}
