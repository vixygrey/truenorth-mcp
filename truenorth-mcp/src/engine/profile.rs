//! The five built-in methodology profiles, as fixed data, and their resolution.
//!
//! A methodology profile declares a project workflow shape: a grouping vocabulary,
//! whether grouping is required or optional, the starter files seeded under `.agent/`,
//! the branch pattern the post-merge sweep matches, and whether the commit-msg hook
//! requires an issue id. The profiles are a fixed `const` table (ADR-8). There is no
//! registration path, so a custom profile cannot be defined in this spec (Requirement
//! 3.7).
//!
//! `resolve_active` reads the active profile name from `.agent/profile.yml`. An absent
//! config resolves to the issue-per-task default (Requirement 3.5). An unknown name
//! returns an error naming the value and the five valid names, with no partial state
//! (Requirement 3.6).
//!
//! Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7. Design: agent-workspace-profiles §3,
//! ADR-8.

// The profile table and its resolution are consumed by later tasks: the neutral grouping
// key in `record_task` (task 5), the scaffold (task 9), and the emitted hooks (task 10).
// The items are unused until those tasks wire them, so the module-scoped allow prevents a
// premature dead-code error under `clippy -D warnings`. Remove this allow once task 10
// wires the last consumer.
#![allow(dead_code)]

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::engine::agent_ws::AGENT_DIR;

/// The grouping vocabulary a profile uses (Requirement 3.3).
///
/// A profile groups tasks under exactly one label, or under none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupingVocab {
    /// Group tasks under an epic.
    Epic,
    /// Group tasks under a sprint.
    Sprint,
    /// Group tasks under a milestone.
    Milestone,
    /// Group tasks under a ticket.
    Ticket,
    /// No grouping.
    None,
}

/// Whether a profile requires a grouping key (Requirement 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingRule {
    /// A `truenorth_record_task` call must supply a grouping key.
    Required,
    /// A `truenorth_record_task` call can omit the grouping key.
    Optional,
}

/// A built-in methodology profile, as fixed data (Requirement 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// The methodology name, one of the five built-ins.
    pub name: &'static str,
    /// The grouping vocabulary, exactly one label.
    pub vocab: GroupingVocab,
    /// Whether grouping is required or optional.
    pub rule: GroupingRule,
    /// The starter file set seeded under `.agent/` for this profile.
    pub starter_files: &'static [&'static str],
    /// The branch-name pattern the post-merge sweep matches (Requirement 7.7).
    pub branch_pattern: &'static str,
    /// Whether the commit-msg hook requires an id reference (Requirement 6.6).
    pub require_issue_id: bool,
}

/// The epic-based profile: required epic grouping.
pub const EPIC_BASED: Profile = Profile {
    name: "epic-based",
    vocab: GroupingVocab::Epic,
    rule: GroupingRule::Required,
    starter_files: &["tasks/release-plan.yml", "tasks/backlog.yml"],
    branch_pattern: r"^(feat|fix)/e[0-9]+",
    require_issue_id: true,
};

/// The issue-per-task profile: the default, optional ticket grouping (Requirement 3.2).
pub const ISSUE_PER_TASK: Profile = Profile {
    name: "issue-per-task",
    vocab: GroupingVocab::Ticket,
    rule: GroupingRule::Optional,
    starter_files: &["tasks/backlog.yml"],
    branch_pattern: r"^(feat|fix|chore)/",
    require_issue_id: true,
};

/// The kanban continuous-flow profile: no grouping, no id required.
pub const KANBAN: Profile = Profile {
    name: "kanban",
    vocab: GroupingVocab::None,
    rule: GroupingRule::Optional,
    starter_files: &["tasks/backlog.yml"],
    branch_pattern: r"^(feat|fix|chore)/",
    require_issue_id: false,
};

/// The milestone/release-based profile: required milestone grouping.
pub const MILESTONE_BASED: Profile = Profile {
    name: "milestone-based",
    vocab: GroupingVocab::Milestone,
    rule: GroupingRule::Required,
    starter_files: &["tasks/release-plan.yml"],
    branch_pattern: r"^(feat|fix)/m[0-9]+",
    require_issue_id: true,
};

/// The generic no-grouping profile: no grouping, no id required.
pub const GENERIC: Profile = Profile {
    name: "generic",
    vocab: GroupingVocab::None,
    rule: GroupingRule::Optional,
    starter_files: &["tasks/backlog.yml"],
    branch_pattern: r"^(feat|fix|chore)/",
    require_issue_id: false,
};

/// The five built-in profiles (Requirement 3.1).
pub const ALL_PROFILES: [Profile; 5] =
    [EPIC_BASED, ISSUE_PER_TASK, KANBAN, MILESTONE_BASED, GENERIC];

/// An error from profile resolution.
#[derive(Debug, Error)]
pub enum ProfileError {
    /// The declared profile name is not one of the five built-ins (Requirement 3.6).
    #[error(
        "unknown methodology profile `{name}`. \
         The valid profiles are: epic-based, issue-per-task, kanban, milestone-based, \
         generic."
    )]
    UnknownProfile {
        /// The unknown value, as read from config.
        name: String,
    },

    /// The `.agent/profile.yml` file could not be read (a present but unreadable file).
    #[error("could not read the profile config `{path}`: {source}")]
    Io {
        /// The config path.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// The `.agent/profile.yml` file could not be parsed as the expected shape.
    #[error("could not parse the profile config `{path}`: {source}")]
    Parse {
        /// The config path.
        path: String,
        /// The underlying parse error.
        source: serde_yaml::Error,
    },
}

/// The parsed `.agent/profile.yml` shape: `{ profile: <name> }` (design §3.3).
#[derive(Debug, Deserialize)]
struct ProfileConfig {
    profile: String,
}

/// Look up a profile by name, or `None` when the name is unknown (Requirement 3.6).
pub fn by_name(name: &str) -> Option<Profile> {
    ALL_PROFILES.into_iter().find(|p| p.name == name)
}

/// Resolve the active profile from `.agent/profile.yml`.
///
/// An absent config resolves to [`ISSUE_PER_TASK`] (Requirement 3.5). A present config
/// names one profile; an unknown name returns [`ProfileError::UnknownProfile`] naming the
/// value and the five valid names, and retains no partial profile state (Requirement
/// 3.6).
///
/// # Errors
///
/// Returns [`ProfileError::Io`] when a present config cannot be read,
/// [`ProfileError::Parse`] when it cannot be parsed, and
/// [`ProfileError::UnknownProfile`] when it names a profile outside the five built-ins.
pub fn resolve_active(repo_root: &Path) -> Result<Profile, ProfileError> {
    let config_path = repo_root.join(AGENT_DIR).join("profile.yml");

    let text = match std::fs::read_to_string(&config_path) {
        Ok(text) => text,
        // An absent config resolves to the default (Requirement 3.5).
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ISSUE_PER_TASK);
        }
        Err(source) => {
            return Err(ProfileError::Io {
                path: config_path.display().to_string(),
                source,
            });
        }
    };

    let config: ProfileConfig =
        serde_yaml::from_str(&text).map_err(|source| ProfileError::Parse {
            path: config_path.display().to_string(),
            source,
        })?;

    by_name(&config.profile).ok_or(ProfileError::UnknownProfile {
        name: config.profile,
    })
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `profile`.
#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
