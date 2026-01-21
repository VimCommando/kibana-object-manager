//! Workflows API loader
//!
//! Loads workflow definitions to Kibana via POST /api/workflows/<id>

use crate::client::KibanaClient;
use crate::etl::Loader;

use eyre::Result;
use owo_colors::OwoColorize;
use serde_json::Value;
use tokio::task::JoinSet;

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
/// let client = KibanaClient::try_new(url, Auth::None, Path::new("."), 8)?;
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

    /// Sanitize workflow payload by removing read-only system fields
    fn sanitize_workflow(workflow: &Value) -> Value {
        let mut sanitized = workflow.clone();
        if let Value::Object(ref mut map) = sanitized {
            map.remove("createdAt");
            map.remove("lastUpdatedAt");
            map.remove("createdBy");
            map.remove("lastUpdatedBy");
            map.remove("valid");
            map.remove("validationErrors");
            map.remove("history");
        }
        sanitized
    }
}

impl Loader for WorkflowsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;
        let mut set = JoinSet::new();

        for workflow in items {
            let client = self.client.clone();

            set.spawn(async move {
                let workflow_id = workflow
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| eyre::eyre!("Workflow missing 'id' field"))?;

                // Check if a workflow exists using HEAD request
                let path = format!("api/workflows/{}", workflow_id);
                let exists = match client.head_internal(&path).await?.status().as_u16() {
                    200 => true,
                    404 => false,
                    status => {
                        eyre::bail!(
                            "Failed to check workflow existence ({}): {}",
                            workflow_id,
                            status
                        );
                    }
                };

                let workflow_name = workflow
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                if exists {
                    // Update
                    let path = format!("api/workflows/{}", workflow_id);
                    let sanitized = WorkflowsLoader::sanitize_workflow(&workflow);
                    let response = client.put_json_value_internal(&path, &sanitized).await?;
                    if !response.status().is_success() {
                        eyre::bail!(
                            "Failed to update workflow {} ({}): {}",
                            workflow_name,
                            workflow_id,
                            response.status()
                        );
                    }
                    log::info!(
                        "Updated workflow: {} (id: {})",
                        workflow_name.cyan(),
                        workflow_id.cyan()
                    );
                } else {
                    // Create
                    let path = "api/workflows";
                    let sanitized = WorkflowsLoader::sanitize_workflow(&workflow);
                    let response = client.post_json_value_internal(path, &sanitized).await?;
                    if !response.status().is_success() {
                        eyre::bail!(
                            "Failed to create workflow {} ({}): {}",
                            workflow_name,
                            workflow_id,
                            response.status()
                        );
                    }
                    log::info!(
                        "Created workflow: {} (id: {})",
                        workflow_name.cyan(),
                        workflow_id.cyan()
                    );
                }

                Ok::<(), eyre::Report>(())
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(())) => count += 1,
                Ok(Err(e)) => log::error!("Failed to load workflow: {}", e),
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
        let _loader = WorkflowsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path(), 8).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = WorkflowsLoader::new(space_client);

        let workflow = json!({"name": "No ID"});

        let result = loader.load(vec![workflow]).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_sanitize_workflow() {
        let workflow = json!({
            "id": "workflow-123",
            "name": "test-workflow",
            "createdAt": "2023-01-01T00:00:00Z",
            "lastUpdatedAt": "2023-01-02T00:00:00Z",
            "createdBy": "user",
            "lastUpdatedBy": "user",
            "valid": true,
            "validationErrors": [],
            "history": [],
            "definition": {"some": "data"},
            "yaml": "name: test"
        });

        let sanitized = WorkflowsLoader::sanitize_workflow(&workflow);
        let sanitized_obj = sanitized.as_object().unwrap();

        assert!(sanitized_obj.contains_key("id"));
        assert!(sanitized_obj.contains_key("name"));
        assert!(sanitized_obj.contains_key("yaml"));
        assert!(sanitized_obj.contains_key("definition"));

        assert!(!sanitized_obj.contains_key("createdAt"));
        assert!(!sanitized_obj.contains_key("lastUpdatedAt"));
        assert!(!sanitized_obj.contains_key("createdBy"));
        assert!(!sanitized_obj.contains_key("lastUpdatedBy"));
        assert!(!sanitized_obj.contains_key("valid"));
        assert!(!sanitized_obj.contains_key("validationErrors"));
        assert!(!sanitized_obj.contains_key("history"));
    }
}
