//! Workflows API extractor
//!
//! Extracts workflow definitions from Kibana via GET /api/workflows/workflow/<id>

use crate::client::KibanaClient;
use crate::etl::Extractor;
use crate::kibana::workflows::workflow_resource_path;

use crate::{Error, Result, ResultContext};
use serde_json::Value;
use tokio::task::JoinSet;

/// Extractor for Kibana workflows
///
/// Fetches workflows by ID from the manifest. If no manifest is provided,
/// you should use the search API to discover workflows first.
///
/// # Example
/// ```no_run
/// use kibana_sync::kibana::workflows::{WorkflowsExtractor, WorkflowsManifest, WorkflowEntry};
/// use kibana_sync::client::{Auth, KibanaClient};
/// use kibana_sync::etl::Extractor;
/// use url::Url;
///
/// # async fn example() -> kibana_sync::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
/// let space_client = client.space("default")?;
/// let manifest = WorkflowsManifest::with_workflows(vec![
///     WorkflowEntry::new("workflow-123", "my-workflow"),
///     WorkflowEntry::new("workflow-456", "alert-workflow")
/// ]);
///
/// let extractor = WorkflowsExtractor::new(space_client, Some(manifest));
/// let workflows = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct WorkflowsExtractor {
    client: KibanaClient,
    manifest: Option<super::WorkflowsManifest>,
}

impl WorkflowsExtractor {
    /// Create a new workflows extractor
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    /// * `manifest` - Manifest containing workflow IDs to extract
    pub fn new(client: KibanaClient, manifest: Option<super::WorkflowsManifest>) -> Self {
        Self { client, manifest }
    }

    /// Search for workflows via the Workflows API
    ///
    /// Uses POST /api/workflows/search with optional query parameter.
    /// This is useful for discovering workflows before adding them to the manifest.
    ///
    /// # Arguments
    /// * `query` - Optional search query string to filter workflows
    /// * `size` - Maximum number of results to return (default: 100)
    ///
    /// # Returns
    /// Vector of workflow JSON objects from the search results
    pub async fn search_workflows(
        &self,
        query: Option<&str>,
        size: Option<usize>,
    ) -> Result<Vec<Value>> {
        let search_body = serde_json::json!({
            "size": size.unwrap_or(100),
            "query": query.unwrap_or("")
        });

        tracing::debug!(
            "Searching workflows with query: {:?} in space '{}'",
            query,
            self.client.space_id()
        );

        let response = self
            .client
            .post_json_value_internal("api/workflows/search", &search_body)
            .await
            .context("Failed to search workflows")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        let search_result: Value = response
            .json()
            .await
            .context("Failed to parse workflow search response")?;

        // Extract workflows from results array
        let workflows: Vec<Value> = search_result
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.to_vec())
            .unwrap_or_default();

        tracing::info!("Found {} workflow(s) via search", workflows.len());

        Ok(workflows)
    }

    /// Fetch specific workflows by ID from manifest
    async fn fetch_manifest_workflows(
        &self,
        manifest: &super::WorkflowsManifest,
    ) -> Result<Vec<Value>> {
        let mut workflows = Vec::new();
        let mut set = JoinSet::new();

        for entry in &manifest.workflows {
            let client = self.client.clone();
            let workflow_id = entry.id.clone();
            let workflow_name = entry.name.clone();

            set.spawn(async move {
                let path = workflow_resource_path(&workflow_id);
                tracing::debug!(
                    "Fetching workflow '{}' from space '{}'",
                    workflow_id,
                    client.space_id()
                );

                let response = client.get_internal(&path).await.with_context(|| {
                    format!(
                        "Failed to fetch workflow '{}' ({})",
                        workflow_name, workflow_id
                    )
                })?;

                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Err(Error::api_response(status, body));
                }

                let workflow: Value = response.json().await.with_context(|| {
                    format!("Failed to parse workflow '{}' response", workflow_id)
                })?;

                tracing::debug!("Fetched workflow: {}", workflow_id);
                Ok::<Value, Error>(workflow)
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(workflow)) => workflows.push(workflow),
                Ok(Err(e)) => tracing::warn!("{}", e),
                Err(e) => tracing::error!("Task panicked: {}", e),
            }
        }

        tracing::info!("Fetched {} workflow(s) from manifest", workflows.len());

        Ok(workflows)
    }
}

impl Extractor for WorkflowsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        let workflows = if let Some(manifest) = &self.manifest {
            // Fetch only workflows from manifest by ID
            self.fetch_manifest_workflows(manifest).await?
        } else {
            // No manifest provided - return empty list
            // Use search API separately to discover workflows
            tracing::warn!("No manifest provided - use search API to discover workflows");
            Vec::new()
        };

        tracing::info!(
            "Extracted {} workflow(s){}",
            workflows.len(),
            if self.manifest.is_some() {
                " (from manifest)"
            } else {
                ""
            }
        );

        Ok(workflows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_extractor_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let _extractor = WorkflowsExtractor::new(space_client, None);
    }

    #[tokio::test]
    async fn fetches_manifest_workflow_with_documented_endpoint() {
        let server = TestServer::new(vec![MockResponse {
            method: "GET",
            path: "/api/workflows/workflow/workflow-123",
            status: 200,
            body: json!({
                "id": "workflow-123",
                "name": "test-workflow",
                "yaml": "name: test"
            }),
        }]);
        let manifest = super::super::WorkflowsManifest::with_workflows(vec![
            super::super::WorkflowEntry::new("workflow-123", "test-workflow"),
        ]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), Some(manifest));

        let workflows = extractor.extract().await.unwrap();

        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0]["id"], "workflow-123");
        let requests = server.requests();
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, "/api/workflows/workflow/workflow-123");
    }
}
