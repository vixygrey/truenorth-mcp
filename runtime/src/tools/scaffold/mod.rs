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
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::agent_ws::{write_repo_seed, write_under_agent};
use crate::engine::profile::{self, Profile};
use crate::server::TrueNorthServer;
use crate::tools::hooks::{commit_msg_hook, post_merge_hook};

mod bootstrap;
mod templates;

pub use bootstrap::bootstrap_project;

use templates::{
    COMMIT_TEMPLATE, EXECUTION_STATUS_SEED, ISSUE_TEMPLATE_CONFIG, JEV_GUARD_HOOK,
    PULL_REQUEST_TEMPLATE, agents_md, bug_form, conventions_md, feature_form, layout_contract,
    starter_seed,
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
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Resolve the profile. Absent uses issue-per-task; an unknown name makes no file
        // change and returns an error naming the value and the known names (R5.1, R5.2).
        let profile = resolve_scaffold_profile(args.profile.as_deref())?;

        let emissions = scaffold_project(&self.ctx.repo_root, profile)?;

        result::success(
            vec![ContentBlock::text(
                result_json(profile, &emissions).to_string(),
            )],
            self.ctx.token_caps,
        )
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

/// Emit all existing-project scaffold targets and return their write outcomes.
///
/// Both the MCP tool and the command-line bootstrap use this shared core so the workspace
/// layout, profile seeds, root docs, hooks, and GitHub templates cannot drift apart.
pub(super) fn scaffold_project(
    repo_root: &Path,
    profile: Profile,
) -> Result<Vec<Emission>, ErrorData> {
    let mut emissions = Vec::new();
    emit_agent_tree(repo_root, profile, &mut emissions)?;
    emit_root_docs(repo_root, profile, &mut emissions)?;
    emit_hooks(repo_root, profile, &mut emissions)?;
    emit_github(repo_root, profile, &mut emissions)?;
    Ok(emissions)
}

/// Emit the `.agent/` tree and the cockpit seed files through the write guard.
///
/// The tree follows the layout contract (design §1.1). Each file is written only when
/// absent, so an existing `.agent/` tree is left unchanged (Requirement 5.12).
fn emit_agent_tree(
    repo_root: &Path,
    profile: Profile,
    out: &mut Vec<Emission>,
) -> Result<(), ErrorData> {
    // The layout contract and the active profile name.
    seed_under_agent(repo_root, "layout.yml", &layout_contract(), out)?;
    seed_under_agent(
        repo_root,
        "profile.yml",
        &format!("profile: {}\n", profile.name),
        out,
    )?;

    // The required area files (Requirement 1.5 to 1.9).
    let area_files: [(&str, &str); 8] = [
        (
            "config/rules.yml",
            "# Runtime configuration.\n# Token estimates use ceil(characters / 4). Oversized tool responses fail without truncation.\ntoken_caps:\n  skill_lean_tokens: 1500\n  tool_payload_tokens: 4000\n",
        ),
        ("spec/requirements.md", "# Requirements\n"),
        ("tasks/state.yml", "phase: discover\n"),
        ("tasks/execution-status.yml", EXECUTION_STATUS_SEED),
        ("memories/lessons.md", "# Lessons\n"),
        ("memories/glossary.md", "# Glossary\n"),
        ("product/scope.md", "# Product scope\n"),
        ("telemetry/runs.yml", "runs: []\n"),
    ];
    for (rel, body) in area_files {
        seed_under_agent(repo_root, rel, body, out)?;
    }

    // The profile's starter files, seeded as empty-but-valid cockpit files (R5.3).
    for starter in profile.starter_files {
        seed_under_agent(repo_root, starter, &starter_seed(starter), out)?;
    }

    Ok(())
}

/// Emit the root `AGENTS.md` and `CONVENTIONS.md` through the audited repo-seed path.
///
/// Both are wired to `.agent/`, never to `specs/` (Requirement 5.5). They are written
/// only when absent (Requirement 5.12).
fn emit_root_docs(
    repo_root: &Path,
    profile: Profile,
    out: &mut Vec<Emission>,
) -> Result<(), ErrorData> {
    seed_repo_root(repo_root, "AGENTS.md", &agents_md(profile), out)?;
    seed_repo_root(repo_root, "CONVENTIONS.md", &conventions_md(profile), out)?;
    Ok(())
}

/// Emit the two git hooks into `.githooks/` through the audited repo-seed path.
///
/// The hooks are templated per profile (Requirement 5.6). The scaffold prints the
/// `core.hooksPath` command in its result and never runs it (Requirement 5.7).
fn emit_hooks(
    repo_root: &Path,
    profile: Profile,
    out: &mut Vec<Emission>,
) -> Result<(), ErrorData> {
    seed_repo_root(
        repo_root,
        ".githooks/commit-msg",
        &commit_msg_hook(profile),
        out,
    )?;
    seed_repo_root(
        repo_root,
        ".githooks/post-merge",
        &post_merge_hook(profile),
        out,
    )?;

    // The emitted PreToolUse hook that calls the guardrail on a write tool (R7.1). It is
    // profile-independent, so it is the same for every profile. The automatic interception
    // holds only where the client honors the hook; the guard tool is the fallback (R7.4).
    seed_repo_root(repo_root, ".kiro/hooks/jev-guard.json", JEV_GUARD_HOOK, out)?;
    Ok(())
}

/// Emit the `.github/` templates through the audited repo-seed path.
///
/// The commit and PR templates are neutral and profile-identical (Requirement 5.8). The
/// issue forms are generic, with no copied domain content and no hardcoded URLs
/// (Requirement 5.9, 5.10). The forms include a grouping field when the profile vocabulary
/// is epic or milestone, and omit the issue-id field when the profile is kanban or generic
/// (Requirement 5.11).
fn emit_github(
    repo_root: &Path,
    profile: Profile,
    out: &mut Vec<Emission>,
) -> Result<(), ErrorData> {
    seed_repo_root(
        repo_root,
        ".github/commit-template.md",
        COMMIT_TEMPLATE,
        out,
    )?;
    seed_repo_root(
        repo_root,
        ".github/pull-request-template.md",
        PULL_REQUEST_TEMPLATE,
        out,
    )?;
    seed_repo_root(
        repo_root,
        ".github/ISSUE_TEMPLATE/bug.md",
        &bug_form(profile),
        out,
    )?;
    seed_repo_root(
        repo_root,
        ".github/ISSUE_TEMPLATE/feature.md",
        &feature_form(profile),
        out,
    )?;
    seed_repo_root(
        repo_root,
        ".github/ISSUE_TEMPLATE/config.yml",
        ISSUE_TEMPLATE_CONFIG,
        out,
    )?;
    Ok(())
}

/// Write `rel` under `.agent/` when absent, recording the outcome (Requirement 5.12).
fn seed_under_agent(
    repo_root: &Path,
    rel: &str,
    body: &str,
    out: &mut Vec<Emission>,
) -> Result<(), ErrorData> {
    let target = repo_root.join(".agent").join(rel);
    let display = format!(".agent/{rel}");
    if target.exists() {
        out.push(Emission::Skipped(display));
        return Ok(());
    }
    write_under_agent(repo_root, Path::new(rel), body)
        .map_err(|e| ErrorData::internal_error(format!("could not seed `{display}`: {e}"), None))?;
    out.push(Emission::Wrote(display));
    Ok(())
}

/// Write a repo-root `rel` when absent, recording the outcome (Requirement 5.12).
fn seed_repo_root(
    repo_root: &Path,
    rel: &str,
    body: &str,
    out: &mut Vec<Emission>,
) -> Result<(), ErrorData> {
    let target = repo_root.join(rel);
    if target.exists() {
        out.push(Emission::Skipped(rel.to_string()));
        return Ok(());
    }
    write_repo_seed(repo_root, Path::new(rel), body, true)
        .map_err(|e| ErrorData::internal_error(format!("could not seed `{rel}`: {e}"), None))?;
    out.push(Emission::Wrote(rel.to_string()));
    Ok(())
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
