//! Complete validation for workspace configuration that affects runtime behavior.

use std::path::Path;

use thiserror::Error;

use crate::engine::{agent_ws, backlog, features, profile};

/// An invalid workspace configuration.
#[derive(Debug, Error)]
pub enum WorkspaceValidationError {
    /// The workspace layout contract is invalid.
    #[error(transparent)]
    Layout(#[from] agent_ws::LayoutError),
    /// The active methodology profile is invalid.
    #[error(transparent)]
    Profile(#[from] profile::ProfileError),
    /// The backlog ownership declaration is invalid.
    #[error(transparent)]
    Backlog(#[from] backlog::BacklogError),
    /// Feature or token-cap configuration is invalid.
    #[error(transparent)]
    Features(#[from] features::FeaturesError),
}

/// Validate every repository-owned configuration surface used at runtime.
///
/// Operator-only process configuration, including the verify command and environment
/// allowlist, is intentionally excluded.
pub fn validate_workspace(repo_root: &Path) -> Result<(), WorkspaceValidationError> {
    agent_ws::read_layout(repo_root)?;
    profile::resolve_active(repo_root)?;
    backlog::validate_optional(repo_root)?;
    features::resolve(repo_root)?;
    features::resolve_token_caps(repo_root)?;
    Ok(())
}
