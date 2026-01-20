//! Tools API loader
//!
//! Loads tool definitions to Kibana via POST/PUT /api/agent_builder/tools

use crate::client::Kibana;
use crate::etl::Loader;

use eyre::Result;
use owo_colors::OwoColorize;
use serde_json::Value;

/// Loader for Kibana tools
///
/// Creates or updates tools in Kibana using POST (create) and PUT (update)
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::tools::ToolsLoader;
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None)?;
/// let loader = ToolsLoader::new(client, "default");
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
    client: Kibana,
    space_id: String,
}

impl ToolsLoader {
    /// Create a new tools loader
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    /// * `space_id` - Space ID to load tools into
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

    /// Check if a tool exists by ID using HEAD request
    ///
    /// Returns true if the tool exists, false if it returns 404
    async fn tool_exists(&self, tool_id: &str) -> Result<bool> {
        let path = format!("api/agent_builder/tools/{}", tool_id);
        let display_path = self.space_path(&path);

        log::debug!("{} {}", "HEAD".green(), display_path);

        let response = self.client.head_with_space(&self.space_id, &path).await?;

        match response.status().as_u16() {
            200 => {
                log::debug!(
                    "{} {} - tool exists, will update",
                    "200".green(),
                    tool_id.cyan()
                );
                Ok(true)
            }
            404 => {
                log::debug!(
                    "{} {} - tool not found, will create",
                    "404".yellow(),
                    tool_id.cyan()
                );
                Ok(false)
            }
            _ => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                eyre::bail!(
                    "Failed to check tool {} existence ({}): {}",
                    tool_id.cyan(),
                    status,
                    body
                )
            }
        }
    }

    /// Create a new tool using POST /api/agent_builder/tools/
    async fn create_tool(&self, tool: &Value) -> Result<()> {
        let tool_id = tool
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Tool missing 'id' field"))?;

        let path = "api/agent_builder/tools/";
        let display_path = self.space_path(path);

        log::debug!("{} {}", "POST".green(), display_path);

        let response = self
            .client
            .post_json_value_with_space(&self.space_id, path, tool)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to create tool {} ({}): {}",
                tool_id.cyan(),
                status,
                body
            );
        }

        log::info!("Created tool: {}", tool_id.cyan());

        Ok(())
    }

    /// Update an existing tool using PUT /api/agent_builder/tools/<id>
    ///
    /// Note: Unlike the POST (create) endpoint, the PUT (update) endpoint does NOT
    /// include the 'id' field in the request body - it's only in the URL path.
    async fn update_tool(&self, tool_id: &str, tool: &Value) -> Result<()> {
        // Remove the 'id' field from the body since it shouldn't be in PUT requests
        let mut tool_body = tool.clone();
        if let Some(obj) = tool_body.as_object_mut() {
            obj.remove("id");
        }

        let path = format!("api/agent_builder/tools/{}", tool_id);
        let display_path = self.space_path(&path);

        log::debug!("{} {}", "PUT".green(), display_path);

        let response = self
            .client
            .put_json_value_with_space(&self.space_id, &path, &tool_body)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to update tool {} ({}): {}",
                tool_id.cyan(),
                status,
                body
            );
        }

        log::info!("Updated tool: {}", tool_id.cyan());

        Ok(())
    }

    /// Create or update a single tool
    ///
    /// Checks if the tool exists first to determine whether to use
    /// POST (create) or PUT (update)
    ///
    /// Skips readonly tools as they cannot be modified
    async fn upsert_tool(&self, tool: &Value) -> Result<()> {
        let tool_id = tool
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Tool missing 'id' field"))?;

        // Skip readonly tools (builtin tools that can't be modified)
        if tool
            .get("readonly")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            log::debug!("Skipping readonly tool: {}", tool_id.cyan());
            return Ok(());
        }

        if self.tool_exists(tool_id).await? {
            // Tool exists - update it
            self.update_tool(tool_id, tool).await
        } else {
            // Tool doesn't exist - create it
            self.create_tool(tool).await
        }
    }
}

impl Loader for ToolsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;

        for tool in items {
            self.upsert_tool(&tool).await?;
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
        let _loader = ToolsLoader::new(client, "default");
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = ToolsLoader::new(client, "default");

        let tool = json!({"description": "No ID"});

        let result = loader.upsert_tool(&tool).await;

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
        let loader = ToolsLoader::new(client, "default");

        assert_eq!(
            loader.space_path("api/agent_builder/tools/123"),
            "/api/agent_builder/tools/123"
        );
    }

    #[test]
    fn test_space_path_non_default() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = ToolsLoader::new(client, "shanks");

        assert_eq!(
            loader.space_path("api/agent_builder/tools/123"),
            "/s/shanks/api/agent_builder/tools/123"
        );
    }
}
