//! Holdout fixture validation uses production input builders without invoking a client.

use std::path::{Path, PathBuf};

use super::{load_fixtures, load_policy, validate_coverage, verify_source};

#[test]
fn frozen_holdout_has_two_sided_labels_and_transmitted_context() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let path = root.join("benchmarks/jev/holdout/fixtures-v1.yml");
    let (fixtures, _) = load_fixtures(&path).expect("holdout fixture schema");
    verify_source(&path, &fixtures.source).expect("bound source manifest");
    let (policy, _) = load_policy(&root.join("benchmarks/jev/policy-v1.yml")).unwrap();
    validate_coverage(&fixtures, &policy, &path).expect("both aspects and every signal covered");

    let source: serde_json::Value = crate::engine::yaml::from_str(
        &std::fs::read_to_string(path.with_file_name("source-v1.yml")).unwrap(),
    )
    .unwrap();
    let rationale_bytes = std::fs::read(path.with_file_name("rationales-v1.yml")).unwrap();
    assert_eq!(
        source["rationales_sha256"].as_str().unwrap(),
        crate::engine::digest::sha256(&rationale_bytes)
    );
    let config = super::super::config::JevConfig::default();
    assert_eq!(
        source["scoring_candidate"]["secret_boundary"],
        config.secret_boundary
    );
    assert_eq!(
        source["scoring_candidate"]["complexity_boundary"],
        config.complexity_boundary
    );
    assert_eq!(
        source["scoring_candidate"]["drift_boundary"],
        config.drift_boundary
    );
    assert_eq!(
        source["scoring_candidate"]["rigor_failure_boundary"],
        config.rigor_failure_boundary
    );
    let rationales: serde_json::Value = crate::engine::yaml::from_slice(&rationale_bytes).unwrap();
    let rationale_cases = rationales["cases"].as_array().unwrap();
    assert_eq!(rationale_cases.len(), fixtures.cases.len());

    let mut positive = [0; 5];
    let mut negative = [0; 5];
    let mut rigor_count = 0;
    let mut drift_count = 0;
    for (case, rationale) in fixtures.cases.iter().zip(rationale_cases) {
        assert_eq!(rationale["case_id"], case.case_id);
        if let Some(labels) = &case.expected.rigor {
            rigor_count += 1;
            let state = super::run::rigor_state(case).expect("production-filtered rigor input");
            let entries = state.as_array().unwrap();
            assert!(entries.iter().any(|entry| entry["path"] == "Cargo.toml"));
            assert!(
                entries
                    .iter()
                    .any(|entry| entry["path"] == "project-rules.txt")
            );
            let source = entries.last().unwrap()["content"].as_str().unwrap();
            assert_eq!(source.contains("svcx_"), labels.contains_secrets);
            assert!(!source.contains("// secret:"));
            for label in [
                "hallucinated_import",
                "violates_conventions",
                "complexity_high",
                "contains_secrets",
            ] {
                assert!(rationale["labels"][label].as_str().is_some());
            }
            for (index, value) in [
                labels.hallucinated_import,
                labels.violates_conventions,
                labels.complexity_high,
                labels.contains_secrets,
            ]
            .into_iter()
            .enumerate()
            {
                if value {
                    positive[index] += 1;
                } else {
                    negative[index] += 1;
                }
            }
        } else {
            drift_count += 1;
            let scope = case.drift.as_ref().unwrap();
            let paths: Vec<PathBuf> = scope.changed_paths.iter().map(PathBuf::from).collect();
            super::super::secret_filter::build_drift_scope_state(
                &paths,
                &scope.protected_paths,
                Some(&scope.task),
            )
            .expect("production drift input");
            assert!(!scope.changed_paths.iter().any(|path| {
                super::super::drift::path_is_protected(path, &scope.protected_paths)
            }));
            assert!(rationale["labels"]["drift_out_of_scope"].as_str().is_some());
            if case.expected.drift_out_of_scope.unwrap() {
                positive[4] += 1;
            } else {
                negative[4] += 1;
            }
        }
    }
    assert_eq!((rigor_count, drift_count), (50, 50));
    assert!(positive.iter().all(|count| *count >= 20));
    assert!(negative.iter().all(|count| *count >= 20));
}
