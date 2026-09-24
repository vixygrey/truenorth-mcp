//! The ported legacy catalog tools (task 8b).
//!
//! These tools keep the existing agent flows working during migration (Requirement 7.1).
//! They are exposed alongside the active `truenorth_*` tools. Each tool returns a JSON
//! text result on success and an [`ErrorData`] on failure, faithful to the legacy
//! contracts and error strings.
//!
//! Requirements: 7.1 to 7.9. Design: Part II §2.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::engine::agent_ws;
use crate::engine::git::{self, GitAction, GitError};
use crate::engine::graph::{self, SkillGraph};
use crate::engine::skill::{SkillError, discover_skills, read_skill_raw};
use crate::engine::skill_parser::{ParsedSkill, parse_skill};
use crate::engine::skill_validate::validate_skill;
use crate::server::{ServerContext, TrueNorthServer};
use crate::tools::result;

/// Render a serializable value as a pretty JSON tool result.
fn json_result<T: Serialize>(
    value: &T,
    caps: crate::engine::features::TokenCaps,
) -> Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| ErrorData::internal_error(format!("could not serialize result: {e}"), None))?;
    result::success(vec![ContentBlock::text(text)], caps)
}

/// Map a skill resolution error to an MCP error, preserving the legacy message.
fn skill_error(error: SkillError) -> ErrorData {
    match error {
        SkillError::InvalidName(_) | SkillError::PathEscape(_) | SkillError::NotFound(_) => {
            ErrorData::invalid_params(error.to_string(), None)
        }
        SkillError::Read { .. } => ErrorData::internal_error(error.to_string(), None),
    }
}

/// The `read_skill` and `validate_skill` name argument.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NameArg {
    /// The skill directory name (verb-noun, kebab-case).
    pub name: String,
}

/// The `search_skills` argument.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSkillsArgs {
    /// The search query.
    pub query: String,
    /// Whether to match the whole skill name exactly rather than a substring.
    #[serde(default)]
    pub exact: bool,
}

/// The `search_nodes` argument.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryArg {
    /// The search query over graph entities.
    pub query: String,
}

/// The `open_nodes` argument.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NamesArg {
    /// The entity names to open.
    pub names: Vec<String>,
}

/// The `get_git_context` argument.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitContextArgs {
    /// The git action: `status`, `log`, or `diff`. Defaults to `status`.
    #[serde(default)]
    pub action: Option<String>,
}

/// A search result entry for `search_skills`.
#[derive(Debug, Serialize)]
struct SearchResult {
    name: String,
    path: String,
    phase: String,
    score: usize,
}

#[tool_router(router = catalog_router, vis = "pub")]
impl TrueNorthServer {
    /// Enumerate skills with name, path, and phase.
    #[tool(description = "List all skills with name, relative path, and lifecycle phase.")]
    pub async fn index_skills(&self) -> Result<CallToolResult, ErrorData> {
        let skills = discover_skills(&self.ctx.repo_root);
        json_result(
            &serde_json::json!({ "count": skills.len(), "skills": skills_json(&skills) }),
            self.ctx.token_caps,
        )
    }

    /// Parse a SKILL.md into its structure.
    #[tool(
        description = "Parse a skill's SKILL.md into frontmatter, headings, sections, and links."
    )]
    pub async fn read_skill(
        &self,
        params: Parameters<NameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let parsed = self.parse_named_skill(&params.0.name)?;
        json_result(&parsed, self.ctx.token_caps)
    }

    /// Search skills by metadata.
    #[tool(description = "Search skills by name, phase, or description.")]
    pub async fn search_skills(
        &self,
        params: Parameters<SearchSkillsArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;
        if args.query.trim().is_empty() {
            return Err(ErrorData::invalid_params(
                "search query must not be empty".to_string(),
                None,
            ));
        }
        let results = self.run_search(&args);
        json_result(
            &serde_json::json!({ "count": results.len(), "results": results }),
            self.ctx.token_caps,
        )
    }

    /// Build and persist the skill graph.
    #[tool(description = "Build the entity-relation skill graph and persist it to disk.")]
    pub async fn build_skill_graph(&self) -> Result<CallToolResult, ErrorData> {
        let parsed = self.parse_all_skills();
        let graph = graph::build_graph(&parsed);
        let response = json_result(
            &serde_json::json!({
                "entities": graph.entities.len(),
                "relations": graph.relations.len(),
                "graph_path": self.ctx.graph_path().display().to_string(),
            }),
            self.ctx.token_caps,
        )?;
        // Write the cache under `.agent/` through the single write guard (ADR-0008). The
        // guard rejects any target outside `.agent/` and writes atomically, so the graph
        // never lands in the crate source tree.
        let jsonl = graph::to_jsonl(&graph);
        agent_ws::write_under_agent(&self.ctx.repo_root, ServerContext::graph_rel_path(), &jsonl)
            .map_err(|e| {
            ErrorData::internal_error(format!("could not persist the skill graph: {e}"), None)
        })?;
        Ok(response)
    }

    /// Return the full skill graph.
    #[tool(description = "Return the persisted skill graph (entities and relations).")]
    pub async fn read_graph(&self) -> Result<CallToolResult, ErrorData> {
        let graph = self.load_persisted_graph()?;
        json_result(
            &serde_json::json!({
                "entities": graph.entities.values().collect::<Vec<_>>(),
                "relations": graph.relations,
            }),
            self.ctx.token_caps,
        )
    }

    /// Search graph entities.
    #[tool(description = "Search skill-graph entities by name, type, or observation text.")]
    pub async fn search_nodes(
        &self,
        params: Parameters<QueryArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let graph = self.load_persisted_graph()?;
        let results = graph::search_nodes(&graph, &params.0.query);
        json_result(
            &serde_json::json!({ "results": results }),
            self.ctx.token_caps,
        )
    }

    /// Open graph entities by name.
    #[tool(description = "Open specific skill-graph entities by name.")]
    pub async fn open_nodes(
        &self,
        params: Parameters<NamesArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let graph = self.load_persisted_graph()?;
        let entities = graph::open_nodes(&graph, &params.0.names);
        json_result(
            &serde_json::json!({ "entities": entities }),
            self.ctx.token_caps,
        )
    }

    /// Report a skill's dependencies and handoff chain.
    #[tool(
        description = "Report forward and reverse dependencies, handoff chain, and conventions."
    )]
    pub async fn get_dependencies(
        &self,
        params: Parameters<NameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let graph = self.load_persisted_graph()?;
        let name = &params.0.name;
        json_result(
            &serde_json::json!({
                "skill": name,
                "depends_on": graph::forward_deps(&graph, name),
                "depended_by": graph::reverse_deps(&graph, name),
                "handoff_chain": graph::handoff_chain(&graph, name),
                "conventions": graph::conventions(&graph, name),
            }),
            self.ctx.token_caps,
        )
    }

    /// Return git context scoped to the cockpit directories.
    #[tool(description = "Git status, log, or diff scoped to skills/ and specs/.")]
    pub async fn get_git_context(
        &self,
        params: Parameters<GitContextArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let action = parse_git_action(params.0.action.as_deref())?;
        let output = git::git_context(&self.ctx.repo_root, action).map_err(git_error)?;
        json_result(
            &serde_json::json!({ "action": action_name(action), "output": output }),
            self.ctx.token_caps,
        )
    }

    /// Validate a skill against the conventions.
    #[tool(description = "Check a skill's SKILL.md against the naming and structure conventions.")]
    pub async fn validate_skill(
        &self,
        params: Parameters<NameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let parsed = self.parse_named_skill(&params.0.name)?;
        let line_count = self.skill_line_count(&params.0.name)?;
        let report = validate_skill(&parsed, &self.ctx.repo_root, line_count);
        json_result(&report, self.ctx.token_caps)
    }
}

impl TrueNorthServer {
    /// Parse a skill by name, mapping resolution errors to MCP errors.
    fn parse_named_skill(&self, name: &str) -> Result<ParsedSkill, ErrorData> {
        let raw = read_skill_raw(&self.ctx.repo_root, name).map_err(skill_error)?;
        Ok(parse_skill(&raw))
    }

    /// Parse every discovered skill, skipping any that fail to read.
    fn parse_all_skills(&self) -> Vec<ParsedSkill> {
        discover_skills(&self.ctx.repo_root)
            .into_iter()
            .filter_map(|entry| read_skill_raw(&self.ctx.repo_root, &entry.name).ok())
            .map(|raw| parse_skill(&raw))
            .collect()
    }

    /// Load the persisted graph, erroring when none has been built.
    fn load_persisted_graph(&self) -> Result<SkillGraph, ErrorData> {
        let path = self.ctx.graph_path();
        if !path.is_file() {
            return Err(ErrorData::invalid_request(
                "the skill graph is unavailable; run build_skill_graph first".to_string(),
                None,
            ));
        }
        Ok(graph::load_graph(&path))
    }

    /// The line count of a skill's `SKILL.md`, for the size-cap check.
    fn skill_line_count(&self, name: &str) -> Result<usize, ErrorData> {
        let raw = read_skill_raw(&self.ctx.repo_root, name).map_err(skill_error)?;
        Ok(raw.markdown.lines().count())
    }

    /// Run the skill search over the discovered skills.
    fn run_search(&self, args: &SearchSkillsArgs) -> Vec<SearchResult> {
        let query = args.query.to_lowercase();
        let mut results: Vec<SearchResult> = Vec::new();

        for entry in discover_skills(&self.ctx.repo_root) {
            let Ok(raw) = read_skill_raw(&self.ctx.repo_root, &entry.name) else {
                continue;
            };
            let parsed = parse_skill(&raw);
            let fm = &parsed.frontmatter;
            let haystack = [
                entry.name.clone(),
                entry.phase.clone(),
                fm_str(fm, "description"),
            ]
            .join(" ")
            .to_lowercase();

            let matched = if args.exact {
                entry.name.to_lowercase() == query
            } else {
                haystack.contains(&query)
            };
            if !matched {
                continue;
            }
            let score = if args.exact {
                100
            } else {
                haystack.matches(&query).count()
            };
            results.push(SearchResult {
                name: entry.name,
                path: entry.path.display().to_string(),
                phase: entry.phase,
                score,
            });
        }

        results.sort_by_key(|r| std::cmp::Reverse(r.score));
        results
    }
}

/// Serialize skill index entries for the JSON result.
fn skills_json(skills: &[crate::engine::skill::SkillIndexEntry]) -> Vec<serde_json::Value> {
    skills
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "path": s.path.display().to_string(),
                "phase": s.phase,
            })
        })
        .collect()
}

/// A frontmatter field as a display string, or empty when absent or non-scalar.
fn fm_str(fm: &std::collections::BTreeMap<String, serde_yaml::Value>, key: &str) -> String {
    match fm.get(key) {
        Some(serde_yaml::Value::String(s)) => s.clone(),
        Some(serde_yaml::Value::Number(n)) => n.to_string(),
        Some(serde_yaml::Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// Parse the git action, defaulting to `status` and rejecting an unsupported value.
fn parse_git_action(action: Option<&str>) -> Result<GitAction, ErrorData> {
    match action.unwrap_or("status") {
        "status" => Ok(GitAction::Status),
        "log" => Ok(GitAction::Log),
        "diff" => Ok(GitAction::Diff),
        other => Err(ErrorData::invalid_params(
            format!("unsupported git action `{other}`. Use status, log, or diff."),
            None,
        )),
    }
}

/// The action name for the result payload.
fn action_name(action: GitAction) -> &'static str {
    match action {
        GitAction::Status => "status",
        GitAction::Log => "log",
        GitAction::Diff => "diff",
    }
}

/// Map a git error to an MCP error.
fn git_error(error: GitError) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `catalog`.
#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
