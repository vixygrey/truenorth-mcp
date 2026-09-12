//! The TrueNorth-MCP server type and its shared context.
//!
//! [`TrueNorthServer`] holds the tool router and the [`ServerContext`]. Each tools
//! submodule attaches a named `#[tool_router]` impl to this type, and the routers merge
//! in [`TrueNorthServer::new`]. The full aggregation of tools and resources lands in a
//! later task; this establishes the shared type and the first tool router.
//!
//! Design: Part II §1 (entrypoint), §2 (tools).

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::{
    ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
};

/// The shared context passed to every tool: the resolved repository root and the paths
/// derived from it.
#[derive(Debug, Clone)]
pub struct ServerContext {
    /// The governed repository root.
    pub repo_root: PathBuf,
}

impl ServerContext {
    /// Build a context rooted at `repo_root`.
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
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
    /// Build the server for a repository root.
    ///
    /// The tool router is the merge of every tools submodule's named router. Task 8a
    /// wires the skills router; later tasks add the rest.
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            ctx: Arc::new(ServerContext::new(repo_root)),
            tool_router: Self::skills_router() + Self::catalog_router(),
        }
    }

    /// The server info reported at the MCP `initialize` handshake.
    pub fn server_info() -> ServerInfo {
        let mut implementation = Implementation::from_build_env();
        implementation.name = env!("CARGO_PKG_NAME").to_string();
        implementation.version = env!("CARGO_PKG_VERSION").to_string();

        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(implementation)
            .with_protocol_version(ProtocolVersion::LATEST)
            .with_instructions(
                "TrueNorth-MCP: spec-driven engineering discipline as active MCP tools \
                 and resources."
                    .to_string(),
            )
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl ServerHandler for TrueNorthServer {
    fn get_info(&self) -> ServerInfo {
        Self::server_info()
    }
}
