//! The TrueNorth-MCP server type and its shared context.
//!
//! [`TrueNorthServer`] holds the tool router and the [`ServerContext`]. Each tools
//! submodule attaches a named `#[tool_router]` impl to this type, and the routers merge
//! in [`TrueNorthServer::from_context`]. The production constructor
//! [`TrueNorthServer::resolve`] resolves the per-project feature flags from disk.
//!
//! Design: Part II §1 (entrypoint), §2 (tools).

use std::path::PathBuf;
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
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
};

use crate::engine::features::{self, Features, FeaturesError};
use crate::resources::{ALL_RESOURCES, ResourceCache, ResourceDoc, ResourceReadError};

/// The shared context: the resolved repository root, the resource cache, and the resolved
/// per-project feature flags.
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
    pub fn resolve(repo_root: PathBuf) -> Result<Self, FeaturesError> {
        let features = features::resolve(&repo_root)?;
        Ok(Self::with_features(repo_root, features))
    }

    /// Build a context from explicit feature flags, reading no disk.
    ///
    /// This is the dependency-injected constructor. A test builds a deterministic context,
    /// enabled or disabled, without writing a `rules.yml` to a temp directory.
    pub fn with_features(repo_root: PathBuf, features: Features) -> Self {
        Self {
            repo_root,
            resource_cache: ResourceCache::new(),
            features,
        }
    }

    /// The persisted skill-graph path, `<repo_root>/truenorth-mcp/graph.jsonl`.
    pub fn graph_path(&self) -> PathBuf {
        self.repo_root.join("truenorth-mcp").join("graph.jsonl")
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
    fn from_context(ctx: ServerContext) -> Self {
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

        Self {
            ctx: Arc::new(ctx),
            tool_router,
        }
    }

    /// The server info reported at the MCP `initialize` handshake.
    pub fn server_info() -> ServerInfo {
        let mut implementation = Implementation::from_build_env();
        implementation.name = env!("CARGO_PKG_NAME").to_string();
        implementation.version = env!("CARGO_PKG_VERSION").to_string();

        ServerInfo::new(
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
    fn get_info(&self) -> ServerInfo {
        Self::server_info()
    }

    /// List the five served resources: the cockpit set and the read-only ADR resource
    /// (Requirements 5.1, 9.2).
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resources: Vec<Resource> = ALL_RESOURCES
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
        let doc = ResourceDoc::from_uri(&request.uri).ok_or_else(|| {
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
