//! Skills tools: the tiered `get_skill` tool (task 8a) plus the ported legacy catalog
//! Skills tools: the tiered `get_skill` tool (task 8a).
//!
//! The legacy catalog tools live in the sibling `catalog` module (task 8b).
//!
//! `get_skill` renders a skill at the full, reasoning, or lean tier. The effective tier
//! resolves from the per-call argument, then the `TRUENORTH_TIER` environment variable,
//! else `full` (Requirements 6.4, 6.5). An unknown tier or an unresolved name is rejected
//! without a payload (Requirements 6.2, 6.3).
//!
//! Requirements: 6.1, 6.2, 6.3, 6.4, 6.5. Design: Part II §2, §4.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::agnostic::ADAPTATION_NOTE;
use crate::engine::skill::{SkillError, read_skill_raw};
use crate::engine::tier::{Tier, render_skill};
use crate::server::TrueNorthServer;
use crate::tools::result;

/// The environment variable that sets the default skill tier.
const TIER_ENV: &str = "TRUENORTH_TIER";

/// The `get_skill` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSkillArgs {
    /// The skill directory name (verb-noun, kebab-case).
    pub name: String,
    /// The rendering tier. Overrides `TRUENORTH_TIER` for this call. One of `full`,
    /// `reasoning`, or `lean`.
    #[serde(default)]
    pub tier: Option<String>,
}

#[tool_router(router = skills_router, vis = "pub")]
impl TrueNorthServer {
    /// Read a skill at the full, reasoning, or lean tier.
    #[tool(description = "Read a skill's SKILL.md rendered at a tier (full | reasoning | lean).")]
    pub async fn get_skill(
        &self,
        params: Parameters<GetSkillArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Resolve the effective tier before touching disk, so an unknown tier is rejected
        // without a read (Requirement 6.2).
        let tier = resolve_tier(args.tier.as_deref())?;

        let raw = read_skill_raw(&self.ctx.repo_root, &args.name).map_err(skill_error)?;
        let rendered = render_skill(&raw.markdown, tier, self.ctx.token_caps.skill_lean_tokens);

        // The skill content is the first block, byte-identical at the full tier
        // (Requirement 6.6). A second block states that no model-specific or
        // harness-specific adaptation was applied, since the server never identifies the
        // client (Requirement 11.5). The content is identical for every client
        // (Requirements 11.2, 11.3).
        result::success(
            vec![
                ContentBlock::text(rendered),
                ContentBlock::text(ADAPTATION_NOTE),
            ],
            self.ctx.token_caps,
        )
    }
}

/// Resolve the effective tier from the per-call argument, then `TRUENORTH_TIER`, else
/// `full` (Requirements 6.4, 6.5).
///
/// # Errors
///
/// Returns an invalid-params error when a supplied tier value is not one of the three
/// tiers (Requirement 6.2).
fn resolve_tier(arg: Option<&str>) -> Result<Tier, ErrorData> {
    if let Some(value) = arg {
        return parse_tier(value);
    }
    match std::env::var(TIER_ENV) {
        Ok(value) if !value.is_empty() => parse_tier(&value),
        // An unset or empty env var falls back to full (Requirement 6.5). An unrecognized
        // env value is treated as absent, so the default still applies rather than
        // failing a call that supplied no tier.
        _ => Ok(Tier::Full),
    }
}

/// Parse a tier string into a [`Tier`].
///
/// # Errors
///
/// Returns an invalid-params error for an unrecognized value (Requirement 6.2).
fn parse_tier(value: &str) -> Result<Tier, ErrorData> {
    match value {
        "full" => Ok(Tier::Full),
        "reasoning" => Ok(Tier::Reasoning),
        "lean" => Ok(Tier::Lean),
        other => Err(ErrorData::invalid_params(
            format!("unrecognized tier `{other}`. Use one of: full, reasoning, lean."),
            None,
        )),
    }
}

/// Map a skill resolution error to an MCP error (Requirement 6.3).
fn skill_error(error: SkillError) -> ErrorData {
    match error {
        SkillError::InvalidName(_) | SkillError::PathEscape(_) | SkillError::NotFound(_) => {
            ErrorData::invalid_params(error.to_string(), None)
        }
        SkillError::Read { .. } => ErrorData::internal_error(error.to_string(), None),
    }
}

/// A test-only helper: the raw skill a `get_skill` call would render, for asserting the
/// resolution path without an rmcp round-trip.
#[cfg(test)]
fn resolve_for_test(
    repo_root: &std::path::Path,
    name: &str,
) -> Result<crate::engine::skill::RawSkill, SkillError> {
    read_skill_raw(repo_root, name)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `skills`.
#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
