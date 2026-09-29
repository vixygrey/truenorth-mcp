//! Schema and validation for `.agent/config/rules.yml` (ADR-0017, issue #432).
//!
//! Separates enforced runtime settings from advisory conventions, validates
//! recognized blocks, identifies misspelled keys, and provides strict-mode validation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Supported schema versions for rules.yml.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Known top-level blocks in rules.yml v2.
pub const V2_TOP_LEVEL_BLOCKS: [&str; 6] = [
    "version",
    "runtime",
    "features",
    "protected_paths",
    "jev",
    "advisory",
];

/// Known legacy top-level blocks from v1 that belong under advisory in v2.
pub const V1_ADVISORY_BLOCKS: [&str; 5] = [
    "methodology",
    "quality_gates",
    "approval_gates",
    "lint_standards",
    "secret_denylist",
];

/// Enforcement status of a configuration block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockEnforcement {
    Enforced,
    Advisory,
    LegacyV1,
    Unknown,
}

/// Diagnostic inspection of `.agent/config/rules.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RulesConfigInspection {
    /// The declared schema version (e.g. 2, 1, or absent for legacy).
    pub version: Option<u32>,
    /// The recognized or detected blocks and their enforcement classification.
    pub blocks: BTreeMap<String, BlockEnforcement>,
    /// Misspelled or unrecognized key warnings.
    pub warnings: Vec<String>,
    /// Errors that block startup or fail strict mode.
    pub errors: Vec<String>,
}

impl RulesConfigInspection {
    /// Inspect the raw parsed YAML mapping of a rules.yml file.
    pub fn inspect_yaml(raw_yaml: &str, strict: bool) -> Self {
        let parsed: Result<Value, serde_yaml::Error> = serde_yaml::from_str(raw_yaml);
        let value = match parsed {
            Ok(val) => val,
            Err(err) => {
                return Self {
                    version: None,
                    blocks: BTreeMap::new(),
                    warnings: Vec::new(),
                    errors: vec![format!("YAML syntax error: {err}")],
                };
            }
        };

        let mapping = match value.as_mapping() {
            Some(map) => map,
            None => {
                return Self {
                    version: None,
                    blocks: BTreeMap::new(),
                    warnings: Vec::new(),
                    errors: vec!["rules.yml root must be a YAML mapping".to_string()],
                };
            }
        };

        let mut version = None;
        let mut blocks = BTreeMap::new();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        let mut top_keys = BTreeSet::new();
        for (k, v) in mapping {
            if let Some(key_str) = k.as_str() {
                top_keys.insert(key_str.to_string());
                if key_str == "version" || key_str == "schema_version" {
                    if let Some(v_int) = v.as_u64() {
                        version = Some(v_int as u32);
                    } else {
                        errors.push(format!("`{key_str}` must be an integer"));
                    }
                }
            }
        }

        let is_v2 = version == Some(2);

        for key in &top_keys {
            match key.as_str() {
                "version" | "schema_version" => {}
                "runtime" => {
                    blocks.insert("runtime".to_string(), BlockEnforcement::Enforced);
                    if let Some(runtime_val) = mapping.get(Value::from("runtime")) {
                        validate_runtime_block(runtime_val, &mut warnings, &mut errors, strict);
                    }
                }
                "features" => {
                    blocks.insert("features".to_string(), BlockEnforcement::Enforced);
                    if let Some(feat_val) = mapping.get(Value::from("features")) {
                        validate_features_block(feat_val, &mut warnings, &mut errors, strict);
                    }
                }
                "token_caps" => {
                    if is_v2 {
                        blocks.insert("token_caps".to_string(), BlockEnforcement::Enforced);
                        warnings.push("top-level `token_caps` in version 2 is supported for compatibility; prefer `runtime.token_caps`.".to_string());
                    } else {
                        blocks.insert("token_caps".to_string(), BlockEnforcement::Enforced);
                    }
                    if let Some(tc_val) = mapping.get(Value::from("token_caps")) {
                        validate_token_caps_block(tc_val, "token_caps", &mut warnings, &mut errors);
                    }
                }
                "protected_paths" => {
                    blocks.insert("protected_paths".to_string(), BlockEnforcement::Enforced);
                }
                "jev" => {
                    blocks.insert("jev".to_string(), BlockEnforcement::Enforced);
                }
                "advisory" => {
                    blocks.insert("advisory".to_string(), BlockEnforcement::Advisory);
                    if let Some(adv_val) = mapping.get(Value::from("advisory")) {
                        validate_advisory_block(adv_val, &mut warnings, &mut errors, strict);
                    }
                }
                v1_key if V1_ADVISORY_BLOCKS.contains(&v1_key) => {
                    if is_v2 {
                        blocks.insert(v1_key.to_string(), BlockEnforcement::Unknown);
                        let msg =
                            format!("`{v1_key}` should be moved under `advisory:` in version 2");
                        if strict {
                            errors.push(msg);
                        } else {
                            warnings.push(msg);
                        }
                    } else {
                        blocks.insert(v1_key.to_string(), BlockEnforcement::LegacyV1);
                        warnings.push(format!("top-level `{v1_key}` is legacy v1 advisory metadata; consider migrating to version 2"));
                    }
                }
                unknown => {
                    blocks.insert(unknown.to_string(), BlockEnforcement::Unknown);
                    let suggestion = suggest_similar(unknown);
                    let msg = if let Some(sug) = suggestion {
                        format!("unknown key `{unknown}`; did you mean `{sug}`?")
                    } else {
                        format!("unknown configuration key `{unknown}`")
                    };
                    if strict {
                        errors.push(msg);
                    } else {
                        warnings.push(msg);
                    }
                }
            }
        }

        Self {
            version,
            blocks,
            warnings,
            errors,
        }
    }
}

fn validate_runtime_block(
    val: &Value,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
    strict: bool,
) {
    if let Some(map) = val.as_mapping() {
        for (k, v) in map {
            if let Some(k_str) = k.as_str() {
                match k_str {
                    "token_caps" => {
                        validate_token_caps_block(v, "runtime.token_caps", warnings, errors);
                    }
                    unknown => {
                        let msg = format!("unknown key `runtime.{unknown}`");
                        if strict {
                            errors.push(msg);
                        } else {
                            warnings.push(msg);
                        }
                    }
                }
            }
        }
    } else {
        errors.push("`runtime` must be a mapping".to_string());
    }
}

fn validate_features_block(
    val: &Value,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
    strict: bool,
) {
    if let Some(map) = val.as_mapping() {
        for (k, v) in map {
            if let Some(k_str) = k.as_str() {
                match k_str {
                    "ontology" | "jev" => {
                        if !v.is_bool() {
                            errors.push(format!(
                                "`features.{k_str}` must be a boolean (true or false)"
                            ));
                        }
                    }
                    unknown => {
                        let sug = if unknown == "ontolgy" {
                            Some("ontology")
                        } else {
                            None
                        };
                        let msg = if let Some(s) = sug {
                            format!(
                                "unknown key `features.{unknown}`; did you mean `features.{s}`?"
                            )
                        } else {
                            format!("unknown key `features.{unknown}`")
                        };
                        if strict {
                            errors.push(msg);
                        } else {
                            warnings.push(msg);
                        }
                    }
                }
            }
        }
    } else {
        errors.push("`features` must be a mapping".to_string());
    }
}

fn validate_token_caps_block(
    val: &Value,
    prefix: &str,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(map) = val.as_mapping() {
        for (k, v) in map {
            if let Some(k_str) = k.as_str() {
                match k_str {
                    "skill_lean_tokens" | "tool_payload_tokens" => {
                        if let Some(n) = v.as_u64() {
                            if n == 0 {
                                errors
                                    .push(format!("`{prefix}.{k_str}` must be greater than zero"));
                            }
                        } else {
                            errors.push(format!("`{prefix}.{k_str}` must be a positive integer"));
                        }
                    }
                    unknown => {
                        let sug = if unknown.contains("lean") {
                            Some("skill_lean_tokens")
                        } else if unknown.contains("payload") {
                            Some("tool_payload_tokens")
                        } else {
                            None
                        };
                        let msg = if let Some(s) = sug {
                            format!(
                                "unknown key `{prefix}.{unknown}`; did you mean `{prefix}.{s}`?"
                            )
                        } else {
                            format!("unknown key `{prefix}.{unknown}`")
                        };
                        warnings.push(msg);
                    }
                }
            }
        }
    } else {
        errors.push(format!("`{prefix}` must be a mapping"));
    }
}

fn validate_advisory_block(
    val: &Value,
    _warnings: &mut [String],
    errors: &mut Vec<String>,
    _strict: bool,
) {
    if !val.is_mapping() {
        errors.push("`advisory` must be a mapping".to_string());
    }
}

fn suggest_similar(name: &str) -> Option<&'static str> {
    let candidates = [
        "version",
        "runtime",
        "features",
        "protected_paths",
        "jev",
        "advisory",
        "token_caps",
    ];
    candidates
        .into_iter()
        .find(|&c| levenshtein_distance(name, c) <= 2)
}

fn levenshtein_distance(s: &str, t: &str) -> usize {
    let s_chars: Vec<char> = s.chars().collect();
    let t_chars: Vec<char> = t.chars().collect();
    let mut d = vec![vec![0; t_chars.len() + 1]; s_chars.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=s_chars.len() {
        for j in 1..=t_chars.len() {
            let cost = if s_chars[i - 1] == t_chars[j - 1] {
                0
            } else {
                1
            };
            d[i][j] = std::cmp::min(
                std::cmp::min(d[i - 1][j] + 1, d[i][j - 1] + 1),
                d[i - 1][j - 1] + cost,
            );
        }
    }
    d[s_chars.len()][t_chars.len()]
}
