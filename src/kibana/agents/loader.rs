//! Agents API loader
//!
//! Loads agent definitions to Kibana via POST/PUT /api/agent_builder/agents

use crate::client::KibanaClient;
use crate::etl::Loader;

use eyre::Result;
use owo_colors::OwoColorize;
use serde_json::Value;

/// Loader for Kibana agents
///
/// Creates or updates agents in Kibana using POST (create) and PUT (update)
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::agents::AgentsLoader;
/// use kibana_object_manager::client::{Auth, KibanaClient};
/// use kibana_object_manager::etl::Loader;
/// use serde_json::json;
/// use url::Url;
/// use std::path::Path;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::try_new(url, Auth::None, Path::new("."))?;
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

    /// Check if an agent exists by ID using HEAD request
    ///
    /// Returns true if the agent exists, false if it returns 404
    async fn agent_exists(&self, agent_id: &str) -> Result<bool> {
        let path = format!("api/agent_builder/agents/{}", agent_id);

        log::debug!("{} {}", "HEAD".green(), path);

        let response = self.client.head(&path).await?;

        match response.status().as_u16() {
            200 => {
                log::debug!(
                    "{} {} - agent exists, will update",
                    "200".green(),
                    agent_id.cyan()
                );
                Ok(true)
            }
            404 => {
                log::debug!(
                    "{} {} - agent not found, will create",
                    "404".yellow(),
                    agent_id.cyan()
                );
                Ok(false)
            }
            _ => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                eyre::bail!(
                    "Failed to check agent {} existence ({}): {}",
                    agent_id.cyan(),
                    status,
                    body
                )
            }
        }
    }

    /// Create a new agent using POST /api/agent_builder/agents/
    async fn create_agent(&self, agent: &Value) -> Result<()> {
        let agent_id = agent
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Agent missing 'id' field"))?;

        let agent_name = agent
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let path = "api/agent_builder/agents/";

        log::debug!("{} {}", "POST".green(), path);

        let response = self.client.post_json_value(path, agent).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to create agent {} (id: {}) ({}): {}",
                agent_name.cyan(),
                agent_id.cyan(),
                status,
                body
            );
        }

        log::info!(
            "Created agent: {} (id: {})",
            agent_name.cyan(),
            agent_id.cyan()
        );

        Ok(())
    }

    /// Update an existing agent using PUT /api/agent_builder/agents/<id>
    async fn update_agent(&self, agent_id: &str, agent: &Value) -> Result<()> {
        let agent_name = agent
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let path = format!("api/agent_builder/agents/{}", agent_id);

        log::debug!("{} {}", "PUT".green(), path);

        let response = self.client.put_json_value(&path, agent).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to update agent {} (id: {}) ({}): {}",
                agent_name.cyan(),
                agent_id.cyan(),
                status,
                body
            );
        }

        log::info!(
            "Updated agent: {} (id: {})",
            agent_name.cyan(),
            agent_id.cyan()
        );

        Ok(())
    }

    /// Create or update a single agent
    ///
    /// Checks if the agent exists first to determine whether to use
    /// POST (create) or PUT (update)
    ///
    /// Skips readonly agents as they cannot be modified
    async fn upsert_agent(&self, agent: &Value) -> Result<()> {
        let agent_id = agent
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Agent missing 'id' field"))?;

        // Skip readonly agents (builtin agents that can't be modified)
        if agent
            .get("readonly")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            log::debug!("Skipping readonly agent: {}", agent_id.cyan());
            return Ok(());
        }

        if self.agent_exists(agent_id).await? {
            // Agent exists - update it
            self.update_agent(agent_id, agent).await
        } else {
            // Agent doesn't exist - create it
            self.create_agent(agent).await
        }
    }
}

impl Loader for AgentsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;

        for agent in items {
            self.upsert_agent(&agent).await?;
            count += 1;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use serde_json::json;
    use tempfile::TempDir;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();
        let space_client = client.space("default").unwrap();
        let _loader = AgentsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = AgentsLoader::new(space_client);

        let agent = json!({"name": "No ID"});

        let result = loader.upsert_agent(&agent).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing 'id' field")
        );
    }
}
