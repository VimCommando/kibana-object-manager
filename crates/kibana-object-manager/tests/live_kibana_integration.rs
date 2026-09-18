//! Live Kibana integration tests.
//!
//! These tests are ignored by default and require a running Kibana instance.
//! Use `scripts/live-kibana-tests.sh test` to start the containerized stack and
//! run this suite with the expected environment variables.

mod common;

use common::live_kibana::{LiveKibana, test_space_id};
use eyre::{Result, bail};
use kibana_object_manager::{
    cli::{StandaloneExportSelection, export_standalone_resources, import_standalone_resources},
    client::{ApiCapability, KibanaClient, KibanaVersion},
    etl::{Extractor, Loader},
    kibana::{
        agents::{AgentsExtractor, AgentsLoader},
        saved_objects::{
            SavedObject, SavedObjectsExtractor, SavedObjectsLoader, SavedObjectsManifest,
        },
        skills::{
            SkillsExtractor, SkillsLoader, skill_directory_name, skill_to_directory, skill_to_value,
        },
        spaces::SpacesExtractor,
        tools::{ToolsExtractor, ToolsLoader},
        workflows::{WorkflowsExtractor, WorkflowsLoader, workflow_resource_path_for_version},
    },
    standalone::{ImportPlan, ResourceFamily},
};
use reqwest::{Method, StatusCode};
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
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
        if let Some(tool_ids) = object
            .get_mut("tool_ids")
            .and_then(|value| value.as_array_mut())
        {
            tool_ids.truncate(5);
        }

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

#[derive(Debug)]
enum LiveFamilyStatus {
    Passed(String),
    Skipped(String),
}

#[tokio::test]
#[ignore]
#[serial(live_kibana)]
async fn live_standalone_import_export_roundtrips() -> Result<()> {
    let space_id = test_space_id("standalone");
    let live = LiveKibana::new(std::slice::from_ref(&space_id)).await?;
    live.ensure_space(&space_id).await?;
    live.configure_standalone_environment();

    let mut failures = Vec::new();
    for family in [
        ResourceFamily::Skills,
        ResourceFamily::Tools,
        ResourceFamily::Agents,
        ResourceFamily::Workflows,
    ] {
        match run_standalone_family_case(&live, &space_id, family).await {
            Ok(LiveFamilyStatus::Passed(version)) => {
                eprintln!("standalone live {}: passed (Kibana {version})", family);
            }
            Ok(LiveFamilyStatus::Skipped(reason)) => {
                eprintln!("standalone live {}: skipped ({reason})", family);
            }
            Err(error) => {
                eprintln!("standalone live {}: failed ({error:#})", family);
                failures.push(format!("{family}: {error:#}"));
            }
        }
    }

    if let Err(error) = live.delete_space(&space_id).await {
        eprintln!("Best-effort standalone live space cleanup failed: {error:#}");
    }
    if !failures.is_empty() {
        bail!(
            "standalone live family failures:\n- {}",
            failures.join("\n- ")
        );
    }
    Ok(())
}

async fn run_standalone_family_case(
    live: &LiveKibana,
    space_id: &str,
    family: ResourceFamily,
) -> Result<LiveFamilyStatus> {
    let version = live.client.server_version().await?;
    let capability = family_capability(family);
    if !KibanaClient::supports_capability(&version, capability) {
        return Ok(LiveFamilyStatus::Skipped(format!(
            "Kibana {version}; {} requires {}",
            family,
            capability.minimum_version()
        )));
    }

    let space_client = live.client.space(space_id)?;
    let probe = match family {
        ResourceFamily::Skills => {
            SkillsExtractor::new(space_client.clone(), None)
                .search_skills(false)
                .await
        }
        ResourceFamily::Tools => {
            ToolsExtractor::new(space_client.clone(), None)
                .search_tools(None)
                .await
        }
        ResourceFamily::Agents => {
            AgentsExtractor::new(space_client.clone(), None)
                .search_agents(None)
                .await
        }
        ResourceFamily::Workflows => {
            WorkflowsExtractor::new(space_client.clone(), None)
                .search_workflows(None, Some(1))
                .await
        }
    };
    if let Err(error) = probe {
        if let Some(reason) = explicit_license_unavailable(&error) {
            return Ok(LiveFamilyStatus::Skipped(reason));
        }
        return Err(error.into());
    }

    let id = match family {
        ResourceFamily::Workflows => workflow_live_id(&live.run_id),
        _ => format!("kibob-live-{}-{}", live.run_id, family),
    };
    let value = live_resource_value(family, &id);
    let created = match family {
        ResourceFamily::Skills => {
            SkillsLoader::new(space_client.clone())
                .load_report(vec![value])
                .await
        }
        ResourceFamily::Tools => {
            ToolsLoader::new(space_client.clone())
                .load_report(vec![value])
                .await
        }
        ResourceFamily::Agents => {
            AgentsLoader::new(space_client.clone())
                .load_report(vec![value])
                .await
        }
        ResourceFamily::Workflows => {
            WorkflowsLoader::new(space_client.clone())
                .load_report(vec![value])
                .await
        }
    };
    if created.has_failures() {
        bail!(
            "{} create failed: {}",
            family,
            created.outcomes()[0].detail().unwrap_or("unknown failure")
        );
    }

    let result = async {
        let temp = TempDir::new()?;
        let exported = export_standalone_resources(
            family,
            temp.path(),
            StandaloneExportSelection::Ids(vec![id.clone()]),
            Some(space_id),
            false,
            false,
        )
        .await?;
        if exported.has_failures() {
            bail!("{} export returned failed item outcomes", family);
        }

        let artifact = ImportPlan::discover(family, temp.path())?.validate()?;
        if artifact.resources().len() != 1 || artifact.resources()[0].id() != id {
            bail!("{} exported artifact did not preserve ID {}", family, id);
        }

        let imported =
            import_standalone_resources(family, temp.path(), Some(space_id), false).await?;
        if imported.has_failures() {
            bail!(
                "{} import failed: {}",
                family,
                imported.outcomes()[0].detail().unwrap_or("unknown failure")
            );
        }

        let fetched = fetch_live_resource(space_client.clone(), family, &id).await?;
        if fetched.get("id").and_then(|value| value.as_str()) != Some(&id) {
            bail!("{} remote verification did not preserve ID {}", family, id);
        }
        Ok::<(), eyre::Report>(())
    }
    .await;

    if let Err(error) = delete_live_resource(&space_client, family, &id, &version).await {
        eprintln!("Best-effort cleanup failed for {family} {id}: {error:#}");
    }
    result?;
    Ok(LiveFamilyStatus::Passed(version.to_string()))
}

fn family_capability(family: ResourceFamily) -> ApiCapability {
    match family {
        ResourceFamily::Skills => ApiCapability::Skills,
        ResourceFamily::Tools => ApiCapability::Tools,
        ResourceFamily::Agents => ApiCapability::Agents,
        ResourceFamily::Workflows => ApiCapability::Workflows,
    }
}

fn live_resource_value(family: ResourceFamily, id: &str) -> serde_json::Value {
    match family {
        ResourceFamily::Skills => json!({
            "id": id,
            "name": format!("Kibob Live {id}"),
            "description": "Standalone live test Skill",
            "content": "Return a concise live-test response.\n",
            "tool_ids": [],
            "referenced_content": []
        }),
        ResourceFamily::Tools => json!({
            "id": id,
            "description": "Standalone live test Tool",
            "type": "esql",
            "tags": [],
            "configuration": {
                "query": "FROM logs-* | LIMIT 1",
                "params": {}
            }
        }),
        ResourceFamily::Agents => json!({
            "id": id,
            "name": format!("Kibob Live {id}"),
            "description": "Standalone live test Agent",
            "configuration": {
                "instructions": "Respond only for the standalone live test.",
                "tools": []
            }
        }),
        ResourceFamily::Workflows => json!({
            "id": id,
            "name": format!("Kibob Live {id}"),
            "description": "Standalone live test Workflow",
            "enabled": false,
            "yaml": format!(
                "version: '1'\nname: Kibob Live {id}\nenabled: false\ntriggers: []\nsteps:\n  - name: Log live test\n    type: console\n    with:\n      message: Standalone live test\n"
            )
        }),
    }
}

fn workflow_live_id(run_id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    run_id.hash(&mut hasher);
    let hash = hasher.finish();
    format!(
        "workflow-{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        hash as u32,
        (hash >> 32) & 0xffff,
        (hash >> 20) & 0x0fff,
        (hash >> 8) & 0x0fff,
        hash & 0xffffffffffff
    )
}

async fn fetch_live_resource(
    client: KibanaClient,
    family: ResourceFamily,
    id: &str,
) -> kibana_sync::Result<serde_json::Value> {
    match family {
        ResourceFamily::Skills => SkillsExtractor::new(client, None).fetch_skill(id).await,
        ResourceFamily::Tools => ToolsExtractor::new(client, None).fetch_tool(id).await,
        ResourceFamily::Agents => AgentsExtractor::new(client, None).fetch_agent(id).await,
        ResourceFamily::Workflows => {
            WorkflowsExtractor::new(client, None)
                .fetch_workflow(id)
                .await
        }
    }
}

async fn delete_live_resource(
    client: &KibanaClient,
    family: ResourceFamily,
    id: &str,
    version: &KibanaVersion,
) -> Result<()> {
    let (path, headers) = match family {
        ResourceFamily::Skills => (
            format!("api/agent_builder/skills/{id}?force=true"),
            HashMap::new(),
        ),
        ResourceFamily::Tools => (format!("api/agent_builder/tools/{id}"), HashMap::new()),
        ResourceFamily::Agents => (format!("api/agent_builder/agents/{id}"), HashMap::new()),
        ResourceFamily::Workflows => {
            let mut headers = HashMap::new();
            headers.insert(
                "X-Elastic-Internal-Origin".to_string(),
                "Kibana".to_string(),
            );
            (workflow_resource_path_for_version(version, id), headers)
        }
    };
    let response = client
        .request(Method::DELETE, &headers, &path, None)
        .await?;
    if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
        return Ok(());
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    bail!("{status}: {body}")
}

fn explicit_license_unavailable(error: &kibana_sync::Error) -> Option<String> {
    let kibana_sync::Error::ApiResponse { status, body } = error else {
        return None;
    };
    let body_lower = body.to_ascii_lowercase();
    let explicitly_license_related = [
        "license",
        "licence",
        "subscription",
        "platinum",
        "enterprise",
    ]
    .iter()
    .any(|marker| body_lower.contains(marker));
    if (*status == StatusCode::FORBIDDEN || *status == StatusCode::PAYMENT_REQUIRED)
        && explicitly_license_related
    {
        Some(format!("{status}: {body}"))
    } else {
        None
    }
}

#[test]
fn live_skip_classifier_requires_an_explicit_license_response() {
    let licensed = kibana_sync::Error::api_response(
        StatusCode::FORBIDDEN,
        r#"{"message":"feature requires an Enterprise license"}"#,
    );
    assert!(explicit_license_unavailable(&licensed).is_some());

    for error in [
        kibana_sync::Error::api_response(StatusCode::UNAUTHORIZED, "authentication required"),
        kibana_sync::Error::api_response(StatusCode::FORBIDDEN, "forbidden"),
        kibana_sync::Error::api_response(StatusCode::NOT_FOUND, "route not found"),
    ] {
        assert!(explicit_license_unavailable(&error).is_none());
    }
}

#[test]
fn workflow_live_ids_use_the_server_required_uuid_shape() {
    let id = workflow_live_id("1234-5678");
    let uuid = id.strip_prefix("workflow-").unwrap();
    let groups = uuid.split('-').collect::<Vec<_>>();

    assert_eq!(
        groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
        vec![8, 4, 4, 4, 12]
    );
    assert!(
        groups
            .iter()
            .all(|group| { group.chars().all(|character| character.is_ascii_hexdigit()) })
    );
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

    referenced.sort_by_cached_key(|left| left.to_string());
    referenced
}

fn optional_api_unavailable(error: &kibana_sync::Error) -> bool {
    explicit_license_unavailable(error).is_some()
}
