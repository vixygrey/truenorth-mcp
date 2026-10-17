//! Read-only analysis for the explicit cockpit JSON migration command.

use std::fs;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use crate::engine::digest::sha256;

const TASKS: &str = ".agent/tasks";

#[derive(Debug, Clone, Serialize)]
pub struct MigrationPlan {
    pub entries: Vec<MigrationEntry>,
    pub clean_worktree_required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationEntry {
    pub name: &'static str,
    pub source: Option<String>,
    pub target: String,
    pub source_sha256: Option<String>,
    pub comment_loss_warning: bool,
    pub collision: bool,
    pub unsupported_constructs: Vec<String>,
}

#[derive(Debug, Error)]
pub enum MigrationPlanError {
    #[error("could not read cockpit source `{path}`: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error(
        "JSON migration target `{path}` already exists; resolve the collision before migrating"
    )]
    Collision { path: String },
}

/// Analyze the three supported cockpit documents without writing any workspace path.
pub fn analyze(repo_root: &Path) -> Result<MigrationPlan, MigrationPlanError> {
    let entries = [
        analyze_document(repo_root, "state", Some("specs/state.yaml"), false)?,
        analyze_document(
            repo_root,
            "release-plan",
            Some("specs/release-plan.yaml"),
            false,
        )?,
        analyze_document(repo_root, "bugs", None, true)?,
    ];
    Ok(MigrationPlan {
        entries: entries.into_iter().collect(),
        clean_worktree_required: true,
    })
}

fn analyze_document(
    repo_root: &Path,
    name: &'static str,
    legacy: Option<&str>,
    default_when_absent: bool,
) -> Result<MigrationEntry, MigrationPlanError> {
    let source_path = repo_root.join(TASKS).join(format!("{name}.yml"));
    let legacy_path = legacy.map(|path| repo_root.join(path));
    let selected = if source_path.is_file() {
        Some(source_path)
    } else {
        legacy_path.filter(|path| path.is_file())
    };
    let target = format!("{TASKS}/{name}.json");
    let target_path = repo_root.join(&target);
    let collision = target_path.exists();
    if collision {
        return Err(MigrationPlanError::Collision { path: target });
    }
    let (source, source_sha256, comment_loss_warning, unsupported_constructs) = match selected {
        Some(path) => {
            let bytes = fs::read(&path).map_err(|source| MigrationPlanError::Read {
                path: path.display().to_string(),
                source,
            })?;
            let text = String::from_utf8_lossy(&bytes);
            (
                Some(relative(repo_root, &path)),
                Some(sha256(&bytes)),
                text.contains('#'),
                unsupported_constructs(&text),
            )
        }
        None if default_when_absent => (None, None, false, Vec::new()),
        None => (None, None, false, Vec::new()),
    };
    Ok(MigrationEntry {
        name,
        source,
        target,
        source_sha256,
        comment_loss_warning,
        collision,
        unsupported_constructs,
    })
}

fn relative(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn unsupported_constructs(input: &str) -> Vec<String> {
    let Ok(events) = saneyaml::parse_events(input) else {
        return vec!["malformed YAML".to_string()];
    };
    let documents = events
        .iter()
        .filter(|event| matches!(event, saneyaml::Event::DocumentStart { .. }))
        .count();
    let mut unsupported = Vec::new();
    if documents > 1 {
        unsupported.push("multi-document stream".to_string());
    }
    if events
        .iter()
        .any(|event| matches!(event, saneyaml::Event::Alias { .. }))
    {
        unsupported.push("alias".to_string());
    }
    if events.iter().any(|event| match event {
        saneyaml::Event::Scalar { meta, .. }
        | saneyaml::Event::MappingStart { meta, .. }
        | saneyaml::Event::SequenceStart { meta, .. } => {
            meta.anchor.is_some() || meta.tag.is_some()
        }
        _ => false,
    }) {
        unsupported.push("tag or anchor".to_string());
    }
    match crate::engine::yaml::from_str::<saneyaml::Value>(input) {
        Ok(value) => {
            if !value.is_mapping() {
                unsupported.push("non-object root".to_string());
            }
            inspect_value(&value, &mut unsupported);
        }
        Err(_) => unsupported.push("malformed YAML".to_string()),
    }
    unsupported
}

fn inspect_value(value: &saneyaml::Value, unsupported: &mut Vec<String>) {
    match value {
        saneyaml::Value::Mapping(mapping) => {
            for (key, value) in mapping {
                if !key.is_string()
                    && !unsupported
                        .iter()
                        .any(|item| item == "non-string mapping key")
                {
                    unsupported.push("non-string mapping key".to_string());
                }
                inspect_value(value, unsupported);
            }
        }
        saneyaml::Value::Sequence(values) => {
            for value in values {
                inspect_value(value, unsupported);
            }
        }
        saneyaml::Value::Tagged(_) if !unsupported.iter().any(|item| item == "tagged value") => {
            unsupported.push("tagged value".to_string());
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{MigrationPlanError, analyze};

    #[test]
    fn prefers_agent_sources_warns_about_comments_and_never_writes() {
        let repo = tempdir().expect("repo");
        let tasks = repo.path().join(".agent/tasks");
        fs::create_dir_all(&tasks).expect("tasks");
        fs::write(tasks.join("state.yml"), "# keep\nphase: design\n").expect("state");
        fs::create_dir_all(repo.path().join("specs")).expect("specs");
        fs::write(repo.path().join("specs/state.yaml"), "phase: legacy\n").expect("legacy");

        let plan = analyze(repo.path()).expect("plan");
        let state = &plan.entries[0];
        assert_eq!(state.source.as_deref(), Some(".agent/tasks/state.yml"));
        assert!(state.comment_loss_warning);
        assert!(!tasks.join("state.json").exists());
        assert_eq!(plan.entries[2].source, None);
    }

    #[test]
    fn rejects_target_collisions_without_writing() {
        let repo = tempdir().expect("repo");
        let tasks = repo.path().join(".agent/tasks");
        fs::create_dir_all(&tasks).expect("tasks");
        fs::write(tasks.join("state.json"), "{}\n").expect("target");

        assert!(matches!(
            analyze(repo.path()),
            Err(MigrationPlanError::Collision { .. })
        ));
    }
}
