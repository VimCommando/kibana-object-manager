//! Tools API loader
//!
//! Loads tool definitions to Kibana via POST/PUT /api/agent_builder/tools

use crate::client::KibanaClient;
use crate::etl::Loader;
use crate::standalone::{
    ResourceBatchReport, ResourceFamily, ResourceOperation, ResourceOutcome, ResourceOutcomeStatus,
};

use crate::{Error, Result};
use serde_json::Value;
use tokio::task::JoinSet;

/// Loader for Kibana tools
///
/// Creates or updates tools in Kibana using POST (create) and PUT (update)
///
/// # Example
/// ```no_run
/// use kibana_sync::kibana::tools::ToolsLoader;
/// use kibana_sync::client::{Auth, KibanaClient};
/// use kibana_sync::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_sync::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
/// let space_client = client.space("default")?;
/// let loader = ToolsLoader::new(space_client);
///
/// let tools = vec![
///     json!({
///         "id": "tool-123",
///         "name": "my-tool",
///         "description": "Example tool"
///     })
/// ];
///
/// let count = loader.load(tools).await?;
/// # Ok(())
/// # }
/// ```
pub struct ToolsLoader {
    client: KibanaClient,
}

impl ToolsLoader {
    /// Create a new tools loader
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    pub fn new(client: KibanaClient) -> Self {
        Self { client }
    }

    /// Apply Tools while preserving a structured outcome for every input item.
    pub async fn load_report(&self, items: Vec<Value>) -> ResourceBatchReport {
        let mut set = JoinSet::new();
        let mut outcomes = items
            .iter()
            .map(|tool| {
                ResourceOutcome::failed(
                    ResourceFamily::Tools,
                    resource_id(tool),
                    None,
                    "loader task did not complete",
                )
            })
            .collect::<Vec<_>>();

        for (index, tool) in items.into_iter().enumerate() {
            let client = self.client.clone();
            set.spawn(async move { (index, upsert_tool(client, tool).await) });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok((index, outcome)) => outcomes[index] = outcome,
                Err(error) => tracing::error!("Tool loader task panicked: {error}"),
            }
        }

        ResourceBatchReport::new(outcomes)
    }
}

impl Loader for ToolsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let report = self.load_report(items).await;
        for outcome in report.outcomes() {
            if outcome.status() == ResourceOutcomeStatus::Failed {
                tracing::error!(
                    "Failed to load tool '{}': {}",
                    outcome.id(),
                    outcome.detail().unwrap_or("unknown failure")
                );
            }
        }

        Ok(report.counts().applied)
    }
}

async fn upsert_tool(client: KibanaClient, tool: Value) -> ResourceOutcome {
    let Some(tool_id) = tool
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return ResourceOutcome::failed(
            ResourceFamily::Tools,
            "<missing>",
            None,
            Error::MissingResourceId { resource: "tool" }.to_string(),
        );
    };

    if tool.get("readonly").and_then(Value::as_bool) == Some(true) {
        tracing::debug!("Skipping readonly tool: {}", tool_id);
        return ResourceOutcome::skipped(ResourceFamily::Tools, tool_id, "local Tool is readonly");
    }

    let path = format!("api/agent_builder/tools/{tool_id}");
    let exists = match client.head(&path).await {
        Ok(response) => match response.status().as_u16() {
            200 => true,
            404 => false,
            status => {
                return ResourceOutcome::failed(
                    ResourceFamily::Tools,
                    tool_id,
                    None,
                    format!("Failed to check tool existence: {status}"),
                );
            }
        },
        Err(error) => {
            return ResourceOutcome::failed(
                ResourceFamily::Tools,
                tool_id,
                None,
                error.to_string(),
            );
        }
    };

    if exists {
        let mut body = tool;
        if let Some(object) = body.as_object_mut() {
            object.remove("id");
            object.remove("readonly");
            object.remove("schema");
            object.remove("type");
        }
        let response = match client.put_json_value(&path, &body).await {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Tools,
                    tool_id,
                    Some(ResourceOperation::Update),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Tools,
                tool_id,
                Some(ResourceOperation::Update),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Updated tool: {}", tool_id);
        ResourceOutcome::applied(ResourceFamily::Tools, tool_id, ResourceOperation::Update)
    } else {
        let mut body = tool;
        if let Some(object) = body.as_object_mut() {
            object.remove("readonly");
            object.remove("schema");
        }
        let response = match client
            .post_json_value("api/agent_builder/tools", &body)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Tools,
                    tool_id,
                    Some(ResourceOperation::Create),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Tools,
                tool_id,
                Some(ResourceOperation::Create),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Created tool: {}", tool_id);
        ResourceOutcome::applied(ResourceFamily::Tools, tool_id, ResourceOperation::Create)
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
        let _loader = ToolsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = ToolsLoader::new(space_client);

        let tool = json!({"description": "No ID"});

        let result = loader.load(vec![tool]).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn reports_create_with_required_headers_and_space_path() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "HEAD",
                path: "/s/esdiag/api/agent_builder/tools/tool-a",
                status: 404,
                body: json!({}),
            },
            MockResponse {
                method: "POST",
                path: "/s/esdiag/api/agent_builder/tools",
                status: 200,
                body: json!({"id": "tool-a"}),
            },
        ]);
        let loader = ToolsLoader::new(server.client().unwrap().space("esdiag").unwrap());

        let report = loader
            .load_report(vec![json!({
                "id": "tool-a",
                "name": "Tool A",
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
        assert_eq!(body["id"], "tool-a");
        assert!(body.get("readonly").is_none());
    }

    #[tokio::test]
    async fn reports_update_and_removes_server_fields() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "HEAD",
                path: "/api/agent_builder/tools/tool-a",
                status: 200,
                body: json!({}),
            },
            MockResponse {
                method: "PUT",
                path: "/api/agent_builder/tools/tool-a",
                status: 200,
                body: json!({"id": "tool-a"}),
            },
        ]);
        let loader = ToolsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({
                "id": "tool-a",
                "name": "Tool A",
                "readonly": false,
                "schema": "system",
                "type": "index_search"
            })])
            .await;

        assert_eq!(
            report.outcomes()[0].operation(),
            Some(ResourceOperation::Update)
        );
        let body: Value = serde_json::from_str(&server.requests()[1].body).unwrap();
        assert!(body.get("id").is_none());
        assert!(body.get("readonly").is_none());
        assert!(body.get("schema").is_none());
        assert!(body.get("type").is_none());
    }

    #[tokio::test]
    async fn skips_local_readonly_tool_without_requests() {
        let server = TestServer::new(Vec::new());
        let loader = ToolsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({"id": "system-tool", "readonly": true})])
            .await;

        assert_eq!(report.counts().skipped, 1);
        assert!(server.requests().is_empty());
    }
}
