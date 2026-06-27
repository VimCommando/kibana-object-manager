//! Tools API extractor
//!
//! Extracts tool definitions from Kibana via GET /api/agent_builder/tools

use crate::client::KibanaClient;
use crate::etl::Extractor;

use crate::{Error, Result, ResultContext};
use serde_json::Value;
use tokio::task::JoinSet;

/// Extractor for Kibana tools
///
/// Fetches tools by ID from the manifest. If no manifest is provided,
/// you should use the search API to discover tools first.
///
/// # Example
/// ```no_run
/// use kibana_client::kibana::tools::{ToolsExtractor, ToolsManifest};
/// use kibana_client::client::{Auth, KibanaClient};
/// use kibana_client::etl::Extractor;
/// use url::Url;
///
/// # async fn example() -> kibana_client::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
/// let space_client = client.space("default")?;
/// let manifest = ToolsManifest::with_tools(vec![
///     "platform.core.search".to_string(),
///     "platform.core.get_document_by_id".to_string()
/// ]);
///
/// let extractor = ToolsExtractor::new(space_client, Some(manifest));
/// let tools = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct ToolsExtractor {
    client: KibanaClient,
    manifest: Option<super::ToolsManifest>,
}

impl ToolsExtractor {
    /// Create a new tools extractor
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    /// * `manifest` - Manifest containing tool IDs to extract
    pub fn new(client: KibanaClient, manifest: Option<super::ToolsManifest>) -> Self {
        Self { client, manifest }
    }

    /// Search for tools via the Tools API
    ///
    /// Uses GET /api/agent_builder/tools to fetch all tools.
    /// This is useful for discovering tools before adding them to the manifest.
    ///
    /// # Arguments
    /// * `_query` - Reserved for future use (currently unused)
    ///
    /// # Returns
    /// Vector of tool JSON objects from the search results
    pub async fn search_tools(&self, _query: Option<&str>) -> Result<Vec<Value>> {
        let path = "api/agent_builder/tools";

        tracing::debug!(
            "Fetching tools from {} in space '{}'",
            path,
            self.client.space_id()
        );

        let response = self
            .client
            .get(path)
            .await
            .context("Failed to fetch tools")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        let search_result: Value = response
            .json()
            .await
            .context("Failed to parse tools response")?;

        // Extract tools from results array
        let tools: Vec<Value> = search_result
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.to_vec())
            .unwrap_or_default();

        tracing::info!("Found {} tool(s) via search", tools.len());

        Ok(tools)
    }

    /// Fetch specific tools by ID from manifest
    async fn fetch_manifest_tools(&self, manifest: &super::ToolsManifest) -> Result<Vec<Value>> {
        let mut tools = Vec::new();
        let mut set = JoinSet::new();

        for tool_id in &manifest.tools {
            let client = self.client.clone();
            let tool_id = tool_id.clone();

            set.spawn(async move {
                let path = format!("api/agent_builder/tools/{}", tool_id);
                tracing::debug!(
                    "Fetching tool '{}' from space '{}'",
                    tool_id,
                    client.space_id()
                );

                let response = client
                    .get(&path)
                    .await
                    .with_context(|| format!("Failed to fetch tool '{}'", tool_id))?;

                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Err(Error::api_response(status, body));
                }

                let tool: Value = response
                    .json()
                    .await
                    .with_context(|| format!("Failed to parse tool '{}' response", tool_id))?;

                tracing::debug!("Fetched tool: {}", tool_id);
                Ok::<Value, Error>(tool)
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(tool)) => tools.push(tool),
                Ok(Err(e)) => tracing::warn!("{}", e),
                Err(e) => tracing::error!("Task panicked: {}", e),
            }
        }

        tracing::info!("Fetched {} tool(s) from manifest", tools.len());

        Ok(tools)
    }
}

impl Extractor for ToolsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        let tools = if let Some(manifest) = &self.manifest {
            // Fetch only tools from manifest by ID
            self.fetch_manifest_tools(manifest).await?
        } else {
            // No manifest provided - return empty list
            // Use search API separately to discover tools
            tracing::warn!("No manifest provided - use search API to discover tools");
            Vec::new()
        };

        tracing::info!(
            "Extracted {} tool(s){}",
            tools.len(),
            if self.manifest.is_some() {
                " (from manifest)"
            } else {
                ""
            }
        );

        Ok(tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use url::Url;

    #[test]
    fn test_extractor_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let _extractor = ToolsExtractor::new(space_client, None);
    }
}
