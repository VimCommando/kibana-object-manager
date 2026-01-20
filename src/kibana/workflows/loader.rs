//! Workflows API loader
//!
//! Loads workflow definitions to Kibana via POST /api/workflows/<id>

use crate::client::KibanaClient;
use crate::etl::Loader;

use eyre::Result;
use owo_colors::OwoColorize;
use serde_json::Value;

/// Loader for Kibana workflows
///
/// Creates or updates workflows in Kibana using POST /api/workflows/<id>
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::workflows::WorkflowsLoader;
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
/// let loader = WorkflowsLoader::new(space_client);
///
/// let workflows = vec![
///     json!({
///         "id": "workflow-123",
///         "name": "my-workflow",
///         "description": "Example workflow"
///     })
/// ];
///
/// let count = loader.load(workflows).await?;
/// # Ok(())
/// # }
/// ```
pub struct WorkflowsLoader {
    client: KibanaClient,
}

impl WorkflowsLoader {
    /// Create a new workflows loader
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    pub fn new(client: KibanaClient) -> Self {
        Self { client }
    }

    /// Check if a workflow exists using HEAD request
    async fn workflow_exists(&self, workflow_id: &str) -> Result<bool> {
        let path = format!("api/workflows/{}", workflow_id);

        log::debug!("{} {}", "HEAD".green(), path);

        let response = self.client.head_internal(&path).await?;

        match response.status().as_u16() {
            200 => {
                log::debug!(
                    "{} {} - workflow exists, will update",
                    "200".green(),
                    workflow_id.cyan()
                );
                Ok(true)
            }
            404 => {
                log::debug!(
                    "{} {} - workflow not found, will create",
                    "404".yellow(),
                    workflow_id.cyan()
                );
                Ok(false)
            }
            _ => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                eyre::bail!(
                    "Failed to check workflow {} existence ({}): {}",
                    workflow_id.cyan(),
                    status,
                    body
                )
            }
        }
    }

    /// Create a new workflow using POST /api/workflows/<id>
    async fn create_workflow(&self, workflow: &Value) -> Result<()> {
        let workflow_id = workflow
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'id' field"))?;

        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let path = format!("api/workflows/{}", workflow_id);

        log::debug!("{} {}", "POST".green(), path);

        let response = self
            .client
            .post_json_value_internal(&path, workflow)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to create workflow {} (id: {}) ({}): {}",
                workflow_name.cyan(),
                workflow_id.cyan(),
                status,
                body
            );
        }

        log::info!(
            "Created workflow: {} (id: {})",
            workflow_name.cyan(),
            workflow_id.cyan()
        );

        Ok(())
    }

    /// Update an existing workflow using POST /api/workflows/<id>
    async fn update_workflow(&self, workflow: &Value) -> Result<()> {
        let workflow_id = workflow
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'id' field"))?;

        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let path = format!("api/workflows/{}", workflow_id);

        log::debug!("{} {}", "POST".green(), path);

        let response = self
            .client
            .post_json_value_internal(&path, workflow)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to update workflow {} (id: {}) ({}): {}",
                workflow_name.cyan(),
                workflow_id.cyan(),
                status,
                body
            );
        }

        log::info!(
            "Updated workflow: {} (id: {})",
            workflow_name.cyan(),
            workflow_id.cyan()
        );

        Ok(())
    }

    /// Create or update a single workflow
    ///
    /// Checks if the workflow exists first to determine whether to create or update
    async fn upsert_workflow(&self, workflow: &Value) -> Result<()> {
        let workflow_id = workflow
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'id' field"))?;

        if self.workflow_exists(workflow_id).await? {
            // Workflow exists - update it
            self.update_workflow(workflow).await
        } else {
            // Workflow doesn't exist - create it
            self.create_workflow(workflow).await
        }
    }
}

impl Loader for WorkflowsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;

        for workflow in items {
            self.upsert_workflow(&workflow).await?;
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
        let _loader = WorkflowsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = WorkflowsLoader::new(space_client);

        let workflow = json!({"name": "No ID"});

        let result = loader.upsert_workflow(&workflow).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing 'id' field")
        );
    }
}
