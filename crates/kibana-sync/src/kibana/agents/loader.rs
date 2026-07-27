//! Agents API loader
//!
//! Loads agent definitions to Kibana via POST/PUT /api/agent_builder/agents

use crate::client::KibanaClient;
use crate::etl::Loader;
use crate::standalone::{
    ResourceBatchReport, ResourceFamily, ResourceOperation, ResourceOutcome, ResourceOutcomeStatus,
    server_resource_is_readonly,
};

use crate::{Error, Result};
use serde_json::Value;
use tokio::task::JoinSet;

/// Loader for Kibana agents
///
/// Creates or updates agents in Kibana using POST (create) and PUT (update)
///
/// # Example
/// ```no_run
/// use kibana_sync::kibana::agents::AgentsLoader;
/// use kibana_sync::client::{Auth, KibanaClient};
/// use kibana_sync::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_sync::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
/// let space_client = client.space("default")?;
/// let loader = AgentsLoader::new(space_client);
///
/// let agents = vec![
///     json!({
///         "id": "agent-123",
///         "name": "my-agent",
///         "description": "Example agent"
///     })
/// ];
///
/// let count = loader.load(agents).await?;
/// # Ok(())
/// # }
/// ```
pub struct AgentsLoader {
    client: KibanaClient,
}

impl AgentsLoader {
    /// Create a new agents loader
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    pub fn new(client: KibanaClient) -> Self {
        Self { client }
    }

    /// Apply Agents while preserving a structured outcome for every input item.
    pub async fn load_report(&self, items: Vec<Value>) -> ResourceBatchReport {
        let mut set = JoinSet::new();
        let mut outcomes = items
            .iter()
            .map(|agent| {
                ResourceOutcome::failed(
                    ResourceFamily::Agents,
                    resource_id(agent),
                    None,
                    "loader task did not complete",
                )
            })
            .collect::<Vec<_>>();

        for (index, agent) in items.into_iter().enumerate() {
            let client = self.client.clone();
            set.spawn(async move { (index, upsert_agent(client, agent).await) });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok((index, outcome)) => outcomes[index] = outcome,
                Err(error) => tracing::error!("Agent loader task panicked: {error}"),
            }
        }

        ResourceBatchReport::new(outcomes)
    }
}

impl Loader for AgentsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let report = self.load_report(items).await;
        for outcome in report.outcomes() {
            if outcome.status() == ResourceOutcomeStatus::Failed {
                tracing::error!(
                    "Failed to load agent '{}': {}",
                    outcome.id(),
                    outcome.detail().unwrap_or("unknown failure")
                );
            }
        }

        Ok(report.counts().applied)
    }
}

async fn upsert_agent(client: KibanaClient, agent: Value) -> ResourceOutcome {
    let Some(agent_id) = agent
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return ResourceOutcome::failed(
            ResourceFamily::Agents,
            "<missing>",
            None,
            Error::MissingResourceId { resource: "agent" }.to_string(),
        );
    };

    let agent_name = agent
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    if agent.get("readonly").and_then(Value::as_bool) == Some(true) {
        tracing::debug!("Skipping readonly agent: {}", agent_id);
        return ResourceOutcome::skipped(
            ResourceFamily::Agents,
            agent_id,
            "local Agent is readonly",
        );
    }

    let path = format!("api/agent_builder/agents/{agent_id}");
    let exists = match client.head(&path).await {
        Ok(response) => match response.status().as_u16() {
            200 => true,
            404 => false,
            status => {
                return ResourceOutcome::failed(
                    ResourceFamily::Agents,
                    agent_id,
                    None,
                    format!("Failed to check agent existence: {status}"),
                );
            }
        },
        Err(error) => {
            return ResourceOutcome::failed(
                ResourceFamily::Agents,
                agent_id,
                None,
                error.to_string(),
            );
        }
    };

    if exists {
        match server_resource_is_readonly(&client, &path, false, "Agent").await {
            Ok(true) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Agents,
                    agent_id,
                    None,
                    "server-side Agent is readonly",
                );
            }
            Ok(false) => {}
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Agents,
                    agent_id,
                    None,
                    error.to_string(),
                );
            }
        }
    }

    let mut body = agent;
    if let Some(object) = body.as_object_mut() {
        object.remove("readonly");
        object.remove("schema");
        object.remove("type");
        object.remove("created_by");
        object.remove("updated_by");
        object.remove("created_at");
        object.remove("updated_at");
        if exists {
            object.remove("id");
        }
    }

    if exists {
        let response = match client.put_json_value(&path, &body).await {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Agents,
                    agent_id,
                    Some(ResourceOperation::Update),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Agents,
                agent_id,
                Some(ResourceOperation::Update),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Updated agent: {} (id: {})", agent_name, agent_id);
        ResourceOutcome::applied(ResourceFamily::Agents, agent_id, ResourceOperation::Update)
    } else {
        let response = match client
            .post_json_value("api/agent_builder/agents", &body)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Agents,
                    agent_id,
                    Some(ResourceOperation::Create),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Agents,
                agent_id,
                Some(ResourceOperation::Create),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Created agent: {} (id: {})", agent_name, agent_id);
        ResourceOutcome::applied(ResourceFamily::Agents, agent_id, ResourceOperation::Create)
    }
}

fn resource_id(resource: &Value) -> &str {
    resource
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<missing>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let _loader = AgentsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = AgentsLoader::new(space_client);

        let agent = json!({"name": "No ID"});

        // items needs to be a vector for loader.load
        let result = loader.load(vec![agent]).await;

        // In the concurrent version, it might not return Err immediately if it fails in task
        // but it should log error. Actually it should return count < 1.
        // Wait, if it returns Err inside the task, it will log it and return count = 0.
        // Let's check how the old test worked.
        // It called loader.upsert_agent directly which returned Result.
        // Now upsert_agent is gone.
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn reports_create_with_required_headers_and_space_path() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "HEAD",
                path: "/s/esdiag/api/agent_builder/agents/agent-a",
                status: 404,
                body: json!({}),
            },
            MockResponse {
                method: "POST",
                path: "/s/esdiag/api/agent_builder/agents",
                status: 200,
                body: json!({"id": "agent-a"}),
            },
        ]);
        let loader = AgentsLoader::new(server.client().unwrap().space("esdiag").unwrap());

        let report = loader
            .load_report(vec![json!({
                "id": "agent-a",
                "name": "Agent A",
                "readonly": false
            })])
            .await;

        assert_eq!(report.counts().applied, 1);
        assert_eq!(
            report.outcomes()[0].operation(),
            Some(ResourceOperation::Create)
        );
        let requests = server.requests();
        assert_eq!(
            requests[1].headers.get("kbn-xsrf").map(String::as_str),
            Some("true")
        );
        let body: Value = serde_json::from_str(&requests[1].body).unwrap();
        assert_eq!(body["id"], "agent-a");
        assert!(body.get("readonly").is_none());
    }

    #[tokio::test]
    async fn reports_update_and_removes_server_fields() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "HEAD",
                path: "/api/agent_builder/agents/agent-a",
                status: 200,
                body: json!({}),
            },
            MockResponse {
                method: "GET",
                path: "/api/agent_builder/agents/agent-a",
                status: 200,
                body: json!({"id": "agent-a", "readonly": false}),
            },
            MockResponse {
                method: "PUT",
                path: "/api/agent_builder/agents/agent-a",
                status: 200,
                body: json!({"id": "agent-a"}),
            },
        ]);
        let loader = AgentsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({
                "id": "agent-a",
                "name": "Agent A",
                "readonly": false,
                "schema": "system",
                "type": "conversational",
                "created_by": "elastic",
                "updated_by": "elastic",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-02T00:00:00Z"
            })])
            .await;

        assert_eq!(
            report.outcomes()[0].operation(),
            Some(ResourceOperation::Update)
        );
        let body: Value = serde_json::from_str(&server.requests()[2].body).unwrap();
        assert!(body.get("id").is_none());
        assert!(body.get("readonly").is_none());
        assert!(body.get("schema").is_none());
        assert!(body.get("type").is_none());
        assert!(body.get("created_by").is_none());
        assert!(body.get("updated_by").is_none());
        assert!(body.get("created_at").is_none());
        assert!(body.get("updated_at").is_none());
    }

    #[tokio::test]
    async fn fails_existing_server_side_readonly_agent_without_mutation() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "HEAD",
                path: "/api/agent_builder/agents/system-agent",
                status: 200,
                body: json!({}),
            },
            MockResponse {
                method: "GET",
                path: "/api/agent_builder/agents/system-agent",
                status: 200,
                body: json!({"id": "system-agent", "readonly": true}),
            },
        ]);
        let loader = AgentsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({"id": "system-agent", "name": "System Agent"})])
            .await;

        assert_eq!(report.counts().failed, 1);
        assert_eq!(report.outcomes()[0].status(), ResourceOutcomeStatus::Failed);
        assert_eq!(
            report.outcomes()[0].detail(),
            Some("server-side Agent is readonly")
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "HEAD");
        assert_eq!(requests[1].method, "GET");
    }

    #[tokio::test]
    async fn skips_local_readonly_agent_without_requests() {
        let server = TestServer::new(Vec::new());
        let loader = AgentsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({"id": "system-agent", "readonly": true})])
            .await;

        assert_eq!(report.counts().skipped, 1);
        assert!(server.requests().is_empty());
    }
}
