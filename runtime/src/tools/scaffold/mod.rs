//! Greenfield scaffold tool: `truenorth_scaffold_project`.
//!
//! The tool seeds a brand-new project with the TrueNorth workflow conventions before any
//! code is written (Requirement 5). It emits the `.agent/` tree and the cockpit seed
//! files through the write guard, and the root docs, git hooks, and `.github/` templates
//! through the audited repo-seed path (ADR-6, ADR-7). It produces a language-agnostic
//! scaffold: no `Cargo.toml`, no `package.json`, no source tree (Requirement 5.4).
//!
//! The scaffold is non-destructive: an existing target path is left unchanged and a skip
//! message names it (Requirement 5.12). An unknown profile name makes no file change and
//! returns an error naming the value and the known names (Requirement 5.2).
//!
//! Requirements: 5.1 to 5.12. Design: agent-workspace-profiles §5.

use std::path::Path;

use crate::tools::result;
use rmcp::handler::server::{tool::RequestId, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::agent_ws::{
    ObservedFile, WritePrecondition, write_repo_seed, write_under_agent_if_unchanged,
};
use crate::engine::profile::{self, Profile};
use crate::server::TrueNorthServer;
use crate::tools::hooks::{commit_msg_hook, post_merge_hook};
use crate::tools::mutation_error::{mutation_error, write_error};
use crate::tools::receipt::{self, ChangedContent, OperationReceipt};

mod bootstrap;
mod templates;
mod upgrade;

pub use bootstrap::bootstrap_project;
pub use upgrade::{apply_workspace_upgrade, plan_workspace_upgrade};

use templates::{
    COMMIT_TEMPLATE, EXECUTION_STATUS_SEED, ISSUE_TEMPLATE_CONFIG, JEV_GUARD_HOOK,
    PULL_REQUEST_TEMPLATE, STATE_SEED, agents_md, bug_form, conventions_md, feature_form,
    layout_contract, starter_seed,
};

/// The command the scaffold prints for the human to run. The scaffold never runs it
/// (Requirement 5.7).
const HOOKS_PATH_COMMAND: &str = "git config core.hooksPath .githooks";

/// The maximum length of the profile name argument (Requirement 5.1).
const MAX_PROFILE_NAME: usize = 64;

/// The `truenorth_scaffold_project` input contract (design §5).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScaffoldArgs {
    /// The methodology profile name (1 to 64 chars). Absent uses issue-per-task (R5.1).
    #[serde(default)]
    pub profile: Option<String>,
}

/// One emission outcome: a written path or a skipped path (Requirement 5.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Emission {
    /// The path was written.
    Wrote(String),
    /// The path already existed and was left unchanged.
    Skipped(String),
}

#[tool_router(router = scaffold_router, vis = "pub")]
impl TrueNorthServer {
    /// Seed a new project's `.agent/` tree and root workflow conventions.
    #[tool(
        description = "Scaffold a new project's .agent/ tree, cockpit seeds, and root workflow docs for a methodology profile."
    )]
    pub async fn truenorth_scaffold_project(
        &self,
        params: Parameters<ScaffoldArgs>,
        request_id: RequestId,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Resolve the profile. Absent uses issue-per-task; an unknown name makes no file
        // change and returns an error naming the value and the known names (R5.1, R5.2).
        let profile = resolve_scaffold_profile(args.profile.as_deref())?;

        let permit = self.ctx.mutations.begin().await.map_err(mutation_error)?;
        let plan = plan_scaffold(&self.ctx.repo_root, profile)?;
        let emissions = plan.emissions();
        let changes = plan
            .writes()
            .map(|write| ChangedContent::new(&write.source.path, write.source.body.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(receipt_error)?;
        let receipt =
            OperationReceipt::new("truenorth_scaffold_project", &request_id, changes, None)
                .map_err(receipt_error)?;
        let response = result::success(
            vec![ContentBlock::text(
                receipt::attach(result_json(profile, &emissions), receipt).to_string(),
            )],
            self.ctx.token_caps,
        )?;

        // Commit only after the complete receipt has passed the response token cap.
        plan.commit(&self.ctx.repo_root)?;
        drop(permit);
        Ok(response)
    }
}

/// Resolve the profile for the scaffold (Requirements 5.1, 5.2).
///
/// An absent name uses issue-per-task. A name longer than 64 chars or outside the five
/// built-ins returns an error and makes no file change.
pub(super) fn resolve_scaffold_profile(name: Option<&str>) -> Result<Profile, ErrorData> {
    let Some(name) = name else {
        return Ok(profile::ISSUE_PER_TASK);
    };
    let len = name.chars().count();
    if !(1..=MAX_PROFILE_NAME).contains(&len) {
        return Err(ErrorData::invalid_params(
            format!("`profile` must be 1 to {MAX_PROFILE_NAME} characters when supplied"),
            None,
        ));
    }
    profile::by_name(name).ok_or_else(|| {
        ErrorData::invalid_params(
            format!(
                "unknown profile `{name}`. The known profiles are: epic-based, \
                 issue-per-task, kanban, milestone-based, generic."
            ),
            None,
        )
    })
}

/// One deterministic source file emitted by the scaffold and CLI workspace manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldSource {
    pub path: String,
    pub body: String,
}

/// Build the complete profile-specific generated workspace file set.
pub fn scaffold_sources(profile: Profile) -> Vec<ScaffoldSource> {
    let mut files = vec![
        source(".agent/layout.yml", layout_contract()),
        source(".agent/profile.yml", format!("profile: {}\n", profile.name)),
        source(
            ".agent/config/rules.yml",
            "# Runtime configuration.\n# Token estimates use ceil(characters / 4). Oversized tool responses fail without truncation.\ntoken_caps:\n  skill_lean_tokens: 1500\n  tool_payload_tokens: 4000\n",
        ),
        source(".agent/spec/requirements.md", "# Requirements\n"),
        source(".agent/tasks/state.yml", STATE_SEED),
        source(".agent/tasks/execution-status.yml", EXECUTION_STATUS_SEED),
        source(".agent/memories/lessons.md", "# Lessons\n"),
        source(".agent/memories/glossary.md", "# Glossary\n"),
        source(".agent/product/scope.md", "# Product scope\n"),
    ];
    files.extend(
        profile
            .starter_files
            .iter()
            .map(|starter| source(format!(".agent/{starter}"), starter_seed(starter))),
    );
    files.extend([
        source("AGENTS.md", agents_md(profile)),
        source("CONVENTIONS.md", conventions_md(profile)),
        source(".githooks/commit-msg", commit_msg_hook(profile)),
        source(".githooks/post-merge", post_merge_hook(profile)),
        source(".kiro/hooks/jev-guard.json", JEV_GUARD_HOOK),
        source(".github/commit-template.md", COMMIT_TEMPLATE),
        source(".github/pull-request-template.md", PULL_REQUEST_TEMPLATE),
        source(".github/ISSUE_TEMPLATE/bug.md", bug_form(profile)),
        source(".github/ISSUE_TEMPLATE/feature.md", feature_form(profile)),
        source(".github/ISSUE_TEMPLATE/config.yml", ISSUE_TEMPLATE_CONFIG),
    ]);
    files
}

fn source(path: impl Into<String>, body: impl Into<String>) -> ScaffoldSource {
    ScaffoldSource {
        path: path.into(),
        body: body.into(),
    }
}

#[derive(Debug)]
enum PlannedDestination {
    Agent(WritePrecondition),
    RepoRoot,
    Skip,
}

#[derive(Debug)]
struct PlannedEmission {
    source: ScaffoldSource,
    destination: PlannedDestination,
}

#[derive(Debug)]
struct ScaffoldPlan {
    emissions: Vec<PlannedEmission>,
}

impl ScaffoldPlan {
    fn writes(&self) -> impl Iterator<Item = &PlannedEmission> {
        self.emissions
            .iter()
            .filter(|entry| !matches!(entry.destination, PlannedDestination::Skip))
    }

    fn emissions(&self) -> Vec<Emission> {
        self.emissions
            .iter()
            .map(|entry| match entry.destination {
                PlannedDestination::Skip => Emission::Skipped(entry.source.path.clone()),
                PlannedDestination::Agent(_) | PlannedDestination::RepoRoot => {
                    Emission::Wrote(entry.source.path.clone())
                }
            })
            .collect()
    }

    fn commit(self, repo_root: &Path) -> Result<(), ErrorData> {
        for entry in self.emissions {
            match entry.destination {
                PlannedDestination::Agent(precondition) => {
                    let relative = entry
                        .source
                        .path
                        .strip_prefix(".agent/")
                        .expect("agent scaffold path was classified by its prefix");
                    write_under_agent_if_unchanged(
                        repo_root,
                        Path::new(relative),
                        &entry.source.body,
                        &precondition,
                    )
                    .map_err(write_error)?;
                }
                PlannedDestination::RepoRoot => {
                    write_repo_seed(
                        repo_root,
                        Path::new(&entry.source.path),
                        &entry.source.body,
                        true,
                    )
                    .map_err(|error| {
                        ErrorData::internal_error(
                            format!("could not seed `{}`: {error}", entry.source.path),
                            None,
                        )
                    })?;
                }
                PlannedDestination::Skip => {}
            }
        }
        Ok(())
    }
}

fn plan_scaffold(repo_root: &Path, profile: Profile) -> Result<ScaffoldPlan, ErrorData> {
    let mut emissions = Vec::new();
    for source in scaffold_sources(profile) {
        let destination = if let Some(relative) = source.path.strip_prefix(".agent/") {
            let target = repo_root.join(".agent").join(relative);
            let (existing, observed) = ObservedFile::read_string(&target).map_err(|error| {
                ErrorData::internal_error(
                    format!("could not inspect `{}`: {error}", source.path),
                    None,
                )
            })?;
            if existing.is_some() {
                PlannedDestination::Skip
            } else {
                PlannedDestination::Agent(WritePrecondition::new(observed))
            }
        } else if repo_root.join(&source.path).exists() {
            PlannedDestination::Skip
        } else {
            PlannedDestination::RepoRoot
        };
        emissions.push(PlannedEmission {
            source,
            destination,
        });
    }
    Ok(ScaffoldPlan { emissions })
}

/// Emit all existing-project scaffold targets and return their write outcomes.
pub(super) fn scaffold_project(
    repo_root: &Path,
    profile: Profile,
) -> Result<Vec<Emission>, ErrorData> {
    let plan = plan_scaffold(repo_root, profile)?;
    let emissions = plan.emissions();
    plan.commit(repo_root)?;
    Ok(emissions)
}

fn receipt_error(error: String) -> ErrorData {
    ErrorData::internal_error(format!("could not build mutation receipt: {error}"), None)
}

/// Build the tool result JSON from the emission outcomes.
pub(super) fn result_json(profile: Profile, emissions: &[Emission]) -> serde_json::Value {
    let wrote: Vec<&str> = emissions
        .iter()
        .filter_map(|e| match e {
            Emission::Wrote(p) => Some(p.as_str()),
            Emission::Skipped(_) => None,
        })
        .collect();
    let skipped: Vec<&str> = emissions
        .iter()
        .filter_map(|e| match e {
            Emission::Skipped(p) => Some(p.as_str()),
            Emission::Wrote(_) => None,
        })
        .collect();
    serde_json::json!({
        "profile": profile.name,
        "wrote": wrote,
        "skipped": skipped,
        // The scaffold prints this command for the human to run; it never runs it (R5.7).
        "next_step": HOOKS_PATH_COMMAND,
    })
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `scaffold`.
#[cfg(test)]
#[path = "scaffold_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod bootstrap_tests;

// Property and golden tests (Property 13) live in a separate sibling so the example-based
// unit tests stay focused. The `#[path]` include keeps them a child module of `scaffold`.
#[cfg(test)]
#[path = "scaffold_prop_tests.rs"]
mod prop_tests;
