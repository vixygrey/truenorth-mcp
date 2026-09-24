//! The content templates the scaffold emits.
//!
//! These are pure functions and constants that produce the seed bytes for the `.agent/`
//! tree, the root docs, and the `.github/` templates. They hold no orchestration or file
//! I/O; the parent module wires them to the write guard. Keeping them here holds the
//! scaffold orchestration under the size guidance and isolates the profile-templated
//! content in one place (Requirement 5.5, 5.8, 5.9, 5.10, 5.11).

use crate::engine::profile::{GroupingVocab, Profile};

/// The language-agnostic layout contract seeded at `.agent/layout.yml`.
pub(super) fn layout_contract() -> String {
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

/// The empty execution-status cockpit shared by every methodology profile.
pub(super) const EXECUTION_STATUS_SEED: &str = "stories: {}\ndevelopment_status: {}\n";

/// The seed body for a profile starter file.
pub(super) fn starter_seed(rel: &str) -> String {
    match rel {
        "tasks/release-plan.yml" => "tasks: []\n".to_string(),
        "tasks/backlog.yml" => "version: '1'\nownership:\n  mode: local\nbacklog: []\n".to_string(),
        _ => "{}\n".to_string(),
    }
}

/// The root `AGENTS.md`, wired to `.agent/` and templated per profile (Requirement 5.5).
pub(super) fn agents_md(profile: Profile) -> String {
    format!(
        "# Agent workspace\n\n\
         This project uses the `{}` methodology profile.\n\n\
         The machine-facing workspace is `.agent/`. The runtime reads, watches, and writes\n\
         only under `.agent/`. Human-authored narrative lives under `specs/`.\n\n\
         - Cockpit state: `.agent/tasks/`\n\
         - Backlog ownership: `.agent/tasks/backlog.yml`\n\
         - Ontology: `.agent/ontology.yml`\n\
         - Product scope and vision: `.agent/product/`\n",
        profile.name
    )
}

/// The root `CONVENTIONS.md`, wired to `.agent/` and templated per profile.
pub(super) fn conventions_md(profile: Profile) -> String {
    format!(
        "# Conventions\n\n\
         Methodology profile: `{}`.\n\n\
         Commits are atomic and follow Conventional Commits. The grouping key for a task\n\
         is `{}`.\n\n\
         Backlog ownership is independent of methodology. `.agent/tasks/backlog.yml`\n\
         declares local or external ownership. The runtime does not contact external\n\
         trackers by default.\n",
        profile.name,
        grouping_label(profile.vocab),
    )
}

/// The emitted `PreToolUse` hook that calls the guardrail on a write tool (R7.1, R7.4).
///
/// The hook is profile-independent: the guardrail is the same regardless of the methodology,
/// so this is a constant, not a templated function. It matches the common write tools and runs
/// the guard CLI on standard input. The `description` field states the honest boundary: the
/// automatic interception holds only where the client honors the `PreToolUse` hook, and the
/// `truenorth_guard_change` tool is the fallback everywhere else (R7.4, R9.4).
///
/// The command runs `truenorth-mcp guard`, which reads the proposed change on standard input,
/// runs the guardrail, and exits non-zero on a block. The client passes the proposed write to
/// the hook on standard input.
pub(super) const JEV_GUARD_HOOK: &str = "\
{
  \"version\": \"v1\",
  \"hooks\": [
    {
      \"name\": \"Jev active guardrail\",
      \"description\": \"Runs the TrueNorth guardrail before a write tool. Automatic interception depends on the client honoring the PreToolUse hook. Where the client does not honor it, call the truenorth_guard_change tool before a write instead. TrueNorth cannot force-intercept a write an agent makes to a foreign tool.\",
      \"trigger\": \"PreToolUse\",
      \"matcher\": \"write_to_file|fs_write|str_replace\",
      \"action\": { \"type\": \"command\", \"command\": \"truenorth-mcp guard\" }
    }
  ]
}
";

/// The neutral commit-message template, identical for every profile (Requirement 5.8).
pub(super) const COMMIT_TEMPLATE: &str = "\
# type(scope): description
#
# Commits are atomic and follow Conventional Commits.
# type is one of: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.
";

/// The neutral pull-request template, identical for every profile (Requirement 5.8).
pub(super) const PULL_REQUEST_TEMPLATE: &str = "\
## Summary

## Changes

## Tested

<!-- Commits are atomic and follow Conventional Commits. Link the issue this PR resolves. -->
";

/// The issue-template config with placeholder contact links (Requirement 5.9, 5.10).
pub(super) const ISSUE_TEMPLATE_CONFIG: &str = "\
blank_issues_enabled: false
contact_links:
  - name: Question
    url: https://example.invalid/discussions
    about: Ask a question here.
";

/// The bug issue form, generic and profile-aware (Requirements 5.9, 5.10, 5.11).
pub(super) fn bug_form(profile: Profile) -> String {
    let mut form = String::from(
        "---\nname: Bug report\nabout: Report a bug\nlabels: bug\n---\n\n\
         ## What happened\n\n## Steps to reproduce\n\n## Expected behavior\n\n\
         ## External tracker link\n\n",
    );
    push_grouping_and_id_fields(&mut form, profile);
    form
}

/// The feature issue form, generic and profile-aware (Requirements 5.9, 5.10, 5.11).
pub(super) fn feature_form(profile: Profile) -> String {
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
