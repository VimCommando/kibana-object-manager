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
        skills::{
            SkillsExtractor, SkillsLoader, skill_directory_name, skill_to_directory, skill_to_value,
        },
        spaces::SpacesExtractor,
        tools::ToolsExtractor,
        workflows::WorkflowsExtractor,
    },
};
use serde_json::json;
use serial_test::serial;
use tempfile::TempDir;

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
        Ok::<(), kibana_sync::Error>(())
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
        Ok::<(), kibana_sync::Error>(())
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

        if KibanaClient::supports_capability(&version, ApiCapability::Skills) {
            match SkillsExtractor::new(space_client.clone(), None)
                .search_skills(false)
                .await
            {
                Ok(skills) => assert!(skills.iter().all(|skill| skill.is_object())),
                Err(e) if optional_api_unavailable(&e) => {
                    eprintln!("Skipping live skills smoke: {e}");
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

        Ok::<(), kibana_sync::Error>(())
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
async fn live_skills_threat_hunting_referenced_content_roundtrip() -> Result<()> {
    let space_id = "esdiag".to_string();
    let live = LiveKibana::new(std::slice::from_ref(&space_id)).await?;

    let version = live.client.server_version().await?;
    if !KibanaClient::supports_capability(&version, ApiCapability::Skills) {
        eprintln!("Skipping live skills roundtrip: Skills require Kibana 9.4.0+");
        return Ok(());
    }

    let space_client = live.client.space(&space_id)?;
    let extractor = SkillsExtractor::new(space_client.clone(), None);
    let loader = SkillsLoader::new(space_client.clone());
    let source_skill_id =
        std::env::var("KIBANA_TEST_SOURCE_SKILL_ID").unwrap_or_else(|_| "threat-hunting".into());
    let test_skill_id = format!("kibob-live-{}-{}-copy", live.run_id, source_skill_id);
    let test_skill_name = format!("Kibob Live {} {} Copy", live.run_id, source_skill_id);
    let mut created = false;

    let result = async {
        let source = extractor.fetch_skill(&source_skill_id).await?;
        let expected_referenced_content = normalized_referenced_content(&source);
        assert!(
            !expected_referenced_content.is_empty(),
            "{source_skill_id} should include referenced_content"
        );

        let temp = TempDir::new().map_err(kibana_sync::Error::from)?;
        let mut copy = source.clone();
        let object = copy.as_object_mut().ok_or_else(|| {
            kibana_sync::Error::message("source Skill response was not a JSON object")
        })?;
        object.insert("id".to_string(), json!(test_skill_id));
        object.insert("name".to_string(), json!(test_skill_name));

        skill_to_directory(temp.path(), &copy)?;
        let skill_dir = temp.path().join(skill_directory_name(&copy)?);
        let mut projected = skill_to_value(&skill_dir, true)?;
        if let Some(object) = projected.as_object_mut() {
            object.remove("experimental");
        }
        assert!(projected.get("readonly").is_none());

        let response = space_client
            .post_json_value("api/agent_builder/skills", &projected)
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(kibana_sync::Error::api_response(status, body));
        }
        created = true;

        let fetched = extractor.fetch_skill(&test_skill_id).await?;
        assert_eq!(fetched["id"], test_skill_id);
        assert_eq!(
            normalized_referenced_content(&fetched),
            expected_referenced_content
        );

        Ok::<(), kibana_sync::Error>(())
    }
    .await;

    if created && let Err(err) = loader.delete_skill(&test_skill_id, true).await {
        eprintln!("Best-effort cleanup failed for skill {test_skill_id}: {err}");
    }

    result?;
    Ok(())
}

fn normalized_referenced_content(skill: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut referenced = skill
        .get("referenced_content")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    json!({
                        "name": item.get("name").cloned().unwrap_or(serde_json::Value::Null),
                        "relativePath": item
                            .get("relativePath")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                        "content": item.get("content").cloned().unwrap_or(serde_json::Value::Null)
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    referenced.sort_by_key(|left| left.to_string());
    referenced
}

fn optional_api_unavailable(error: &kibana_sync::Error) -> bool {
    if std::env::var("KIBANA_TEST_STRICT_OPTIONAL_APIS").as_deref() == Ok("1") {
        return false;
    }

    let message = error.to_string();
    message.contains("403 Forbidden") || message.contains("404 Not Found")
}
