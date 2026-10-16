use super::*;

#[test]
fn malformed_yaml_and_non_mapping_roots_are_rejected() {
    let malformed = RulesConfigInspection::inspect_yaml("runtime: [", false);
    assert_eq!(malformed.version, None);
    assert!(malformed.blocks.is_empty());
    assert!(malformed.warnings.is_empty());
    assert_eq!(malformed.errors.len(), 1);
    assert!(malformed.errors[0].starts_with("YAML syntax error:"));

    for document in ["- runtime", "true", "42"] {
        let inspected = RulesConfigInspection::inspect_yaml(document, false);
        assert_eq!(inspected.errors, ["rules.yml root must be a YAML mapping"]);
        assert!(inspected.blocks.is_empty());
    }
}

#[test]
fn accepted_v2_configuration_classifies_every_public_block() {
    let inspected = RulesConfigInspection::inspect_yaml(
        r#"
version: 2
runtime:
  token_caps:
    skill_lean_tokens: 256
    tool_payload_tokens: 1024
features:
  ontology: true
  jev: false
protected_paths:
  - .agent
jev:
  endpoint: local
advisory:
  methodology: spec-driven
"#,
        true,
    );

    assert_eq!(inspected.version, Some(CURRENT_SCHEMA_VERSION));
    assert_eq!(
        inspected.blocks,
        BTreeMap::from([
            ("advisory".to_string(), BlockEnforcement::Advisory),
            ("features".to_string(), BlockEnforcement::Enforced),
            ("jev".to_string(), BlockEnforcement::Enforced),
            ("protected_paths".to_string(), BlockEnforcement::Enforced),
            ("runtime".to_string(), BlockEnforcement::Enforced),
        ])
    );
    assert!(inspected.warnings.is_empty());
    assert!(inspected.errors.is_empty());
}

#[test]
fn version_and_top_level_diagnostics_respect_strict_mode() {
    let yaml = r#"
schema_version: latest
runtme: {}
completely_unrelated: true
42: ignored
"#;

    let lenient = RulesConfigInspection::inspect_yaml(yaml, false);
    assert_eq!(lenient.version, None);
    assert_eq!(lenient.errors, ["`schema_version` must be an integer"]);
    assert_eq!(
        lenient.warnings,
        [
            "unknown configuration key `completely_unrelated`",
            "unknown key `runtme`; did you mean `runtime`?",
        ]
    );
    assert_eq!(
        lenient.blocks,
        BTreeMap::from([
            (
                "completely_unrelated".to_string(),
                BlockEnforcement::Unknown
            ),
            ("runtme".to_string(), BlockEnforcement::Unknown),
        ])
    );

    let strict = RulesConfigInspection::inspect_yaml(yaml, true);
    assert!(strict.warnings.is_empty());
    assert_eq!(strict.errors.len(), 3);
    assert!(
        strict
            .errors
            .contains(&"`schema_version` must be an integer".to_string())
    );
    assert!(
        strict
            .errors
            .contains(&"unknown key `runtme`; did you mean `runtime`?".to_string())
    );
    assert!(
        strict
            .errors
            .contains(&"unknown configuration key `completely_unrelated`".to_string())
    );
}

#[test]
fn legacy_advisory_blocks_are_migrated_differently_for_v1_and_v2() {
    let legacy = RulesConfigInspection::inspect_yaml("methodology: tdd", false);
    assert_eq!(
        legacy.blocks.get("methodology"),
        Some(&BlockEnforcement::LegacyV1)
    );
    assert_eq!(legacy.errors, Vec::<String>::new());
    assert_eq!(legacy.warnings.len(), 1);
    assert!(legacy.warnings[0].contains("legacy v1 advisory metadata"));

    let lenient_v2 = RulesConfigInspection::inspect_yaml("version: 2\nquality_gates: {}", false);
    assert_eq!(
        lenient_v2.blocks.get("quality_gates"),
        Some(&BlockEnforcement::Unknown)
    );
    assert_eq!(
        lenient_v2.warnings,
        ["`quality_gates` should be moved under `advisory:` in version 2"]
    );
    assert!(lenient_v2.errors.is_empty());

    let strict_v2 = RulesConfigInspection::inspect_yaml("version: 2\nquality_gates: {}", true);
    assert!(strict_v2.warnings.is_empty());
    assert_eq!(
        strict_v2.errors,
        ["`quality_gates` should be moved under `advisory:` in version 2"]
    );
}

#[test]
fn runtime_and_features_validate_shape_types_and_unknown_keys() {
    let scalar_blocks = RulesConfigInspection::inspect_yaml(
        "runtime: disabled\nfeatures: []\nadvisory: false",
        false,
    );
    assert_eq!(
        scalar_blocks.errors,
        [
            "`advisory` must be a mapping",
            "`features` must be a mapping",
            "`runtime` must be a mapping",
        ]
    );

    let lenient = RulesConfigInspection::inspect_yaml(
        r#"
runtime:
  timeout: 30
features:
  ontology: enabled
  jev: 1
  ontolgy: true
  preview: false
"#,
        false,
    );
    assert_eq!(
        lenient.errors,
        [
            "`features.ontology` must be a boolean (true or false)",
            "`features.jev` must be a boolean (true or false)",
        ]
    );
    assert_eq!(
        lenient.warnings,
        [
            "unknown key `features.ontolgy`; did you mean `features.ontology`?",
            "unknown key `features.preview`",
            "unknown key `runtime.timeout`",
        ]
    );

    let strict = RulesConfigInspection::inspect_yaml(
        "runtime:\n  timeout: 30\nfeatures:\n  ontolgy: true\n  preview: false",
        true,
    );
    assert!(strict.warnings.is_empty());
    assert_eq!(strict.errors.len(), 3);
    assert!(
        strict
            .errors
            .contains(&"unknown key `runtime.timeout`".to_string())
    );
    assert!(strict.errors.contains(
        &"unknown key `features.ontolgy`; did you mean `features.ontology`?".to_string()
    ));
    assert!(
        strict
            .errors
            .contains(&"unknown key `features.preview`".to_string())
    );
}

#[test]
fn senior_developer_is_recognized_and_strictly_typed() {
    let valid = RulesConfigInspection::inspect_yaml("features:\n  senior_developer: false\n", true);
    assert!(valid.errors.is_empty());
    assert!(valid.warnings.is_empty());

    let invalid = RulesConfigInspection::inspect_yaml(
        "features:\n  senior_developer: disabled\n  senior_developr: true\n",
        true,
    );
    assert!(
        invalid
            .errors
            .contains(&"`features.senior_developer` must be a boolean (true or false)".to_string())
    );
    assert!(
        invalid.errors.contains(
            &"unknown key `features.senior_developr`; did you mean `features.senior_developer`?"
                .to_string()
        )
    );
}

#[test]
fn non_string_nested_yaml_keys_do_not_become_configuration_names() {
    let inspected = RulesConfigInspection::inspect_yaml(
        r#"
runtime:
  1: ignored
features:
  2: true
token_caps:
  3: 4
"#,
        true,
    );

    assert!(inspected.warnings.is_empty());
    assert!(inspected.errors.is_empty());
}

#[test]
fn token_caps_enforce_positive_integers_and_diagnose_nearby_names() {
    let inspected = RulesConfigInspection::inspect_yaml(
        r#"
token_caps:
  skill_lean_tokens: 0
  tool_payload_tokens: many
  lean_limit: 3
  payload_limit: 4
  other_limit: 5
runtime:
  token_caps: disabled
"#,
        true,
    );

    assert_eq!(
        inspected.errors,
        [
            "`runtime.token_caps` must be a mapping",
            "`token_caps.skill_lean_tokens` must be greater than zero",
            "`token_caps.tool_payload_tokens` must be a positive integer",
        ]
    );
    assert_eq!(
        inspected.warnings,
        [
            "unknown key `token_caps.lean_limit`; did you mean `token_caps.skill_lean_tokens`?",
            "unknown key `token_caps.payload_limit`; did you mean `token_caps.tool_payload_tokens`?",
            "unknown key `token_caps.other_limit`",
        ]
    );
}

#[test]
fn v2_top_level_token_caps_remain_accepted_with_migration_warning() {
    let inspected = RulesConfigInspection::inspect_yaml(
        "version: 2\ntoken_caps:\n  skill_lean_tokens: 1",
        true,
    );

    assert_eq!(
        inspected.blocks.get("token_caps"),
        Some(&BlockEnforcement::Enforced)
    );
    assert_eq!(
        inspected.warnings,
        [
            "top-level `token_caps` in version 2 is supported for compatibility; prefer `runtime.token_caps`."
        ]
    );
    assert!(inspected.errors.is_empty());
}

#[test]
fn enforcement_values_have_stable_wire_names() {
    let values = [
        (BlockEnforcement::Enforced, "enforced"),
        (BlockEnforcement::Advisory, "advisory"),
        (BlockEnforcement::LegacyV1, "legacy_v1"),
        (BlockEnforcement::Unknown, "unknown"),
    ];

    for (value, wire_name) in values {
        assert_eq!(serde_yaml::to_string(&value).unwrap().trim(), wire_name);
        assert_eq!(
            serde_yaml::from_str::<BlockEnforcement>(wire_name).unwrap(),
            value
        );
    }
}
