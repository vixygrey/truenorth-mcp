//! Protocol-version regression tests for resource cache metadata (#424).

use std::fs;

use rmcp::model::{
    CacheScope, ClientConfig, InitializeResult, ListResourceTemplatesResult, ListResourcesResult,
    ProtocolVersion, ReadResourceRequestParams, ReadResourceResult, ResourceContents, ResultType,
};
use rmcp::service::serve_directly;
use rmcp::{ClientHandler, RoleClient, RoleServer};
use tempfile::tempdir;

use crate::server::TrueNorthServer;

#[derive(Debug, Clone)]
struct VersionedClient {
    protocol_version: ProtocolVersion,
}

impl ClientHandler for VersionedClient {
    fn get_info(&self) -> ClientConfig {
        let mut info = ClientConfig::default();
        info.protocol_version = self.protocol_version.clone();
        info
    }
}

async fn resource_results(
    protocol_version: ProtocolVersion,
) -> anyhow::Result<(
    ListResourcesResult,
    ListResourceTemplatesResult,
    ReadResourceResult,
)> {
    let repo = tempdir()?;
    let tasks = repo.path().join(".agent/tasks");
    fs::create_dir_all(&tasks)?;
    fs::write(
        tasks.join("state.yml"),
        concat!(
            "active_flow: null\n",
            "active_group: null\n",
            "active_task: null\n",
            "phase: null\n",
            "tdd:\n",
            "  step: refactor\n",
            "handoff:\n",
            "  next_skill: null\n",
            "  context: null\n",
            "  group: null\n",
            "git:\n",
            "  branch: main\n",
        ),
    )?;

    let (server_transport, client_transport) = tokio::io::duplex(8192);
    let client_handler = VersionedClient {
        protocol_version: protocol_version.clone(),
    };
    let mut server_peer_info = InitializeResult::default();
    server_peer_info.protocol_version = protocol_version;

    let server = serve_directly::<RoleServer, _, _, _, _>(
        TrueNorthServer::test_server(repo.path().to_path_buf()),
        server_transport,
        Some(client_handler.get_info()),
    );
    let server_handle = tokio::spawn(async move {
        server.waiting().await?;
        anyhow::Ok(())
    });
    let client = serve_directly::<RoleClient, _, _, _, _>(
        client_handler,
        client_transport,
        Some(server_peer_info.into()),
    );

    let resources = client.list_resources(None).await?;
    let templates = client.list_resource_templates(None).await?;
    let read = client
        .read_resource(ReadResourceRequestParams::new("truenorth://state"))
        .await?;

    client.cancel().await?;
    server_handle.await??;
    Ok((resources, templates, read))
}

fn assert_resource_payloads(
    resources: &ListResourcesResult,
    templates: &ListResourceTemplatesResult,
    read: &ReadResourceResult,
) {
    let uris: Vec<&str> = resources
        .resources
        .iter()
        .map(|resource| resource.uri.as_str())
        .collect();
    assert_eq!(
        uris,
        [
            "truenorth://state",
            "truenorth://cockpit",
            "truenorth://conventions",
            "truenorth://ontology",
            "truenorth://adr",
        ]
    );
    assert!(templates.resource_templates.is_empty());
    let Some(ResourceContents::TextResourceContents { uri, text, .. }) = read.contents.first()
    else {
        panic!("state resource must return one text content block");
    };
    assert_eq!(uri, "truenorth://state");
    assert!(text.contains("phase: null"));
}

#[tokio::test]
async fn resource_cache_metadata_is_emitted_for_modern_protocol() -> anyhow::Result<()> {
    let (resources, templates, read) = resource_results(ProtocolVersion::V_2026_07_28).await?;
    assert_resource_payloads(&resources, &templates, &read);
    let expected = (
        Some(ResultType::COMPLETE),
        Some(0),
        Some(CacheScope::Private),
    );

    assert_eq!(
        (
            resources.result_type,
            resources.ttl_ms,
            resources.cache_scope
        ),
        expected
    );
    assert_eq!(
        (
            templates.result_type,
            templates.ttl_ms,
            templates.cache_scope
        ),
        expected
    );
    assert_eq!((read.result_type, read.ttl_ms, read.cache_scope), expected);
    Ok(())
}

#[tokio::test]
async fn resource_cache_metadata_is_omitted_for_legacy_protocol() -> anyhow::Result<()> {
    let (resources, templates, read) = resource_results(ProtocolVersion::V_2025_11_25).await?;
    assert_resource_payloads(&resources, &templates, &read);
    let expected = (None, None, None);

    assert_eq!(
        (
            resources.result_type,
            resources.ttl_ms,
            resources.cache_scope
        ),
        expected
    );
    assert_eq!(
        (
            templates.result_type,
            templates.ttl_ms,
            templates.cache_scope
        ),
        expected
    );
    assert_eq!((read.result_type, read.ttl_ms, read.cache_scope), expected);
    Ok(())
}
