//! The TrueNorth-MCP server type and its shared context.
//!
//! [`TrueNorthServer`] holds the tool router and the [`ServerContext`]. Each tools
//! submodule attaches a named `#[tool_router]` impl to this type, and the routers merge
//! in [`TrueNorthServer::from_context`]. The production constructor
//! [`TrueNorthServer::resolve`] resolves per-project features and token caps from disk.
//!
//! Design: Part II §1 (entrypoint), §2 (tools).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::ReadResourceResponse;
use rmcp::model::{
    CacheScope, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResult, Resource, ResourceContents,
};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerConfig},
};

use crate::engine::agent_ws::{LayoutCache, LayoutError};
use crate::engine::features::{self, Features, FeaturesError, TokenCaps};
use crate::resources::{ResourceCache, ResourceReadError, served_from_uri, served_resources};

/// The shared context: repository root, resource cache, feature flags, token caps,
/// and the last-good layout contract.
#[derive(Debug, Default)]
pub struct ServerContext {
    /// The governed repository root.
    pub repo_root: PathBuf,
    /// The last-good resource content cache (Requirement 5.7).
    pub resource_cache: ResourceCache,
    /// The resolved per-project feature flags (Requirement 1.2).
    ///
    /// Read by the ontology tool gate in [`TrueNorthServer::from_context`] and, from issue
    /// #142, the ontology resource gate.
    pub features: Features,
    /// The resolved token budgets for Lean skill rendering and tool responses.
    pub token_caps: TokenCaps,
    /// The last-good `.agent/` layout contract (Requirement 1.12).
    ///
    /// [`ServerContext::resolve`] validates the contract at startup. A later broken read
    /// leaves the cached contract intact, so the runtime keeps the last valid state.
    pub layout: LayoutCache,
}

impl ServerContext {
    /// Build a context, resolving feature flags and token caps from disk.
    ///
    /// Reads `.agent/config/rules.yml`. An absent file resolves to defaults.
    /// An invalid configured value returns a typed error naming the offending rule.
    ///
    /// # Errors
    ///
    /// Returns [`FeaturesError`] when a present `rules.yml` cannot be read or parsed.
    ///
    /// The layout-contract validation is non-fatal. An absent `.agent/layout.yml` (a legacy
    /// `specs/` cockpit or an unscaffolded repo) is skipped. A present-but-incomplete
    /// contract is logged as a warning and the server keeps serving, retaining the last
    /// valid contract (Requirement 1.12).
    pub fn resolve(repo_root: PathBuf) -> Result<Self, FeaturesError> {
        let features = features::resolve(&repo_root)?;
        let token_caps = features::resolve_token_caps(&repo_root)?;
        let ctx = Self::with_config(repo_root, features, token_caps);
        ctx.validate_layout();
        Ok(ctx)
    }

    /// Build a context from explicit feature flags and token caps, reading no disk.
    ///
    /// This is the dependency-injected constructor used by deterministic tests.
    pub fn with_config(repo_root: PathBuf, features: Features, token_caps: TokenCaps) -> Self {
        Self {
            repo_root,
            resource_cache: ResourceCache::new(),
            features,
            token_caps,
            layout: LayoutCache::new(),
        }
    }

    /// Build a context with default token caps for focused feature-gate tests.
    pub fn with_features(repo_root: PathBuf, features: Features) -> Self {
        Self::with_config(repo_root, features, TokenCaps::default())
    }

    /// Validate the `.agent/` layout contract and cache the last valid result.
    ///
    /// The check is non-fatal (Requirement 1.12). An absent `.agent/layout.yml` means the
    /// repo has no layout contract yet, so legacy and unscaffolded repositories still serve.
    /// A present but malformed, unsupported, or incomplete contract is logged; the server
    /// keeps serving and retains the last valid contract.
    fn validate_layout(&self) {
        match self.layout.read(&self.repo_root) {
            Ok(_) => {
                tracing::debug!("the .agent/ layout contract validated");
            }
            Err(LayoutError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                // No `.agent/layout.yml` present. The repo is a legacy cockpit or is not
                // scaffolded yet. Serve without a cached contract.
                tracing::debug!("no .agent/layout.yml contract present; skipping validation");
            }
            Err(error @ LayoutError::Io { .. }) => {
                // A present contract file that could not be read (permissions, for example).
                tracing::warn!(%error, "the .agent/layout.yml contract could not be read");
            }
            Err(error @ LayoutError::Parse { .. })
            | Err(error @ LayoutError::UnsupportedVersion { .. }) => {
                tracing::warn!(%error, "the .agent/layout.yml contract is invalid");
            }
            Err(error @ LayoutError::MissingEntry { .. }) => {
                tracing::warn!(%error, "the .agent/ layout contract is incomplete");
            }
        }
    }

    /// The persisted skill-graph cache path, `<repo_root>/.agent/tasks/skill-graph.jsonl`.
    ///
    /// The graph is a regenerable cache, rebuilt on demand by `build_skill_graph`, so it
    /// lives under `.agent/` and not in the crate source tree (ADR-0008). The write goes
    /// through the `.agent/` write guard; see `build_skill_graph`.
    pub fn graph_path(&self) -> PathBuf {
        self.repo_root
            .join(".agent")
            .join("tasks")
            .join("skill-graph.jsonl")
    }

    /// The graph cache path relative to `.agent/`, for the write guard.
    ///
    /// `write_under_agent` takes a path relative to `.agent/`, so this returns
    /// `tasks/skill-graph.jsonl` to match `graph_path`.
    pub fn graph_rel_path() -> &'static Path {
        Path::new("tasks/skill-graph.jsonl")
    }
}

/// The TrueNorth-MCP server.
///
/// It carries the shared context and the merged tool router. The `tool_router` field is
/// read by the `#[tool_handler]` macro.
#[derive(Clone)]
pub struct TrueNorthServer {
    /// The shared context, shared cheaply across tool calls.
    pub ctx: Arc<ServerContext>,
    /// The merged tool router. Read by the `#[tool_handler]` macro.
    pub tool_router: ToolRouter<Self>,
}

impl TrueNorthServer {
    /// Build the server for a repository root, resolving the feature flags from disk.
    ///
    /// # Errors
    ///
    /// Returns [`FeaturesError`] when a present `.agent/config/rules.yml` cannot be read or
    /// parsed (Requirement 1.7, 1.8).
    pub fn resolve(repo_root: PathBuf) -> Result<Self, FeaturesError> {
        Ok(Self::from_context(ServerContext::resolve(repo_root)?))
    }

    /// Assemble the server from a built context.
    ///
    /// The tool router is the merge of every tools submodule's named router. The ontology
    /// router merges only when the ontology feature is enabled (Requirement 2.1). When the
    /// feature is disabled, neither ontology tool is advertised or callable (Requirement
    /// 2.2), and every other router is unchanged (Requirement 2.3).
    pub(crate) fn from_context(ctx: ServerContext) -> Self {
        let mut tool_router = Self::skills_router()
            + Self::catalog_router()
            + Self::lifecycle_router()
            + Self::gates_router()
            + Self::tdd_router()
            + Self::bugref_router()
            + Self::scaffold_router();

        if ctx.features.ontology {
            tool_router += Self::ontology_router();
        }

        // The guardrail tool is advertised only when the jev feature is on, matching the
        // ontology gate (jev-active-guardrail R1.6, R8.2). Every other router is unchanged.
        if ctx.features.jev {
            tool_router += Self::guard_router();
        }

        Self {
            ctx: Arc::new(ctx),
            tool_router,
        }
    }

    /// The server config reported at the MCP `initialize` handshake.
    pub fn server_info() -> ServerConfig {
        let mut implementation = Implementation::from_build_env();
        implementation.name = env!("CARGO_PKG_NAME").to_string();
        implementation.version = env!("CARGO_PKG_VERSION").to_string();

        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(implementation)
        .with_protocol_version(ProtocolVersion::LATEST)
        .with_instructions(
            "TrueNorth-MCP: spec-driven engineering discipline as active MCP tools \
                 and resources."
                .to_string(),
        )
    }
}

#[cfg(test)]
impl TrueNorthServer {
    /// A default-enabled server for tests, built from an injected context and no disk read.
    /// Tests use the same token-cap defaults as a project with no rules file.
    pub fn test_server(repo_root: PathBuf) -> Self {
        Self::from_context(ServerContext::with_config(
            repo_root,
            Features::default(),
            TokenCaps::default(),
        ))
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl ServerHandler for TrueNorthServer {
    fn get_info(&self) -> ServerConfig {
        Self::server_info()
    }

    /// List the five served resources: the cockpit set and the read-only ADR resource
    /// (Requirements 5.1, 9.2).
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resources: Vec<Resource> = served_resources(self.ctx.features)
            .into_iter()
            .map(|doc| Resource::new(doc.uri(), doc.name()).with_mime_type(doc.mime_type()))
            .collect();
        let result = ListResourcesResult::with_all_items(resources);
        Ok(with_resource_cache_hints(result, &context))
    }

    /// List the resource templates. TrueNorth currently serves only fixed resource URIs.
    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let result = ListResourceTemplatesResult::default();
        Ok(with_resource_template_cache_hints(result, &context))
    }

    /// Read a resource's current on-disk content (Requirements 5.5, 5.7).
    ///
    /// A parse or validation failure returns an error naming the file and retains the
    /// last-good content in the cache, so other resources keep serving.
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        // Resolve against the flag-filtered set. A disabled ontology URI is unknown here,
        // so it never reaches read_current and no ontology file is seeded (Requirement 3.3,
        // 3.4).
        let doc = served_from_uri(&request.uri, self.ctx.features).ok_or_else(|| {
            ErrorData::invalid_params(format!("unknown resource: {}", request.uri), None)
        })?;

        let content = self
            .ctx
            .resource_cache
            .read(doc, &self.ctx.repo_root)
            .map_err(resource_error)?;

        let contents = ResourceContents::text(content, doc.uri()).with_mime_type(doc.mime_type());
        let result = ReadResourceResult::new(vec![contents]);
        Ok(with_read_resource_cache_hints(result, &context).into())
    }
}

fn supports_resource_cache_hints(context: &RequestContext<RoleServer>) -> bool {
    context
        .protocol_version()
        .is_some_and(|version| version.as_str() >= ProtocolVersion::V_2026_07_28.as_str())
}

fn with_resource_cache_hints(
    result: ListResourcesResult,
    context: &RequestContext<RoleServer>,
) -> ListResourcesResult {
    if supports_resource_cache_hints(context) {
        result.with_ttl_ms(0).with_cache_scope(CacheScope::Private)
    } else {
        result
    }
}

fn with_resource_template_cache_hints(
    result: ListResourceTemplatesResult,
    context: &RequestContext<RoleServer>,
) -> ListResourceTemplatesResult {
    if supports_resource_cache_hints(context) {
        result.with_ttl_ms(0).with_cache_scope(CacheScope::Private)
    } else {
        result
    }
}

fn with_read_resource_cache_hints(
    result: ReadResourceResult,
    context: &RequestContext<RoleServer>,
) -> ReadResourceResult {
    if supports_resource_cache_hints(context) {
        result.with_ttl_ms(0).with_cache_scope(CacheScope::Private)
    } else {
        result
    }
}

/// Map a resource read error to an MCP error (Requirement 5.7).
fn resource_error(error: ResourceReadError) -> ErrorData {
    match error {
        ResourceReadError::NotFound(file) => {
            ErrorData::invalid_request(format!("{file} not found"), None)
        }
        ResourceReadError::Invalid(detail) => ErrorData::invalid_request(detail, None),
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `server`.
#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
