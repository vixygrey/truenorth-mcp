//! Lifecycle phase map for skills (ports `phase-map.ts`).
//!
//! `phase_for_skill` returns the lifecycle phase for a skill name, mirroring the legacy
//! `PHASE_MAP`. The catalog tools report these legacy phase names verbatim, so existing
//! agent flows keep working (Requirement 7.1). The six-phase mapping used for lifecycle
//! transitions lives in [`crate::engine::validate`] (Requirement 9.5).
//!
//! Requirements: 7.1. Design: Part II §8.

// The phase map is consumed by the skills tools (task 8). It is unused until they wire
// it, so the module-scoped allow prevents a premature dead-code error under
// `clippy -D warnings`. Remove this allow once task 8 wires the consumer.
#![allow(dead_code)]

/// The lifecycle phase for a skill name (ports `phaseForSkill`).
///
/// An unmapped name returns `"Other"`, matching the legacy default.
pub fn phase_for_skill(name: &str) -> String {
    phase_lookup(name).unwrap_or("Other").to_string()
}

/// The raw phase lookup, mirroring the legacy `PHASE_MAP` (`phase-map.ts`).
fn phase_lookup(name: &str) -> Option<&'static str> {
    let phase = match name {
        "survey-context" | "research-first" | "search-skills" | "using-truenorth"
        | "map-codebase" | "elaborate-spec" => "Discover",

        "model-domain"
        | "define-language"
        | "grill-me"
        | "grill-with-docs"
        | "deepen-architecture"
        | "design-interface" => "Design",

        "scope-work" | "slice-tasks" | "plan-work" | "plan-release" | "plan-refactor"
        | "assess-impact" | "change-request" | "run-planning" | "seed-conventions"
        | "plan-tests" => "Plan",

        "develop-tdd"
        | "kickoff-branch"
        | "execute-plan"
        | "build-epic"
        | "spike-prototype"
        | "craft-skill"
        | "quick-fix"
        | "setup-environment"
        | "wire-observability"
        | "wire-ci"
        | "publish-package"
        | "align-grid"
        | "orchestrate-project"
        | "guard-git"
        | "hook-commits"
        | "deploy"
        | "smoke-test"
        | "validate-contracts" => "Build",

        "verify-work" | "validate-fix" | "audit-code" | "enforce-first" | "run-evals"
        | "investigate-bug" | "diagnose-root" | "fix-bug" | "inspect-quality"
        | "request-review" | "respond-review" | "trace-requirement" | "gate-trace"
        | "security-review" => "Verify",

        "release-branch" | "commit-message" => "Release",

        "session-state" | "terse-mode" | "compose-workflow" | "delegate-task"
        | "dispatch-agents" | "edit-document" | "evolve-skill" | "migrate-spec"
        | "organize-workspace" | "reset-baseline" | "simulate-agents" | "stocktake-skills"
        | "write-document" => "Sustain",

        _ => return None,
    };
    Some(phase)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `phase`.
#[cfg(test)]
#[path = "phase_tests.rs"]
mod tests;
