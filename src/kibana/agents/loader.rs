//! Agents API loader
//!
//! Loads agent definitions to Kibana via POST/PUT /api/agent_builder/agents

use crate::client::Kibana;
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
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None)?;
/// let loader = AgentsLoader::new(client, "default");
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
    client: Kibana,
    space_id: String,
}

impl AgentsLoader {
    /// Create a new agents loader
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    /// * `space_id` - Space ID to load agents into
    pub fn new(client: Kibana, space_id: impl Into<String>) -> Self {
        Self {
            client,
            space_id: space_id.into(),
        }
    }

    /// Build the space-qualified API path
    fn space_path(&self, endpoint: &str) -> String {
        if self.space_id == "default" {
            format!("/{}", endpoint)
        } else {
            format!("/s/{}/{}", self.space_id, endpoint)
        }
    }

    /// Check if an agent exists by ID using HEAD request
    ///
    /// Returns true if the agent exists, false if it returns 404
    async fn agent_exists(&self, agent_id: &str) -> Result<bool> {
        let path = format!("api/agent_builder/agents/{}", agent_id);
        let display_path = self.space_path(&path);

        log::debug!("{} {}", "HEAD".green(), display_path);

        let response = self.client.head_with_space(&self.space_id, &path).await?;

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
        let display_path = self.space_path(path);

        log::debug!("{} {}", "POST".green(), display_path);

        let response = self
            .client
            .post_json_value_with_space(&self.space_id, path, agent)
            .await?;

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
        let display_path = self.space_path(&path);

        log::debug!("{} {}", "PUT".green(), display_path);

        let response = self
            .client
            .put_json_value_with_space(&self.space_id, &path, agent)
            .await?;

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
    use crate::client::Auth;
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let _loader = AgentsLoader::new(client, "default");
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = AgentsLoader::new(client, "default");

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

    #[test]
    fn test_space_path_default() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = AgentsLoader::new(client, "default");

        assert_eq!(
            loader.space_path("api/agent_builder/agents/123"),
            "/api/agent_builder/agents/123"
        );
    }

    #[test]
    fn test_space_path_non_default() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = AgentsLoader::new(client, "shanks");

        assert_eq!(
            loader.space_path("api/agent_builder/agents/123"),
            "/s/shanks/api/agent_builder/agents/123"
        );
    }
}
