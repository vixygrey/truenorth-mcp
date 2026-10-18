//! Public qualification-input preflight regressions; none of these paths calls a provider.

use super::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Inputs {
    _directory: TempDir,
    fixtures_path: PathBuf,
    policy_path: PathBuf,
    original: Value,
}

impl Inputs {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary qualification inputs");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let original_path = repository.join("benchmarks/jev/guard-qualification");
        for name in [
            "fixtures-v1.yml",
            "source-v1.yml",
            "rationales-v1.yml",
            "policy-v1.yml",
        ] {
            std::fs::copy(original_path.join(name), directory.path().join(name))
                .expect("copy versioned input into disposable directory");
        }
        let fixtures_path = directory.path().join("fixtures-v1.yml");
        let policy_path = directory.path().join("policy-v1.yml");
        let original = yaml_value(&fixtures_path);
        Self {
            _directory: directory,
            fixtures_path,
            policy_path,
            original,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.fixtures_path.with_file_name(name)
    }

    fn policy(&self) -> AcceptancePolicy {
        super::super::super::load_policy(&self.policy_path)
            .expect("parse reviewed candidate policy")
            .0
    }

    fn reject(&self, fixture: &Value) {
        self.reject_with_policy(fixture, &self.policy());
    }

    fn reject_with_policy(&self, fixture: &Value, policy: &AcceptancePolicy) {
        write_value(&self.fixtures_path, fixture);
        match load_guard_fixtures(&self.fixtures_path, policy) {
            Err(JevError::InvalidFixtureSet { .. }) => {}
            Err(other) => panic!("expected fixture preflight rejection, got {other}"),
            Ok(_) => panic!("unsafe or under-covered qualification input was accepted"),
        }
    }

    fn rebind_rationales(&self, fixture: &mut Value) {
        let mut source = yaml_value(&self.path("source-v1.yml"));
        source["rationales_sha256"] = json!(crate::engine::digest::sha256(
            &std::fs::read(self.path("rationales-v1.yml")).unwrap()
        ));
        let source_bytes = source.to_string();
        std::fs::write(self.path("source-v1.yml"), &source_bytes).unwrap();
        fixture["source"]["sha256"] = json!(crate::engine::digest::sha256(source_bytes.as_bytes()));
    }
}

fn yaml_value(path: &Path) -> Value {
    crate::engine::yaml::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn write_value(path: &Path, value: &Value) {
    // JSON is a YAML subset; preserve the fixture structure while mutating its actual wire value.
    std::fs::write(path, value.to_string()).unwrap();
}

#[test]
fn reviewer_rationales_must_be_integrity_bound_and_match_every_case() {
    let inputs = Inputs::new();
    let rationales_path = inputs.path("rationales-v1.yml");
    let mut rationales = yaml_value(&rationales_path);
    rationales["cases"][0]["case_id"] = json!("substituted-review");
    write_value(&rationales_path, &rationales);
    inputs.reject(&inputs.original);

    let mut fixture = inputs.original.clone();
    inputs.rebind_rationales(&mut fixture);
    inputs.reject(&fixture);

    rationales["cases"] = json!([]);
    write_value(&rationales_path, &rationales);
    inputs.rebind_rationales(&mut fixture);
    inputs.reject(&fixture);

    rationales.as_object_mut().unwrap().remove("cases");
    write_value(&rationales_path, &rationales);
    inputs.rebind_rationales(&mut fixture);
    inputs.reject(&fixture);
}

#[test]
fn source_and_review_files_cannot_be_replaced_without_provenance() {
    let inputs = Inputs::new();
    let mut fixture = inputs.original.clone();
    fixture["source"]["sha256"] = json!("0".repeat(64));
    inputs.reject(&fixture);

    std::fs::remove_file(inputs.path("source-v1.yml")).unwrap();
    inputs.reject(&inputs.original);

    let inputs = Inputs::new();
    std::fs::remove_file(inputs.path("rationales-v1.yml")).unwrap();
    inputs.reject(&inputs.original);
}

#[cfg(unix)]
#[test]
fn rationale_symlink_is_not_a_trusted_review_file() {
    let inputs = Inputs::new();
    let rationale = inputs.path("rationales-v1.yml");
    let separate = inputs.path("review-copy.yml");
    std::fs::copy(&rationale, &separate).unwrap();
    std::fs::remove_file(&rationale).unwrap();
    std::os::unix::fs::symlink(separate, rationale).unwrap();
    inputs.reject(&inputs.original);
}

#[test]
fn preflight_rejects_missing_sampled_or_deterministic_coverage() {
    let inputs = Inputs::new();
    let mut fixture = inputs.original.clone();
    fixture["cases"].as_array_mut().unwrap().pop();
    inputs.reject(&fixture);

    let mut fixture = inputs.original.clone();
    fixture["deterministic_cases"]
        .as_array_mut()
        .unwrap()
        .truncate(3);
    let mut rationales = yaml_value(&inputs.path("rationales-v1.yml"));
    rationales["cases"].as_array_mut().unwrap().truncate(103);
    write_value(&inputs.path("rationales-v1.yml"), &rationales);
    inputs.rebind_rationales(&mut fixture);
    inputs.reject(&fixture);
}

#[test]
fn duplicate_identity_and_initial_files_cannot_ambiguate_reviewed_context() {
    let inputs = Inputs::new();
    let mut fixture = inputs.original.clone();
    let file = fixture["cases"][0]["files"][0].clone();
    fixture["cases"][0]["files"]
        .as_array_mut()
        .unwrap()
        .push(file);
    inputs.reject(&fixture);

    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["proposal"]["paths"] = json!([]);
    inputs.reject(&fixture);

    let mut fixture = inputs.original.clone();
    fixture["cases"][1]["case_id"] = fixture["cases"][0]["case_id"].clone();
    let mut rationales = yaml_value(&inputs.path("rationales-v1.yml"));
    rationales["cases"][1]["case_id"] = fixture["cases"][0]["case_id"].clone();
    write_value(&inputs.path("rationales-v1.yml"), &rationales);
    inputs.rebind_rationales(&mut fixture);
    inputs.reject(&fixture);
}

#[test]
fn context_and_proposal_paths_cannot_escape_or_include_sensitive_files() {
    let inputs = Inputs::new();
    for (field, value) in [
        ("file", "../other-worktree.rs"),
        ("file", ".env"),
        ("proposal", "../other-worktree.rs"),
        ("proposal", ".env"),
        ("protected", "../other-worktree.rs"),
    ] {
        let mut fixture = inputs.original.clone();
        let case = &mut fixture["cases"][0];
        match field {
            "file" => case["files"][0]["path"] = json!(value),
            "proposal" => case["proposal"]["paths"][0] = json!(value),
            "protected" => case["protected_paths"][0] = json!(value),
            _ => unreachable!(),
        }
        inputs.reject(&fixture);
    }
}

#[test]
fn deterministic_outcomes_and_skipped_model_labels_must_match_policy() {
    let inputs = Inputs::new();
    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["expected"]["deterministic"] = json!("secret");
    inputs.reject(&fixture);

    let mut fixture = inputs.original.clone();
    fixture["deterministic_cases"][0]["expected"]["deterministic"] = json!("secret");
    inputs.reject(&fixture);

    for field in ["rigor", "drift_out_of_scope"] {
        let mut fixture = inputs.original.clone();
        fixture["deterministic_cases"][0]["expected"][field] =
            inputs.original["cases"][0]["expected"][field].clone();
        inputs.reject(&fixture);
    }

    let mut fixture = inputs.original.clone();
    fixture["deterministic_cases"][0]["expected"]["decision"] = json!("annotate");
    fixture["deterministic_cases"][0]["expected"]["mcp"] = json!("annotate");
    fixture["deterministic_cases"][0]["expected"]["cli"] = json!("annotate");
    inputs.reject(&fixture);

    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["expected"]["mcp"] = json!("block");
    inputs.reject(&fixture);
}

#[test]
fn unavailable_model_labels_and_unbalanced_signals_cannot_qualify() {
    let inputs = Inputs::new();
    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["expected"]["rigor"] = Value::Null;
    inputs.reject(&fixture);

    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["expected"]["drift_out_of_scope"] = Value::Null;
    inputs.reject(&fixture);

    for all_positive in [false, true] {
        let mut fixture = inputs.original.clone();
        for case in fixture["cases"].as_array_mut().unwrap() {
            case["expected"]["rigor"]["hallucinated_import"] = json!(all_positive);
        }
        inputs.reject(&fixture);
    }
}

#[test]
fn unsupported_focus_or_absent_task_cannot_hide_unavailable_scope_evidence() {
    let inputs = Inputs::new();
    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["focus"] = json!("unreviewed");
    inputs.reject(&fixture);

    for field in ["task", "cockpit_task"] {
        let mut fixture = inputs.original.clone();
        let case = &mut fixture["cases"][0];
        if field == "task" {
            case["proposal"][field] = json!(" \t ");
        } else {
            case[field] = json!(" \t ");
        }
        inputs.reject(&fixture);
    }

    let mut fixture = inputs.original.clone();
    fixture["cases"][0]["focus"] = json!("scope");
    inputs.reject(&fixture);
}

#[test]
fn numerical_qualification_limits_must_be_present_and_not_weakened() {
    let inputs = Inputs::new();
    let original_policy = yaml_value(&inputs.policy_path);
    let mut policy = original_policy.clone();
    policy["limits"] = Value::Null;
    write_value(&inputs.policy_path, &policy);
    inputs.reject(&inputs.original);

    for (name, weak) in [
        ("minimum_samples_per_aspect", json!(49)),
        ("minimum_positive_per_signal", json!(19)),
        ("minimum_negative_per_signal", json!(19)),
        ("minimum_agreement_per_signal", json!(0.97)),
        ("maximum_false_positives_per_signal", json!(2)),
        ("maximum_false_negatives_per_signal", json!(1)),
        ("maximum_p95_latency_ms", json!(10_001)),
        ("maximum_error_rate", json!(0.01)),
        ("maximum_cost_per_case", json!(0.003)),
        ("maximum_total_cost", json!(0.26)),
    ] {
        let mut policy = original_policy.clone();
        policy["limits"][name] = weak;
        write_value(&inputs.policy_path, &policy);
        inputs.reject(&inputs.original);
    }
}
