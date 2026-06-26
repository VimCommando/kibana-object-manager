//! Live Kibana integration tests.
//!
//! These tests are ignored by default and require a running Kibana instance.
//! Use `scripts/live-kibana-tests.sh test` to start the containerized stack and
//! run this suite with the expected environment variables.

mod common;

use common::live_kibana::{LiveKibana, test_space_id};
use eyre::Result;
use kibana_object_manager::{
    client::{ApiCapability, KibanaClient},
    etl::{Extractor, Loader},
    kibana::{
        agents::AgentsExtractor,
        saved_objects::{
            SavedObject, SavedObjectsExtractor, SavedObjectsLoader, SavedObjectsManifest,
        },
        spaces::SpacesExtractor,
        tools::ToolsExtractor,
        workflows::WorkflowsExtractor,
    },
};
use serde_json::json;
use serial_test::serial;

#[tokio::test]
#[ignore]
#[serial(live_kibana)]
async fn live_saved_objects_roundtrip_in_owned_space() -> Result<()> {
    let space_id = test_space_id("saved-objects");
    let live = LiveKibana::new(std::slice::from_ref(&space_id)).await?;
    live.ensure_space(&space_id).await?;

    let result = async {
        let space_client = live.client.space(&space_id)?;
        let object_id = format!("kibob-live-data-view-{}", live.run_id);
        let objects = vec![json!({
            "type": "index-pattern",
            "id": object_id,
            "attributes": {
                "title": format!("kibob-live-{}-*", live.run_id),
                "timeFieldName": "@timestamp"
            }
        })];

        let imported = SavedObjectsLoader::new(space_client.clone())
            .with_overwrite(true)
            .load(objects)
            .await?;
        assert_eq!(imported, 1);

        let mut manifest = SavedObjectsManifest::new();
        manifest.add_object(SavedObject::new("index-pattern", object_id));
        let exported = SavedObjectsExtractor::new(space_client, manifest)
            .extract()
            .await?;

        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0]["type"], "index-pattern");
        assert!(exported[0].get("attributes").is_some());
        Ok::<(), eyre::Report>(())
    }
    .await;

    let cleanup = live.delete_space(&space_id).await;
    result?;
    cleanup?;
    Ok(())
}

#[tokio::test]
#[ignore]
#[serial(live_kibana)]
async fn live_spaces_create_fetch_and_delete() -> Result<()> {
    let space_id = test_space_id("spaces");
    let live = LiveKibana::new(std::slice::from_ref(&space_id)).await?;
    live.ensure_space(&space_id).await?;

    let result = async {
        let extractor = SpacesExtractor::all(live.client.clone());
        let fetched = extractor.fetch_space(&space_id).await?;
        assert_eq!(fetched["id"], space_id);
        assert_eq!(fetched["name"], space_id);
        Ok::<(), eyre::Report>(())
    }
    .await;

    let cleanup = live.delete_space(&space_id).await;
    result?;
    cleanup?;
    Ok(())
}

#[tokio::test]
#[ignore]
#[serial(live_kibana)]
async fn live_supported_api_smoke_tests() -> Result<()> {
    let space_id = test_space_id("api-smoke");
    let live = LiveKibana::new(std::slice::from_ref(&space_id)).await?;
    live.ensure_space(&space_id).await?;

    let result = async {
        let version = live.client.server_version().await?;
        let space_client = live.client.space(&space_id)?;

        if KibanaClient::supports_capability(&version, ApiCapability::Agents) {
            match AgentsExtractor::new(space_client.clone(), None)
                .search_agents(None)
                .await
            {
                Ok(agents) => assert!(agents.iter().all(|agent| agent.is_object())),
                Err(e) if optional_api_unavailable(&e) => {
                    eprintln!("Skipping live agents smoke: {e}");
                }
                Err(e) => return Err(e),
            }
        }

        if KibanaClient::supports_capability(&version, ApiCapability::Tools) {
            match ToolsExtractor::new(space_client.clone(), None)
                .search_tools(None)
                .await
            {
                Ok(tools) => assert!(tools.iter().all(|tool| tool.is_object())),
                Err(e) if optional_api_unavailable(&e) => {
                    eprintln!("Skipping live tools smoke: {e}");
                }
                Err(e) => return Err(e),
            }
        }

        if KibanaClient::supports_capability(&version, ApiCapability::Workflows) {
            match WorkflowsExtractor::new(space_client, None)
                .search_workflows(None, Some(10))
                .await
            {
                Ok(workflows) => assert!(workflows.iter().all(|workflow| workflow.is_object())),
                Err(e) if optional_api_unavailable(&e) => {
                    eprintln!("Skipping live workflows smoke: {e}");
                }
                Err(e) => return Err(e),
            }
        }

        Ok::<(), eyre::Report>(())
    }
    .await;

    let cleanup = live.delete_space(&space_id).await;
    result?;
    cleanup?;
    Ok(())
}

fn optional_api_unavailable(error: &eyre::Report) -> bool {
    if std::env::var("KIBANA_TEST_STRICT_OPTIONAL_APIS").as_deref() == Ok("1") {
        return false;
    }

    let message = error.to_string();
    message.contains("403 Forbidden") || message.contains("404 Not Found")
}
