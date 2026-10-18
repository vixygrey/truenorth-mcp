//! Versioned scenario validation and disposable repository preparation.

use super::*;

#[cfg(test)]
#[path = "bench_guard_qualification_fixtures_tests.rs"]
mod tests;
pub(super) fn invalid(path: &Path, detail: &str) -> JevError {
    JevError::InvalidFixtureSet {
        path: path.to_string_lossy().into_owned(),
        detail: detail.into(),
    }
}

/// Reject malformed or under-covered proposals before starting any billable call.
pub fn load_guard_fixtures(
    path: &Path,
    policy: &AcceptancePolicy,
) -> Result<(GuardFixtureSet, String), JevError> {
    let bytes =
        std::fs::read(path).map_err(|_| invalid(path, "cannot read qualification fixture"))?;
    let digest = crate::engine::digest::sha256(&bytes);
    let set: GuardFixtureSet = crate::engine::yaml::from_slice(&bytes)
        .map_err(|_| invalid(path, "invalid qualification fixture schema"))?;
    if set.schema_version != 1 || set.source.version != "1" || set.cases.len() != 100 {
        return Err(invalid(
            path,
            "qualification requires exactly 100 version-1 proposals",
        ));
    }
    super::verify_source(path, &set.source)?;
    let source_path = path.with_file_name("source-v1.yml");
    let source: serde_json::Value = crate::engine::yaml::from_slice(
        &std::fs::read(&source_path).map_err(|_| invalid(path, "unreadable source manifest"))?,
    )
    .map_err(|_| invalid(path, "invalid source manifest"))?;
    let rationale_path = path.with_file_name("rationales-v1.yml");
    if !std::fs::symlink_metadata(&rationale_path)
        .is_ok_and(|meta| meta.is_file() && !meta.file_type().is_symlink())
    {
        return Err(invalid(path, "reviewer rationale must be a regular file"));
    }
    let rationale_bytes = std::fs::read(&rationale_path)
        .map_err(|_| invalid(path, "unreadable reviewer rationale"))?;
    if source["rationales_sha256"].as_str()
        != Some(crate::engine::digest::sha256(&rationale_bytes).as_str())
    {
        return Err(invalid(
            path,
            "reviewer rationale provenance digest mismatch",
        ));
    }
    let rationales: serde_json::Value = crate::engine::yaml::from_slice(&rationale_bytes)
        .map_err(|_| invalid(path, "invalid reviewer rationale schema"))?;
    let rationale_cases = rationales["cases"]
        .as_array()
        .ok_or_else(|| invalid(path, "reviewer rationale cases are missing"))?;
    if rationale_cases.len() != set.cases.len() + set.deterministic_cases.len()
        || !set
            .cases
            .iter()
            .chain(&set.deterministic_cases)
            .zip(rationale_cases)
            .all(|(case, rationale)| rationale["case_id"].as_str() == Some(case.case_id.as_str()))
    {
        return Err(invalid(
            path,
            "reviewer rationales do not match the fixture cases",
        ));
    }
    if set.deterministic_cases.len() < 4
        || !set
            .deterministic_cases
            .iter()
            .any(|case| case.expected.deterministic == "secret")
        || !set
            .deterministic_cases
            .iter()
            .any(|case| case.expected.deterministic == "protected-path")
    {
        return Err(invalid(
            path,
            "missing deterministic privacy and path probes",
        ));
    }
    let mut ids = HashSet::new();
    let mut focus_counts = [0usize; 2];
    let mut labels: BTreeMap<&str, (usize, usize)> = [
        "hallucinated_import",
        "violates_conventions",
        "complexity_high",
        "contains_secrets",
        "out_of_scope",
    ]
    .into_iter()
    .map(|name| (name, (0, 0)))
    .collect();
    for (index, case) in set.cases.iter().chain(&set.deterministic_cases).enumerate() {
        if case.case_id.trim().is_empty()
            || !ids.insert(&case.case_id)
            || case.proposal.paths.is_empty()
        {
            return Err(invalid(
                path,
                "case IDs must be unique and proposed paths nonempty",
            ));
        }
        let focus = match case.focus.as_str() {
            "rigor" => 0,
            "scope" => 1,
            _ => return Err(invalid(path, "unsupported qualification focus")),
        };
        if (index < 100 && case.expected.deterministic != "allow")
            || (index >= 100 && case.expected.deterministic == "allow")
        {
            return Err(invalid(
                path,
                "only model-sampled primary cases may allow provider calls",
            ));
        }
        if index < 100 {
            focus_counts[focus] += 1;
        }
        if case.protected_paths.is_empty()
            || !["allow", "protected-path", "secret"]
                .contains(&case.expected.deterministic.as_str())
            || !["allow", "annotate", "block"].contains(&case.expected.decision.as_str())
            || (index < 100 && case.expected.decision == "block")
            || case.expected.mcp != case.expected.decision
            || case.expected.cli != case.expected.decision
        {
            return Err(invalid(
                path,
                "unsupported deterministic or interface decision label",
            ));
        }
        if case
            .proposal
            .task
            .as_ref()
            .is_some_and(|task| task.trim().is_empty())
            || case
                .cockpit_task
                .as_ref()
                .is_some_and(|task| task.trim().is_empty())
        {
            return Err(invalid(path, "blank task is not an available task"));
        }
        for file in &case.files {
            safe_path(path, &file.path)?;
            if crate::config::is_secret_path(Path::new(&file.path)) {
                return Err(invalid(path, "sensitive fixture file path"));
            }
        }
        let mut file_paths = HashSet::new();
        if case.files.iter().any(|file| !file_paths.insert(&file.path)) {
            return Err(invalid(path, "duplicate initial file"));
        }
        for name in case
            .proposal
            .paths
            .iter()
            .chain(case.protected_paths.iter())
        {
            safe_path(path, name.trim_end_matches('/'))?;
        }
        if case
            .proposal
            .paths
            .iter()
            .any(|name| crate::config::is_secret_path(Path::new(name)))
        {
            return Err(invalid(
                path,
                "sensitive proposed path is not a qualification input",
            ));
        }
        let actual = deterministic_layer(&case.proposal, &case.protected_paths);
        let marker = match &actual {
            None => "allow",
            Some(GuardDecision::Block(packet)) => packet.violated_check.as_str(),
            Some(_) => return Err(invalid(path, "unexpected deterministic decision")),
        };
        if marker != case.expected.deterministic
            || (marker != "allow" && case.expected.decision != "block")
        {
            return Err(invalid(
                path,
                "deterministic expectation contradicts actual policy",
            ));
        }
        if marker == "allow" {
            let rigor = case
                .expected
                .rigor
                .as_ref()
                .ok_or_else(|| invalid(path, "missing rigor labels"))?;
            let scope = case
                .expected
                .drift_out_of_scope
                .ok_or_else(|| invalid(path, "missing scope label"))?;
            for (name, positive) in [
                ("hallucinated_import", rigor.hallucinated_import),
                ("violates_conventions", rigor.violates_conventions),
                ("complexity_high", rigor.complexity_high),
                ("contains_secrets", rigor.contains_secrets),
                ("out_of_scope", scope),
            ] {
                let counts = labels.entry(name).or_default();
                if positive {
                    counts.0 += 1;
                } else {
                    counts.1 += 1;
                }
            }
        } else if case.expected.rigor.is_some() || case.expected.drift_out_of_scope.is_some() {
            return Err(invalid(
                path,
                "deterministic rejection must not claim model labels",
            ));
        }
    }
    let limits = policy
        .limits
        .as_ref()
        .ok_or_else(|| invalid(path, "qualification limits must be explicitly reviewed"))?;
    if limits.minimum_samples_per_aspect < 50
        || limits.minimum_positive_per_signal < 20
        || limits.minimum_negative_per_signal < 20
        || limits.minimum_agreement_per_signal < 0.98
        || limits.maximum_false_positives_per_signal > 1
        || limits.maximum_false_negatives_per_signal != 0
        || limits.maximum_error_rate != 0.0
        || limits.maximum_p95_latency_ms > 10_000
        || limits.maximum_cost_per_case > 0.002
        || limits.maximum_total_cost > 0.25
    {
        return Err(invalid(
            path,
            "whole-guard acceptance limits cannot be weakened",
        ));
    }
    if focus_counts != [50, 50]
        || labels.values().any(|(pos, neg)| {
            pos + neg < limits.minimum_samples_per_aspect
                || *pos < limits.minimum_positive_per_signal
                || *neg < limits.minimum_negative_per_signal
        })
    {
        return Err(invalid(
            path,
            "insufficient predeclared primary-signal coverage",
        ));
    }
    Ok((set, digest))
}

fn safe_path(fixture: &Path, name: &str) -> Result<(), JevError> {
    if crate::engine::agent_ws::fs_boundary::RepositoryFs::validate_path(Path::new(name)).is_err() {
        Err(invalid(fixture, "unsafe qualification path"))
    } else {
        Ok(())
    }
}
pub(super) fn prepare_case(case: &GuardCase) -> Result<tempfile::TempDir, JevError> {
    let root = tempfile::tempdir().map_err(|_| {
        invalid(
            Path::new("guard-qualification"),
            "cannot create isolated project",
        )
    })?;
    for file in &case.files {
        let target = root.path().join(&file.path);
        std::fs::create_dir_all(
            target
                .parent()
                .ok_or_else(|| invalid(root.path(), "invalid context parent"))?,
        )
        .and_then(|_| std::fs::write(target, &file.content))
        .map_err(|_| invalid(root.path(), "cannot prepare isolated context"))?;
    }
    if let Some(task) = &case.cockpit_task {
        let state = root.path().join(".agent/tasks/state.json");
        std::fs::create_dir_all(
            state
                .parent()
                .ok_or_else(|| invalid(root.path(), "invalid task context parent"))?,
        )
        .and_then(|_| std::fs::write(state, serde_json::json!({"active_task": task}).to_string()))
        .map_err(|_| invalid(root.path(), "cannot prepare task context"))?;
    }
    Ok(root)
}
pub(super) fn source_unchanged(case: &GuardCase, root: &Path) -> bool {
    case.files.iter().all(|file| {
        std::fs::read(root.join(&file.path)).is_ok_and(|actual| actual == file.content.as_bytes())
    }) && case
        .proposal
        .paths
        .iter()
        .filter(|path| {
            !case
                .files
                .iter()
                .any(|file| file.path.as_str() == path.as_str())
        })
        .all(|path| {
            std::fs::symlink_metadata(root.join(path))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        })
}
