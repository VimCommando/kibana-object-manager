use crate::client::{ApiCapability, KibanaClient, KibanaVersion};
use crate::etl::{Extractor, Loader};
use crate::kibana::agents::{AgentsExtractor, AgentsLoader};
use crate::kibana::dependencies::{
    Dependency, find_agent_dependencies, find_skill_dependencies, find_tool_dependencies,
    find_workflow_dependencies,
};
use crate::kibana::saved_objects::{
    SavedObjectsExtractor, SavedObjectsLoader, SavedObjectsManifest,
};
use crate::kibana::skills::{SkillsExtractor, SkillsLoader};
use crate::kibana::spaces::{SpacesExtractor, SpacesLoader};
use crate::kibana::tools::{ToolsExtractor, ToolsLoader};
use crate::kibana::workflows::{WorkflowsExtractor, WorkflowsLoader};
use crate::{Error, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsupportedApiPolicy {
    Skip,
    Warn,
    Force,
}

#[derive(Clone, Debug)]
pub struct SyncSelection {
    pub spaces: Vec<String>,
    pub saved_objects: Option<SavedObjectsManifest>,
    pub include_spaces: bool,
    pub include_workflows: bool,
    pub include_agents: bool,
    pub include_tools: bool,
    pub include_skills: bool,
}

impl Default for SyncSelection {
    fn default() -> Self {
        Self {
            spaces: vec!["default".to_string()],
            saved_objects: None,
            include_spaces: false,
            include_workflows: false,
            include_agents: false,
            include_tools: false,
            include_skills: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyncOptions {
    pub expand_dependencies: bool,
    pub overwrite: bool,
    pub unsupported_api_policy: UnsupportedApiPolicy,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            expand_dependencies: true,
            overwrite: true,
            unsupported_api_policy: UnsupportedApiPolicy::Warn,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpaceBundle {
    pub saved_objects: Vec<Value>,
    pub workflows: Vec<Value>,
    pub agents: Vec<Value>,
    pub tools: Vec<Value>,
    pub skills: Vec<Value>,
}

#[derive(Clone, Debug, Default)]
pub struct SyncBundle {
    pub spaces: Vec<Value>,
    pub by_space: HashMap<String, SpaceBundle>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncSummary {
    pub spaces_attempted: usize,
    pub spaces_applied: usize,
    pub saved_objects_attempted: usize,
    pub saved_objects_applied: usize,
    pub workflows_attempted: usize,
    pub workflows_applied: usize,
    pub agents_attempted: usize,
    pub agents_applied: usize,
    pub tools_attempted: usize,
    pub tools_applied: usize,
    pub skills_attempted: usize,
    pub skills_applied: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityPlan {
    pub supported: Vec<ApiCapability>,
    pub unsupported: Vec<ApiCapabilityWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiCapabilityWarning {
    pub capability: ApiCapability,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyExpansionCapabilities {
    pub agents: bool,
    pub skills: bool,
    pub tools: bool,
    pub workflows: bool,
}

impl DependencyExpansionCapabilities {
    pub fn all() -> Self {
        Self {
            agents: true,
            skills: true,
            tools: true,
            workflows: true,
        }
    }
}

impl Default for DependencyExpansionCapabilities {
    fn default() -> Self {
        Self::all()
    }
}

pub fn plan_capabilities(
    version: &KibanaVersion,
    capabilities: impl IntoIterator<Item = ApiCapability>,
) -> CapabilityPlan {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for capability in capabilities {
        if KibanaClient::supports_capability(version, capability) {
            supported.push(capability);
        } else {
            unsupported.push(ApiCapabilityWarning {
                capability,
                message: KibanaClient::unsupported_capability_reason(version, capability),
            });
        }
    }

    CapabilityPlan {
        supported,
        unsupported,
    }
}

pub async fn pull_sync(
    client: &KibanaClient,
    selection: &SyncSelection,
    options: &SyncOptions,
) -> Result<SyncBundle> {
    let mut bundle = SyncBundle::default();
    let include_spaces = selection.include_spaces
        && capability_allowed(
            client,
            ApiCapability::Spaces,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_saved_objects = selection.saved_objects.is_some()
        && capability_allowed(
            client,
            ApiCapability::SavedObjects,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_workflows = selection.include_workflows
        && capability_allowed(
            client,
            ApiCapability::Workflows,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_agents = selection.include_agents
        && capability_allowed(
            client,
            ApiCapability::Agents,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_tools = selection.include_tools
        && capability_allowed(
            client,
            ApiCapability::Tools,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_skills = selection.include_skills
        && capability_allowed(
            client,
            ApiCapability::Skills,
            &options.unsupported_api_policy,
        )
        .await?;

    if include_spaces {
        bundle.spaces = SpacesExtractor::all(client.clone()).extract().await?;
    }

    for space_id in &selection.spaces {
        let space_client = client.space(space_id)?;
        let mut space_bundle = SpaceBundle::default();

        if include_saved_objects && let Some(manifest) = &selection.saved_objects {
            space_bundle.saved_objects =
                SavedObjectsExtractor::new(space_client.clone(), manifest.clone())
                    .extract()
                    .await?;
        }

        if include_workflows {
            space_bundle.workflows = WorkflowsExtractor::new(space_client.clone(), None)
                .search_workflows(None, None)
                .await?;
        }

        if include_agents {
            space_bundle.agents = AgentsExtractor::new(space_client.clone(), None)
                .search_agents(None)
                .await?;
        }

        if include_tools {
            space_bundle.tools = ToolsExtractor::new(space_client.clone(), None)
                .search_tools(None)
                .await?;
        }

        if include_skills {
            let extractor = SkillsExtractor::new(space_client, None);
            let skills = extractor.search_skills(false).await?;
            for skill in skills.iter().filter(|skill| !is_readonly(skill)) {
                if let Some(skill_id) = skill.get("id").and_then(|id| id.as_str()) {
                    space_bundle
                        .skills
                        .push(extractor.fetch_skill(skill_id).await?);
                }
            }
        }

        if options.expand_dependencies
            && (include_agents || include_skills || include_tools || include_workflows)
        {
            expand_dependencies(
                client,
                space_id,
                &mut space_bundle,
                DependencyExpansionCapabilities {
                    agents: include_agents,
                    skills: include_skills,
                    tools: include_tools,
                    workflows: include_workflows,
                },
            )
            .await?;
        }

        bundle.by_space.insert(space_id.clone(), space_bundle);
    }

    Ok(bundle)
}

pub async fn push_sync(
    client: &KibanaClient,
    bundle: &SyncBundle,
    options: &SyncOptions,
) -> Result<SyncSummary> {
    let mut summary = SyncSummary::default();
    let include_spaces = !bundle.spaces.is_empty()
        && capability_allowed(
            client,
            ApiCapability::Spaces,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_saved_objects = bundle
        .by_space
        .values()
        .any(|space_bundle| !space_bundle.saved_objects.is_empty())
        && capability_allowed(
            client,
            ApiCapability::SavedObjects,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_workflows = bundle
        .by_space
        .values()
        .any(|space_bundle| !space_bundle.workflows.is_empty())
        && capability_allowed(
            client,
            ApiCapability::Workflows,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_agents = bundle
        .by_space
        .values()
        .any(|space_bundle| !space_bundle.agents.is_empty())
        && capability_allowed(
            client,
            ApiCapability::Agents,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_tools = bundle
        .by_space
        .values()
        .any(|space_bundle| !space_bundle.tools.is_empty())
        && capability_allowed(
            client,
            ApiCapability::Tools,
            &options.unsupported_api_policy,
        )
        .await?;
    let include_skills = bundle
        .by_space
        .values()
        .any(|space_bundle| !space_bundle.skills.is_empty())
        && capability_allowed(
            client,
            ApiCapability::Skills,
            &options.unsupported_api_policy,
        )
        .await?;

    if include_spaces {
        summary.spaces_attempted = bundle.spaces.len();
        summary.spaces_applied = SpacesLoader::new(client.clone())
            .with_overwrite(options.overwrite)
            .load(bundle.spaces.clone())
            .await?;
    }

    for (space_id, space_bundle) in &bundle.by_space {
        let space_client = client.space(space_id)?;

        if include_saved_objects {
            summary.saved_objects_attempted += space_bundle.saved_objects.len();
            summary.saved_objects_applied += SavedObjectsLoader::new(space_client.clone())
                .with_overwrite(options.overwrite)
                .load(space_bundle.saved_objects.clone())
                .await?;
        }

        if include_tools {
            summary.tools_attempted += space_bundle.tools.len();
            summary.tools_applied += ToolsLoader::new(space_client.clone())
                .load(space_bundle.tools.clone())
                .await?;
        }

        if include_agents {
            summary.agents_attempted += space_bundle.agents.len();
            summary.agents_applied += AgentsLoader::new(space_client.clone())
                .load(space_bundle.agents.clone())
                .await?;
        }

        if include_skills {
            summary.skills_attempted += space_bundle.skills.len();
            summary.skills_applied += SkillsLoader::new(space_client.clone())
                .load(space_bundle.skills.clone())
                .await?;
        }

        if include_workflows {
            summary.workflows_attempted += space_bundle.workflows.len();
            summary.workflows_applied += WorkflowsLoader::new(space_client)
                .load(space_bundle.workflows.clone())
                .await?;
        }
    }

    Ok(summary)
}

async fn capability_allowed(
    client: &KibanaClient,
    capability: ApiCapability,
    policy: &UnsupportedApiPolicy,
) -> Result<bool> {
    if *policy == UnsupportedApiPolicy::Force {
        return Ok(true);
    }

    let version = client.server_version().await?;
    if KibanaClient::supports_capability(&version, capability) {
        return Ok(true);
    }

    let reason = KibanaClient::unsupported_capability_reason(&version, capability);
    match policy {
        UnsupportedApiPolicy::Skip => tracing::debug!("{reason}; skipping"),
        UnsupportedApiPolicy::Warn => tracing::warn!("{reason}; skipping"),
        UnsupportedApiPolicy::Force => {}
    }

    Ok(false)
}

pub async fn expand_dependencies(
    client: &KibanaClient,
    space_id: &str,
    bundle: &mut SpaceBundle,
    capabilities: DependencyExpansionCapabilities,
) -> Result<()> {
    let space_client = client.space(space_id)?;
    let mut existing_agents = ids(&bundle.agents);
    let mut existing_skills = ids(&bundle.skills);
    let mut existing_tools = ids(&bundle.tools);
    let mut existing_workflows = ids(&bundle.workflows);
    let mut processed = HashSet::new();
    let mut pending = Vec::new();

    for agent in &bundle.agents {
        pending.extend(find_agent_dependencies(agent));
    }
    for skill in &bundle.skills {
        pending.extend(find_skill_dependencies(skill));
    }
    for tool in &bundle.tools {
        pending.extend(find_tool_dependencies(tool));
    }
    for workflow in &bundle.workflows {
        pending.extend(find_workflow_dependencies(workflow));
    }

    while let Some(dependency) = pending.pop() {
        if !processed.insert(dependency.clone()) {
            continue;
        }

        match dependency {
            Dependency::Agent(id) if !existing_agents.contains(&id) && capabilities.agents => {
                let fetched =
                    fetch_dependency(&space_client, "api/agent_builder/agents", &id).await?;
                existing_agents.insert(id);
                pending.extend(find_agent_dependencies(&fetched));
                bundle.agents.push(fetched);
            }
            Dependency::Tool(id) if !existing_tools.contains(&id) && capabilities.tools => {
                let fetched =
                    fetch_dependency(&space_client, "api/agent_builder/tools", &id).await?;
                existing_tools.insert(id);
                pending.extend(find_tool_dependencies(&fetched));
                bundle.tools.push(fetched);
            }
            Dependency::Skill(id) if !existing_skills.contains(&id) && capabilities.skills => {
                let fetched =
                    fetch_dependency(&space_client, "api/agent_builder/skills", &id).await?;
                existing_skills.insert(id);
                pending.extend(find_skill_dependencies(&fetched));
                bundle.skills.push(fetched);
            }
            Dependency::Workflow(id)
                if !existing_workflows.contains(&id) && capabilities.workflows =>
            {
                let path = format!("api/workflows/{id}");
                let response = space_client.get_internal(&path).await?;
                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Err(Error::api_response(status, body));
                }
                let fetched = response.json().await?;
                existing_workflows.insert(id);
                pending.extend(find_workflow_dependencies(&fetched));
                bundle.workflows.push(fetched);
            }
            Dependency::Agent(id) if !capabilities.agents => {
                tracing::debug!("skipping dependent agent {id}; agent API is not enabled")
            }
            Dependency::Tool(id) if !capabilities.tools => {
                tracing::debug!("skipping dependent tool {id}; tool API is not enabled")
            }
            Dependency::Skill(id) if !capabilities.skills => {
                tracing::debug!("skipping dependent skill {id}; skill API is not enabled")
            }
            Dependency::Workflow(id) if !capabilities.workflows => {
                tracing::debug!("skipping dependent workflow {id}; workflow API is not enabled")
            }
            _ => {}
        }
    }

    Ok(())
}

fn is_readonly(value: &Value) -> bool {
    value.get("readonly").and_then(|value| value.as_bool()) == Some(true)
}

fn ids(values: &[Value]) -> HashSet<String> {
    values
        .iter()
        .filter_map(|value| value.get("id").and_then(|id| id.as_str()))
        .map(ToOwned::to_owned)
        .collect()
}

async fn fetch_dependency(client: &KibanaClient, prefix: &str, id: &str) -> Result<Value> {
    let path = format!("{prefix}/{id}");
    let response = client.get(&path).await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::api_response(status, body));
    }

    Ok(response.json().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;

    #[test]
    fn sync_bundle_is_storage_neutral() {
        let mut bundle = SyncBundle::default();
        bundle.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                saved_objects: vec![json!({"type": "dashboard", "id": "one"})],
                ..SpaceBundle::default()
            },
        );

        assert_eq!(bundle.by_space["default"].saved_objects.len(), 1);
    }

    #[test]
    fn capability_plan_reports_boundaries() {
        let version = crate::parse_kibana_version("9.2.0").unwrap();
        let plan = plan_capabilities(
            &version,
            [
                ApiCapability::Agents,
                ApiCapability::Tools,
                ApiCapability::Skills,
                ApiCapability::Workflows,
            ],
        );

        assert!(plan.supported.contains(&ApiCapability::Agents));
        assert!(plan.supported.contains(&ApiCapability::Tools));
        assert!(
            plan.unsupported
                .iter()
                .any(|warning| warning.capability == ApiCapability::Skills)
        );
        assert!(
            plan.unsupported
                .iter()
                .any(|warning| warning.capability == ApiCapability::Workflows)
        );
    }

    #[test]
    fn capability_plan_supports_skills_at_94() {
        let version = crate::parse_kibana_version("9.4.0").unwrap();
        let plan = plan_capabilities(&version, [ApiCapability::Skills]);

        assert_eq!(plan.supported, vec![ApiCapability::Skills]);
        assert!(plan.unsupported.is_empty());
    }

    #[tokio::test]
    async fn expands_agent_skill_tool_workflow_dependencies_transitively() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/agent_builder/skills/skill-s",
                status: 200,
                body: json!({
                    "id": "skill-s",
                    "name": "Skill S",
                    "tool_ids": ["tool-t"]
                }),
            },
            MockResponse {
                method: "GET",
                path: "/api/agent_builder/tools/tool-t",
                status: 200,
                body: json!({
                    "id": "tool-t",
                    "name": "Tool T",
                    "configuration": {
                        "workflow_id": "workflow-w"
                    }
                }),
            },
            MockResponse {
                method: "GET",
                path: "/api/workflows/workflow-w",
                status: 200,
                body: json!({
                    "id": "workflow-w",
                    "name": "Workflow W"
                }),
            },
        ]);
        let client = server.client().unwrap();
        let mut bundle = SpaceBundle {
            agents: vec![json!({
                "id": "agent-a",
                "name": "Agent A",
                "configuration": {
                    "skill_id": "skill-s"
                }
            })],
            ..SpaceBundle::default()
        };

        expand_dependencies(
            &client,
            "default",
            &mut bundle,
            DependencyExpansionCapabilities {
                agents: true,
                skills: true,
                tools: true,
                workflows: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(bundle.agents.len(), 1);
        assert_eq!(bundle.skills.len(), 1);
        assert_eq!(bundle.skills[0]["id"], "skill-s");
        assert_eq!(bundle.tools.len(), 1);
        assert_eq!(bundle.tools[0]["id"], "tool-t");
        assert_eq!(bundle.workflows.len(), 1);
        assert_eq!(bundle.workflows[0]["id"], "workflow-w");

        let paths = server
            .requests()
            .into_iter()
            .map(|request| request.path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/api/agent_builder/skills/skill-s",
                "/api/agent_builder/tools/tool-t",
                "/api/workflows/workflow-w"
            ]
        );
    }
}
