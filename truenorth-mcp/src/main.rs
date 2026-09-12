//! TrueNorth-MCP: an active, protocol-first MCP execution runtime.
//!
//! This entrypoint boots the server over the rmcp stdio transport and answers the MCP
//! `initialize` handshake. The tools, resources, and engine modules are scaffolded here
//! and implemented in later tasks (see `.kiro/specs/truenorth-mcp-refactor/tasks.md`).
//! The tool router and resource handlers attach to `TrueNorthServer` as those tasks
//! land, so the server capabilities grow in place.

mod config;
mod engine;
mod resources;
mod tools;

use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    transport::stdio,
};

/// The TrueNorth-MCP server.
///
/// It carries no state at the scaffold stage. Later tasks add the tool router, the
/// resource handlers, and the engine context to this type.
///
/// # Example
///
/// ```no_run
/// let server = TrueNorthServer::new();
/// // `main` serves `server` over stdio.
/// ```
#[derive(Clone, Default)]
struct TrueNorthServer;

impl TrueNorthServer {
    fn new() -> Self {
        Self
    }
}

impl ServerHandler for TrueNorthServer {
    fn get_info(&self) -> ServerInfo {
        // Report this crate's identity. `from_build_env()` reads the rmcp crate's build
        // env, so set name and version from this crate explicitly.
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

#[tokio::main]
async fn main() -> std::process::ExitCode {
    // Diagnostics go to stderr. Stdout is the MCP channel and must carry only protocol
    // traffic.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    // Resolve the governed repository root before serving. Termination here satisfies
    // Requirement 1.6: no valid root is a non-zero exit with an error indication.
    let repo_root = match config::get_repo_root() {
        Ok(root) => root,
        Err(error) => {
            tracing::error!(%error, "could not resolve the repository root");
            eprintln!("truenorth-mcp: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    tracing::info!(repo_root = %repo_root.display(), "resolved repository root");

    let server = TrueNorthServer::new();

    // Serve over stdio. `serve` fails when the transport cannot initialize.
    let running = match server.serve(stdio()).await {
        Ok(running) => running,
        Err(error) => {
            tracing::error!(%error, "failed to start the stdio transport");
            eprintln!(
                "truenorth-mcp: could not start the MCP stdio transport: {error}. \
                 Check that stdin and stdout are connected to an MCP client."
            );
            return std::process::ExitCode::FAILURE;
        }
    };

    // Run until the client disconnects or the transport closes.
    if let Err(error) = running.waiting().await {
        tracing::error!(%error, "server stopped with an error");
        eprintln!("truenorth-mcp: the server stopped with an error: {error}.");
        return std::process::ExitCode::FAILURE;
    }

    std::process::ExitCode::SUCCESS
}
