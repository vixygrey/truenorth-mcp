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
mod server;
mod tools;

#[cfg(test)]
mod integration_tests;

use rmcp::model::ResourceUpdatedNotificationParam;
use rmcp::{ServiceExt, transport::stdio};

use crate::engine::watcher::{ResourceUri, spawn_watcher};
use crate::server::TrueNorthServer;

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

    // Build the server, resolving the per-project feature flags from
    // `.agent/config/rules.yml`. A present-but-broken config is a non-zero exit with the
    // named path (Requirement 1.7, 1.8).
    let server = match TrueNorthServer::resolve(repo_root.clone()) {
        Ok(server) => server,
        Err(error) => {
            tracing::error!(%error, "could not resolve the feature config");
            eprintln!("truenorth-mcp: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };

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

    // Spawn the file watcher. A debounced disk change to a cockpit file emits
    // `resources/updated` for the affected resource (Requirements 5.3, 14.2). The watcher
    // callback runs on a worker thread, so it bridges to the async peer through the tokio
    // runtime handle. The `WatchHandle` lives until the server stops.
    let peer = running.peer().clone();
    let runtime = tokio::runtime::Handle::current();
    let watch_handle = match spawn_watcher(&repo_root, move |uris| {
        emit_resource_updates(&runtime, &peer, uris);
    }) {
        Ok(handle) => Some(handle),
        Err(error) => {
            // A watcher failure is not fatal: the server still serves tools and reads.
            // Notifications will not fire until the next start.
            tracing::warn!(%error, "the file watcher did not start; resource notifications are off");
            None
        }
    };

    // Run until the client disconnects or the transport closes.
    let outcome = running.waiting().await;
    drop(watch_handle);
    if let Err(error) = outcome {
        tracing::error!(%error, "server stopped with an error");
        eprintln!("truenorth-mcp: the server stopped with an error: {error}.");
        return std::process::ExitCode::FAILURE;
    }

    std::process::ExitCode::SUCCESS
}

/// Emit a `resources/updated` notification for each affected resource URI.
///
/// The watcher callback is synchronous and runs on a worker thread, so this spawns the
/// async notify onto the tokio runtime. A send failure is logged, not fatal, because the
/// notification is advisory.
fn emit_resource_updates(
    runtime: &tokio::runtime::Handle,
    peer: &rmcp::service::Peer<rmcp::RoleServer>,
    uris: Vec<ResourceUri>,
) {
    for uri in uris {
        let peer = peer.clone();
        runtime.spawn(async move {
            let param = ResourceUpdatedNotificationParam::new(uri.as_str());
            if let Err(error) = peer.notify_resource_updated(param).await {
                tracing::debug!(%error, uri = uri.as_str(), "resources/updated was not delivered");
            }
        });
    }
}
