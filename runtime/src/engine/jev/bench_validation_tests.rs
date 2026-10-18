use super::*;
use serde_json::{Value, json};
use tempfile::tempdir;

fn input(name: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crate::engine::yaml::from_str(
        &std::fs::read_to_string(root.join("benchmarks/jev").join(name)).unwrap(),
    )
    .unwrap()
}

#[test]
fn incomplete_or_deterministically_forced_fixtures_are_rejected() {
    let original = input("fixtures-v1.yml");
    let dir = tempdir().unwrap();
    let path = dir.path().join("fixtures.yml");
    std::fs::write(&path, original.to_string()).unwrap();
    load_fixtures(&path).expect("valid baseline before applying invalid mutations");
    let drift_index = original["cases"]
        .as_array()
        .unwrap()
        .iter()
        .position(|case| case["drift"].is_object())
        .unwrap();
    let mut malformed = Vec::new();
    let mut case = original.clone();
    case["cases"][0]["expected"] = json!({});
    malformed.push(case);
    let mut case = original.clone();
    case["cases"][0]["files"] = json!([]);
    malformed.push(case);
    let mut case = original.clone();
    case["cases"][drift_index]["drift"] = Value::Null;
    malformed.push(case);
    let mut case = original.clone();
    case["cases"][0]["state"] = json!({"task": "unexpected state"});
    malformed.push(case);
    for state in [Value::Null, json!(42)] {
        let mut case = original.clone();
        case["cases"][0]["expected"]["routing"] = json!("NONE");
        case["cases"][0]["state"] = state;
        malformed.push(case);
    }
    for (label, value) in [("routing", "UNSUPPORTED"), ("self_heal", "UNSUPPORTED")] {
        let mut case = original.clone();
        case["cases"][0]["expected"][label] = json!(value);
        case["cases"][0]["state"] = json!({"task": "candidate"});
        malformed.push(case);
    }
    for (field, value) in [
        ("task", json!("")),
        ("changed_paths", json!([])),
        ("protected_paths", json!([])),
        ("changed_paths", json!(["../outside.rs"])),
    ] {
        let mut case = original.clone();
        case["cases"][drift_index]["drift"][field] = value;
        malformed.push(case);
    }
    let mut case = original;
    case["cases"][drift_index]["drift"]["protected_paths"] = json!(["src/"]);
    case["cases"][drift_index]["drift"]["changed_paths"] = json!(["src/protected.rs"]);
    malformed.push(case);
    for case in malformed {
        std::fs::write(&path, case.to_string()).unwrap();
        assert!(
            load_fixtures(&path).is_err(),
            "invalid case was accepted: {case}"
        );
    }
}

#[test]
fn approved_policy_requires_finite_in_range_limits() {
    let mut policy = input("policy-v1.yml");
    policy["approval"] = json!("approved");
    policy["limits"] = json!({
        "minimum_samples_per_aspect": 2,
        "minimum_positive_per_signal": 1,
        "minimum_negative_per_signal": 1,
        "minimum_agreement_per_signal": 0.95,
        "maximum_false_positives_per_signal": 0,
        "maximum_false_negatives_per_signal": 0,
        "maximum_p95_latency_ms": 1000,
        "maximum_error_rate": 0.01,
        "maximum_cost_per_case": 0.01,
        "maximum_total_cost": 0.1
    });
    let dir = tempdir().unwrap();
    let path = dir.path().join("policy.yml");
    std::fs::write(&path, policy.to_string()).unwrap();
    let loaded = load_policy(&path).unwrap().0;
    assert_eq!(loaded.approval, "approved");
    assert_eq!(loaded.limits.unwrap().maximum_total_cost, 0.1);
    for (field, invalid) in [
        ("minimum_samples_per_aspect", json!(0)),
        ("minimum_positive_per_signal", json!(0)),
        ("minimum_negative_per_signal", json!(0)),
        ("minimum_agreement_per_signal", json!(-0.1)),
        ("minimum_agreement_per_signal", json!(1.1)),
        ("maximum_error_rate", json!(-0.1)),
        ("maximum_error_rate", json!(1.1)),
        ("maximum_p95_latency_ms", json!(0)),
        ("maximum_cost_per_case", json!(0)),
        ("maximum_total_cost", json!(-1)),
    ] {
        let mut invalid_policy = policy.clone();
        invalid_policy["limits"][field] = invalid;
        std::fs::write(&path, invalid_policy.to_string()).unwrap();
        assert!(load_policy(&path).is_err(), "accepted invalid {field}");
    }
}

#[test]
fn source_manifest_cannot_be_substituted_with_a_symlink() {
    let dir = tempdir().unwrap();
    let source = input("fixtures-v1.yml")["source"].clone();
    let source: Source = serde_json::from_value(source).unwrap();
    let original = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("benchmarks/jev/source-v1.yml");
    std::os::unix::fs::symlink(original, dir.path().join("source-v1.yml")).unwrap();
    assert!(verify_source(&dir.path().join("fixtures.yml"), &source).is_err());
}
