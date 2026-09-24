//! Per-project feature flags and token caps, as data and their resolution.
//!
//! A feature flag turns a runtime capability on or off for one project. The ontology
//! feature is the first flag: it gates the ontology tools, the ontology resource, and the
//! ontology backing file. The flags are read from `.agent/config/rules.yml` under a
//! `features` block. The default is enabled, so a project with no config, no `config/`
//! directory, and no `.agent/` directory all resolve to an enabled ontology feature.
//!
//! `resolve` reads the flags once at startup. An absent `.agent/`, an absent
//! `.agent/config/`, or an absent `rules.yml` resolve to the default (Requirement 1.3). A
//! present file with no `features` block or no `ontology` key resolves the flag to enabled
//! (Requirement 1.4). A present file that cannot be read or parsed returns a typed error
//! with no partial value (Requirement 1.7, 1.8). The reader never consults the `.agent/`
//! layout contract, so a project with no layout contract still resolves the flag
//! (Requirement 1.10).
//!
//! The feature reader deserializes only `features`; the token-cap reader deserializes only
//! `token_caps`. Other `rules.yml` keys remain compatible. Both blocks use typed structs,
//! matching the crate's schema-first style.
//!
//! Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 1.10.
//! Design: optional-ontology §1, ADR-0012.

// The feature flags are consumed by `ServerContext` (issue #140). The ontology tool and
// resource gates (issues #141, #142) read `ctx.features` to decide what to register and
// serve.

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use crate::engine::agent_ws::AGENT_DIR;

/// The resolved per-project feature flags (Requirement 1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features {
    /// Whether the ontology feature is enabled (Requirement 1).
    pub ontology: bool,
    /// Whether the Jev evaluation harness is enabled (jev-integration-eval Requirement
    /// 1). Unlike `ontology`, the default is off: Jev is an optional network dependency on
    /// an external service, so a project must opt in (jev-integration-eval ADR-J1).
    pub jev: bool,
}

impl Default for Features {
    /// The default resolution: the ontology feature is enabled (Requirement 1.3, 1.4) and
    /// the Jev feature is off (jev-integration-eval Requirement 1.2, 1.3).
    fn default() -> Self {
        Self {
            ontology: true,
            jev: false,
        }
    }
}

/// The resolved token budgets for skill rendering and tool responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenCaps {
    /// The estimated-token budget for a Lean skill body.
    pub skill_lean_tokens: usize,
    /// The estimated-token budget for all text blocks in one tool response.
    pub tool_payload_tokens: usize,
}

impl Default for TokenCaps {
    fn default() -> Self {
        Self {
            skill_lean_tokens: 1_500,
            tool_payload_tokens: 4_000,
        }
    }
}

/// An error resolving the feature flags.
#[derive(Debug, Error)]
pub enum FeaturesError {
    /// The `.agent/config/rules.yml` file is present but could not be read (Requirement 1.7).
    #[error("could not read the feature config `{path}`: {source}")]
    Io {
        /// The config path.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// The `.agent/config/rules.yml` file could not be parsed (Requirement 1.8).
    #[error("could not parse the feature config `{path}`: {source}")]
    Parse {
        /// The config path.
        path: String,
        /// The underlying parse error.
        source: serde_yaml::Error,
    },

    /// A token-cap value was zero. Token budgets must be positive.
    #[error("invalid `.agent/config/rules.yml` value `{key}`: must be greater than zero")]
    InvalidTokenCap {
        /// The dotted rules.yml key.
        key: &'static str,
    },
}

/// The subset of `.agent/config/rules.yml` this reader needs.
///
/// Only the `features` block is deserialized. Every other key is ignored on read, so an
/// unrelated `rules.yml` key does not break the parse and is left untouched, because the
/// reader never writes (Requirement 1.9).
#[derive(Debug, Default, Deserialize)]
struct RulesFeatureView {
    #[serde(default)]
    features: FeaturesBlock,
}

/// The subset of `.agent/config/rules.yml` that declares token budgets.
#[derive(Debug, Default, Deserialize)]
struct RulesTokenCapsView {
    #[serde(default)]
    token_caps: TokenCapsBlock,
}

/// The `token_caps` block. Missing fields preserve the documented defaults.
#[derive(Debug, Deserialize)]
struct TokenCapsBlock {
    #[serde(default = "default_skill_lean_tokens")]
    skill_lean_tokens: usize,
    #[serde(default = "default_tool_payload_tokens")]
    tool_payload_tokens: usize,
}

impl Default for TokenCapsBlock {
    fn default() -> Self {
        Self {
            skill_lean_tokens: default_skill_lean_tokens(),
            tool_payload_tokens: default_tool_payload_tokens(),
        }
    }
}

fn default_skill_lean_tokens() -> usize {
    TokenCaps::default().skill_lean_tokens
}

fn default_tool_payload_tokens() -> usize {
    TokenCaps::default().tool_payload_tokens
}
/// The `features` block. A typed struct, one field per known feature (Requirement 1.5).
#[derive(Debug, Deserialize)]
struct FeaturesBlock {
    /// The ontology feature. An absent key resolves to enabled (Requirement 1.4).
    #[serde(default = "default_true")]
    ontology: bool,
    /// The Jev feature. An absent key resolves to off, so a `bool` default of `false`
    /// applies (jev-integration-eval Requirement 1.2).
    #[serde(default)]
    jev: bool,
}

impl Default for FeaturesBlock {
    /// An absent `features` block resolves the ontology feature to enabled (Requirement
    /// 1.4) and the Jev feature to off (jev-integration-eval Requirement 1.3).
    fn default() -> Self {
        Self {
            ontology: true,
            jev: false,
        }
    }
}

/// The default for a missing `ontology` key: enabled (Requirement 1.4).
fn default_true() -> bool {
    true
}

/// Resolve the feature flags from `.agent/config/rules.yml` (Requirement 1.2).
///
/// An absent `.agent/`, an absent `.agent/config/`, or an absent `rules.yml` all resolve to
/// [`Features::default`] (Requirement 1.3). A present file with no `features` block or no
/// `ontology` key resolves the flag to enabled (Requirement 1.4). A present file that
/// cannot be read or parsed returns a typed error with no partial value (Requirement 1.7,
/// 1.8). The reader never consults the `.agent/` layout contract (Requirement 1.10).
///
/// # Errors
///
/// Returns [`FeaturesError::Io`] when a present config cannot be read, and
/// [`FeaturesError::Parse`] when it cannot be parsed.
pub fn resolve(repo_root: &Path) -> Result<Features, FeaturesError> {
    let config_path = repo_root.join(AGENT_DIR).join("config").join("rules.yml");

    let text = match std::fs::read_to_string(&config_path) {
        Ok(text) => text,
        // An absent `.agent/`, `config/`, or `rules.yml` all surface as NotFound and resolve
        // to the default (Requirement 1.3).
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Features::default());
        }
        Err(source) => {
            return Err(FeaturesError::Io {
                path: config_path.display().to_string(),
                source,
            });
        }
    };

    let view: RulesFeatureView =
        serde_yaml::from_str(&text).map_err(|source| FeaturesError::Parse {
            path: config_path.display().to_string(),
            source,
        })?;

    Ok(Features {
        ontology: view.features.ontology,
        jev: view.features.jev,
    })
}

/// Resolve token caps from `.agent/config/rules.yml`.
///
/// Missing configuration preserves [`TokenCaps::default`]. Unknown rules remain ignored so
/// feature and optional-harness configuration retain their independent contracts.
pub fn resolve_token_caps(repo_root: &Path) -> Result<TokenCaps, FeaturesError> {
    let config_path = repo_root.join(AGENT_DIR).join("config").join("rules.yml");
    let text = match std::fs::read_to_string(&config_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TokenCaps::default());
        }
        Err(source) => {
            return Err(FeaturesError::Io {
                path: config_path.display().to_string(),
                source,
            });
        }
    };

    let view: RulesTokenCapsView =
        serde_yaml::from_str(&text).map_err(|source| FeaturesError::Parse {
            path: config_path.display().to_string(),
            source,
        })?;
    let caps = TokenCaps {
        skill_lean_tokens: view.token_caps.skill_lean_tokens,
        tool_payload_tokens: view.token_caps.tool_payload_tokens,
    };
    if caps.skill_lean_tokens == 0 {
        return Err(FeaturesError::InvalidTokenCap {
            key: "token_caps.skill_lean_tokens",
        });
    }
    if caps.tool_payload_tokens == 0 {
        return Err(FeaturesError::InvalidTokenCap {
            key: "token_caps.tool_payload_tokens",
        });
    }
    Ok(caps)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `features`.
#[cfg(test)]
#[path = "features_tests.rs"]
mod tests;
