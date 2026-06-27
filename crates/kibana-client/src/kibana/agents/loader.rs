//! Agents API loader
//!
//! Loads agent definitions to Kibana via POST/PUT /api/agent_builder/agents

use crate::client::KibanaClient;
use crate::etl::Loader;

use crate::{Error, Result};
use serde_json::Value;
use tokio::task::JoinSet;

/// Loader for Kibana agents
///
/// Creates or updates agents in Kibana using POST (create) and PUT (update)
///
/// # Example
/// ```no_run
/// use kibana_client::kibana::agents::AgentsLoader;
/// use kibana_client::client::{Auth, KibanaClient};
/// use kibana_client::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_client::Result<()> {
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
}

impl Loader for AgentsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;
        let mut set = JoinSet::new();

        for agent in items {
            let client = self.client.clone();

            set.spawn(async move {
                let agent_id = agent
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or(Error::MissingResourceId { resource: "agent" })?;

                let agent_name = agent
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Skip readonly agents
                if agent
                    .get("readonly")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    tracing::debug!("Skipping readonly agent: {}", agent_id);
                    return Ok::<bool, Error>(false);
                }

                // Check existence
                let path = format!("api/agent_builder/agents/{}", agent_id);
                let exists = match client.head(&path).await?.status().as_u16() {
                    200 => true,
                    404 => false,
                    status => {
                        return Err(Error::message(format!(
                            "Failed to check agent existence ({agent_id}): {status}"
                        )));
                    }
                };

                if exists {
                    // Update
                    let mut agent_body = agent.clone();
                    if let Some(obj) = agent_body.as_object_mut() {
                        obj.remove("id");
                        obj.remove("readonly");
                        obj.remove("schema");
                        obj.remove("type");
                    }
                    let path = format!("api/agent_builder/agents/{}", agent_id);
                    let response = client.put_json_value(&path, &agent_body).await?;
                    if !response.status().is_success() {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        return Err(Error::api_response(status, body));
                    }
                    tracing::info!("Updated agent: {} (id: {})", agent_name, agent_id);
                } else {
                    // Create
                    let mut agent_body = agent.clone();
                    if let Some(obj) = agent_body.as_object_mut() {
                        obj.remove("readonly");
                        obj.remove("schema");
                        obj.remove("type");
                    }
                    let path = "api/agent_builder/agents";
                    let response = client.post_json_value(path, &agent_body).await?;
                    if !response.status().is_success() {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        return Err(Error::api_response(status, body));
                    }
                    tracing::info!("Created agent: {} (id: {})", agent_name, agent_id);
                }

                Ok::<bool, Error>(true)
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(loaded)) => {
                    if loaded {
                        count += 1;
                    }
                }
                Ok(Err(e)) => tracing::error!("Failed to load agent: {}", e),
                Err(e) => tracing::error!("Task panicked: {}", e),
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
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
}
