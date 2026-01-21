//! Agents API extractor
//!
//! Extracts agent definitions from Kibana via GET /api/agent_builder/agents

use crate::client::KibanaClient;
use crate::etl::Extractor;

use eyre::{Context, Result};
use serde_json::Value;

/// Extractor for Kibana agents
///
/// Fetches agents by ID from the manifest. If no manifest is provided,
/// you should use the search API to discover agents first.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::agents::{AgentsExtractor, AgentsManifest, AgentEntry};
/// use kibana_object_manager::client::{Auth, KibanaClient};
/// use kibana_object_manager::etl::Extractor;
/// use url::Url;
/// use std::path::Path;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::try_new(url, Auth::None, Path::new("."))?;
/// let space_client = client.space("default")?;
/// let manifest = AgentsManifest::with_agents(vec![
///     AgentEntry::new("agent-123", "my-agent"),
///     AgentEntry::new("agent-456", "customer-support-agent")
/// ]);
///
/// let extractor = AgentsExtractor::new(space_client, Some(manifest));
/// let agents = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct AgentsExtractor {
    client: KibanaClient,
    manifest: Option<super::AgentsManifest>,
}

impl AgentsExtractor {
    /// Create a new agents extractor
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    /// * `manifest` - Manifest containing agent IDs to extract
    pub fn new(client: KibanaClient, manifest: Option<super::AgentsManifest>) -> Self {
        Self { client, manifest }
    }

    /// Search for agents via the Agents API
    ///
    /// Uses GET /api/agent_builder/agents to fetch all agents.
    /// This is useful for discovering agents before adding them to the manifest.
    ///
    /// # Arguments
    /// * `_query` - Reserved for future use (currently unused)
    ///
    /// # Returns
    /// Vector of agent JSON objects from the search results
    pub async fn search_agents(&self, _query: Option<&str>) -> Result<Vec<Value>> {
        let path = "api/agent_builder/agents";

        log::debug!(
            "Fetching agents from {} in space '{}'",
            path,
            self.client.space_id()
        );

        let response = self
            .client
            .get(path)
            .await
            .context("Failed to fetch agents")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!("Failed to fetch agents ({}): {}", status, body);
        }

        let search_result: Value = response
            .json()
            .await
            .context("Failed to parse agents response")?;

        // Extract agents from results array
        let agents: Vec<Value> = search_result
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().cloned().collect())
            .unwrap_or_default();

        log::info!("Found {} agent(s) via search", agents.len());

        Ok(agents)
    }

    /// Fetch a single agent by ID from Kibana
    async fn fetch_agent(&self, agent_id: &str) -> Result<Value> {
        let path = format!("api/agent_builder/agents/{}", agent_id);

        log::debug!(
            "Fetching agent '{}' from space '{}'",
            agent_id,
            self.client.space_id()
        );

        let response = self
            .client
            .get(&path)
            .await
            .with_context(|| format!("Failed to fetch agent '{}'", agent_id))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to fetch agent '{}' ({}): {}",
                agent_id,
                status,
                body
            );
        }

        let agent: Value = response
            .json()
            .await
            .with_context(|| format!("Failed to parse agent '{}' response", agent_id))?;

        log::debug!("Fetched agent: {}", agent_id);

        Ok(agent)
    }

    /// Fetch specific agents by ID from manifest
    async fn fetch_manifest_agents(&self, manifest: &super::AgentsManifest) -> Result<Vec<Value>> {
        let mut agents = Vec::new();

        for entry in &manifest.agents {
            match self.fetch_agent(&entry.id).await {
                Ok(agent) => agents.push(agent),
                Err(e) => {
                    log::warn!(
                        "Failed to fetch agent '{}' (id: {}): {}",
                        entry.name,
                        entry.id,
                        e
                    );
                    // Continue with other agents instead of failing completely
                }
            }
        }

        log::info!("Fetched {} agent(s) from manifest", agents.len());

        Ok(agents)
    }
}

impl Extractor for AgentsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        let agents = if let Some(manifest) = &self.manifest {
            // Fetch only agents from manifest by ID
            self.fetch_manifest_agents(manifest).await?
        } else {
            // No manifest provided - return empty list
            // Use search API separately to discover agents
            log::warn!("No manifest provided - use search API to discover agents");
            Vec::new()
        };

        log::info!(
            "Extracted {} agent(s){}",
            agents.len(),
            if self.manifest.is_some() {
                " (from manifest)"
            } else {
                ""
            }
        );

        Ok(agents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use tempfile::TempDir;
    use url::Url;

    #[test]
    fn test_extractor_creation() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();
        let space_client = client.space("default").unwrap();
        let _extractor = AgentsExtractor::new(space_client, None);
    }
}
