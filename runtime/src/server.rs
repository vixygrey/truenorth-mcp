//! The TrueNorth-MCP server type and its shared context.
//!
//! [`TrueNorthServer`] holds the tool router and the [`ServerContext`]. Each tools
//! submodule attaches a named `#[tool_router]` impl to this type, and the routers merge
//! in [`TrueNorthServer::from_context`]. The production constructor
//! [`TrueNorthServer::resolve`] resolves the per-project feature flags from disk.
//!
//! Design: Part II §1 (entrypoint), §2 (tools).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::ReadResourceResponse;
use rmcp::model::{
    ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult,
    Resource, ResourceContents,
};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerConfig},
};

use crate::engine::agent_ws::{LayoutCache, LayoutError};
use crate::engine::features::{self, Features, FeaturesError};
use crate::resources::{ResourceCache, ResourceReadError, served_from_uri, served_resources};

/// The shared context: the resolved repository root, the resource cache, the resolved
/// per-project feature flags, and the last-good layout contract.
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
    /// The last-good `.agent/` layout contract (Requirement 1.12).
    ///
    /// [`ServerContext::resolve`] validates the contract at startup. A later broken read
    /// leaves the cached contract intact, so the runtime keeps the last valid state.
    pub layout: LayoutCache,
}

impl ServerContext {
    /// Build a context, resolving the feature flags from disk (Requirement 1.2).
    ///
    /// Reads `.agent/config/rules.yml`. An absent file resolves to the default-enabled
    /// flags (Requirement 1.3). A present-but-broken file returns a typed error naming the
    /// path (Requirement 1.7, 1.8).
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
        let ctx = Self::with_features(repo_root, features);
        ctx.validate_layout();
        Ok(ctx)
    }

    /// Build a context from explicit feature flags, reading no disk.
    ///
    /// This is the dependency-injected constructor. A test builds a deterministic context,
    /// enabled or disabled, without writing a `rules.yml` to a temp directory. It does not
    /// validate the layout; [`ServerContext::resolve`] does that at startup.
    pub fn with_features(repo_root: PathBuf, features: Features) -> Self {
        Self {
            repo_root,
            resource_cache: ResourceCache::new(),
            features,
            layout: LayoutCache::new(),
        }
    }

    /// Validate the `.agent/` layout contract and cache the last valid result.
    ///
    /// The check is non-fatal (Requirement 1.12). An absent contract file means the repo
    /// has no `.agent/layout.yml` yet (a legacy `specs/` cockpit or an unscaffolded repo),
    /// so the server serves without a cached contract. A present-but-incomplete contract
    /// is logged; the server keeps serving and retains the last valid contract.
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
    ///
    /// Tests that need a disabled feature build the context with
    /// `ServerContext::with_features(root, Features { ontology: false })` and call
    /// [`TrueNorthServer::from_context`] directly.
    pub fn test_server(repo_root: PathBuf) -> Self {
        Self::from_context(ServerContext::with_features(repo_root, Features::default()))
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
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resources: Vec<Resource> = served_resources(self.ctx.features)
            .into_iter()
            .map(|doc| Resource::new(doc.uri(), doc.name()).with_mime_type(doc.mime_type()))
            .collect();
        Ok(ListResourcesResult::with_all_items(resources))
    }

    /// Read a resource's current on-disk content (Requirements 5.5, 5.7).
    ///
    /// A parse or validation failure returns an error naming the file and retains the
    /// last-good content in the cache, so other resources keep serving.
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
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
        Ok(ReadResourceResult::new(vec![contents]).into())
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
