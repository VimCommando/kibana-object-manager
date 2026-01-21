//! Tools API loader
//!
//! Loads tool definitions to Kibana via POST/PUT /api/agent_builder/tools

use crate::client::KibanaClient;
use crate::etl::Loader;

use eyre::Result;
use owo_colors::OwoColorize;
use serde_json::Value;
use tokio::task::JoinSet;

/// Loader for Kibana tools
///
/// Creates or updates tools in Kibana using POST (create) and PUT (update)
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::tools::ToolsLoader;
/// use kibana_object_manager::client::{Auth, KibanaClient};
/// use kibana_object_manager::etl::Loader;
/// use serde_json::json;
/// use url::Url;
/// use std::path::Path;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::try_new(url, Auth::None, Path::new("."), 8)?;
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
}

impl Loader for ToolsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;
        let mut set = JoinSet::new();

        for tool in items {
            let client = self.client.clone();

            set.spawn(async move {
                let tool_id = tool
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| eyre::eyre!("Tool missing 'id' field"))?;

                // Skip readonly tools
                if tool
                    .get("readonly")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    log::debug!("Skipping readonly tool: {}", tool_id.cyan());
                    return Ok::<bool, eyre::Report>(false);
                }

                // Check existence
                let path = format!("api/agent_builder/tools/{}", tool_id);
                let exists = match client.head(&path).await?.status().as_u16() {
                    200 => true,
                    404 => false,
                    status => {
                        eyre::bail!("Failed to check tool existence ({}): {}", tool_id, status);
                    }
                };

                if exists {
                    // Update
                    let mut tool_body = tool.clone();
                    if let Some(obj) = tool_body.as_object_mut() {
                        obj.remove("id");
                        obj.remove("readonly");
                        obj.remove("schema");
                        obj.remove("type");
                    }
                    let path = format!("api/agent_builder/tools/{}", tool_id);
                    let response = client.put_json_value(&path, &tool_body).await?;
                    if !response.status().is_success() {
                        eyre::bail!(
                            "Failed to update tool {} ({}): {}",
                            tool_id,
                            response.status(),
                            response.text().await.unwrap_or_default()
                        );
                    }
                    log::info!("Updated tool: {}", tool_id.cyan());
                } else {
                    // Create
                    let mut tool_body = tool.clone();
                    if let Some(obj) = tool_body.as_object_mut() {
                        obj.remove("readonly");
                        obj.remove("schema");
                    }
                    let path = "api/agent_builder/tools";
                    let response = client.post_json_value(path, &tool_body).await?;
                    if !response.status().is_success() {
                        eyre::bail!(
                            "Failed to create tool {} ({}): {}",
                            tool_id,
                            response.status(),
                            response.text().await.unwrap_or_default()
                        );
                    }
                    log::info!("Created tool: {}", tool_id.cyan());
                }

                Ok::<bool, eyre::Report>(true)
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(loaded)) => {
                    if loaded {
                        count += 1;
                    }
                }
                Ok(Err(e)) => log::error!("Failed to load tool: {}", e),
                Err(e) => log::error!("Task panicked: {}", e),
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
    use tempfile::TempDir;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path(), 8).unwrap();
        let space_client = client.space("default").unwrap();
        let _loader = ToolsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path(), 8).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = ToolsLoader::new(space_client);

        let tool = json!({"description": "No ID"});

        let result = loader.load(vec![tool]).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
