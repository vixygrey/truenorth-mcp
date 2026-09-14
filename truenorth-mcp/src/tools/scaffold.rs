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

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::agent_ws::{write_repo_seed, write_under_agent};
use crate::engine::profile::{self, GroupingVocab, Profile};
use crate::server::TrueNorthServer;
use crate::tools::hooks::{commit_msg_hook, post_merge_hook};

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

        let mut emissions = Vec::new();
        emit_agent_tree(&self.ctx.repo_root, profile, &mut emissions)?;
        emit_root_docs(&self.ctx.repo_root, profile, &mut emissions)?;
        emit_hooks(&self.ctx.repo_root, profile, &mut emissions)?;
        emit_github(&self.ctx.repo_root, profile, &mut emissions)?;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            result_json(profile, &emissions).to_string(),
        )]))
    }
}

/// Resolve the profile for the scaffold (Requirements 5.1, 5.2).
///
/// An absent name uses issue-per-task. A name longer than 64 chars or outside the five
/// built-ins returns an error and makes no file change.
fn resolve_scaffold_profile(name: Option<&str>) -> Result<Profile, ErrorData> {
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
    let area_files: [(&str, &str); 7] = [
        (
            "config/rules.yml",
            "# Token caps, human-approval gates, protected paths.\n",
        ),
        ("spec/requirements.md", "# Requirements\n"),
        ("tasks/state.yml", "phase: discover\n"),
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

/// The language-agnostic layout contract seeded at `.agent/layout.yml`.
fn layout_contract() -> String {
    // A data description of the required areas, parseable by any language (Requirement
    // 1.11). It lists the areas the runtime validates on read (Requirement 1.12).
    "\
version: '1'
areas:
  config: [rules.yml]
  spec: [requirements.md]
  tasks: [state.yml]
  memories: [lessons.md, glossary.md]
  telemetry: [runs.yml]
"
    .to_string()
}

/// The seed body for a profile starter file.
fn starter_seed(rel: &str) -> String {
    match rel {
        "tasks/release-plan.yml" => "tasks: []\n".to_string(),
        "tasks/backlog.yml" => "backlog: []\n".to_string(),
        _ => "{}\n".to_string(),
    }
}

/// The root `AGENTS.md`, wired to `.agent/` and templated per profile (Requirement 5.5).
fn agents_md(profile: Profile) -> String {
    format!(
        "# Agent workspace\n\n\
         This project uses the `{}` methodology profile.\n\n\
         The machine-facing workspace is `.agent/`. The runtime reads, watches, and writes\n\
         only under `.agent/`. Human-authored narrative lives under `specs/`.\n\n\
         - Cockpit state: `.agent/tasks/`\n\
         - Ontology: `.agent/ontology.yml`\n\
         - Product scope and vision: `.agent/product/`\n",
        profile.name
    )
}

/// The root `CONVENTIONS.md`, wired to `.agent/` and templated per profile.
fn conventions_md(profile: Profile) -> String {
    format!(
        "# Conventions\n\n\
         Methodology profile: `{}`.\n\n\
         Commits are atomic and follow Conventional Commits. The grouping key for a task\n\
         is `{}`.\n",
        profile.name,
        grouping_label(profile.vocab),
    )
}

/// The neutral commit-message template, identical for every profile (Requirement 5.8).
const COMMIT_TEMPLATE: &str = "\
# type(scope): description
#
# Commits are atomic and follow Conventional Commits.
# type is one of: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.
";

/// The neutral pull-request template, identical for every profile (Requirement 5.8).
const PULL_REQUEST_TEMPLATE: &str = "\
## Summary

## Changes

## Tested

<!-- Commits are atomic and follow Conventional Commits. Link the issue this PR resolves. -->
";

/// The issue-template config with placeholder contact links (Requirement 5.9, 5.10).
const ISSUE_TEMPLATE_CONFIG: &str = "\
blank_issues_enabled: false
contact_links:
  - name: Question
    url: https://example.invalid/discussions
    about: Ask a question here.
";

/// The bug issue form, generic and profile-aware (Requirements 5.9, 5.10, 5.11).
fn bug_form(profile: Profile) -> String {
    let mut form = String::from(
        "---\nname: Bug report\nabout: Report a bug\nlabels: bug\n---\n\n\
         ## What happened\n\n## Steps to reproduce\n\n## Expected behavior\n\n\
         ## External tracker link\n\n",
    );
    push_grouping_and_id_fields(&mut form, profile);
    form
}

/// The feature issue form, generic and profile-aware (Requirements 5.9, 5.10, 5.11).
fn feature_form(profile: Profile) -> String {
    let mut form = String::from(
        "---\nname: Feature request\nabout: Request a feature\nlabels: enhancement\n---\n\n\
         ## Problem\n\n## Proposed solution\n\n",
    );
    push_grouping_and_id_fields(&mut form, profile);
    form
}

/// Append the profile-conditional grouping and issue-id fields to an issue form.
///
/// A grouping field appears when the profile vocabulary is epic or milestone
/// (Requirement 5.11). The issue-id field is omitted when the profile does not require an
/// id (kanban and generic), matching the commit-msg hook (Requirement 5.11).
fn push_grouping_and_id_fields(form: &mut String, profile: Profile) {
    if matches!(
        profile.vocab,
        GroupingVocab::Epic | GroupingVocab::Milestone
    ) {
        form.push_str(&format!("## {} id\n\n", grouping_label(profile.vocab)));
    }
    if profile.require_issue_id {
        form.push_str("## Issue or ticket id\n\n");
    }
}

/// The human-facing label for a profile's grouping vocabulary.
fn grouping_label(vocab: GroupingVocab) -> &'static str {
    match vocab {
        GroupingVocab::Epic => "epic",
        GroupingVocab::Sprint => "sprint",
        GroupingVocab::Milestone => "milestone",
        GroupingVocab::Ticket => "ticket",
        GroupingVocab::None => "none",
    }
}

/// Build the tool result JSON from the emission outcomes.
fn result_json(profile: Profile, emissions: &[Emission]) -> serde_json::Value {
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

// Property and golden tests (Property 13) live in a separate sibling so the example-based
// unit tests stay focused. The `#[path]` include keeps them a child module of `scaffold`.
#[cfg(test)]
#[path = "scaffold_prop_tests.rs"]
mod prop_tests;
