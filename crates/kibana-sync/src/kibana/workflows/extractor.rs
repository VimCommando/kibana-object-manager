//! Workflows API extractor
//!
//! Extracts workflow definitions from Kibana's version-specific Workflows API.

use crate::client::KibanaClient;
use crate::etl::Extractor;
use crate::kibana::workflows::{uses_current_workflow_routes, workflow_resource_path_for_version};

use crate::{Error, Result, ResultContext};
use serde_json::Value;
use tokio::task::JoinSet;

const DEFAULT_WORKFLOW_SEARCH_SIZE: usize = 100;
const LEGACY_WORKFLOW_EXPORT_SIZE: usize = 1000;

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
    /// Uses the server's Workflow listing endpoint with optional query parameter.
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
        tracing::debug!(
            "Searching workflows with query: {:?} in space '{}'",
            query,
            self.client.space_id()
        );

        let max_results = size.unwrap_or(DEFAULT_WORKFLOW_SEARCH_SIZE);
        if max_results == 0 {
            return Err(Error::message(
                "Workflow search size must be greater than zero",
            ));
        }

        let version = self.client.server_version().await?;
        let workflows = if uses_current_workflow_routes(&version) {
            self.list_current_workflow_page(query, max_results, 1)
                .await?
                .0
        } else {
            self.search_legacy_workflows(query, max_results).await?
        };

        tracing::info!("Found {} workflow(s) via search", workflows.len());
        Ok(workflows)
    }

    /// Discover every Workflow through the version-appropriate listing API.
    pub async fn search_all_workflows(&self, query: Option<&str>) -> Result<Vec<Value>> {
        let version = self.client.server_version().await?;
        let workflows = if uses_current_workflow_routes(&version) {
            self.list_all_current_workflows(query, DEFAULT_WORKFLOW_SEARCH_SIZE)
                .await?
        } else {
            self.search_legacy_workflows(query, LEGACY_WORKFLOW_EXPORT_SIZE)
                .await?
        };

        tracing::info!("Found {} workflow(s) via complete search", workflows.len());
        Ok(workflows)
    }

    async fn list_all_current_workflows(
        &self,
        query: Option<&str>,
        page_size: usize,
    ) -> Result<Vec<Value>> {
        let mut workflows = Vec::new();
        let mut page = 1_usize;
        loop {
            let (page_results, reported_total) = self
                .list_current_workflow_page(query, page_size, page)
                .await?;
            let result_count = page_results.len();
            workflows.extend(page_results);

            if result_count == 0
                && reported_total.is_some_and(|total| (workflows.len() as u64) < total)
            {
                return Err(Error::message(format!(
                    "Workflow listing stopped making progress on page {page}"
                )));
            }
            let listing_complete = reported_total
                .map(|total| workflows.len() as u64 >= total)
                .unwrap_or(result_count < page_size);
            if listing_complete {
                break;
            }
            page = page.checked_add(1).ok_or_else(|| {
                Error::message("Workflow listing exceeded the supported page range")
            })?;
        }

        Ok(workflows)
    }

    async fn list_current_workflow_page(
        &self,
        query: Option<&str>,
        size: usize,
        page: usize,
    ) -> Result<(Vec<Value>, Option<u64>)> {
        let mut query_string = url::form_urlencoded::Serializer::new(String::new());
        query_string
            .append_pair("size", &size.to_string())
            .append_pair("page", &page.to_string());
        if let Some(query) = query.filter(|query| !query.is_empty()) {
            query_string.append_pair("query", query);
        }
        let path = format!("api/workflows?{}", query_string.finish());
        let response = self
            .client
            .get_internal(&path)
            .await
            .with_context(|| format!("Failed to list workflows page {page}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        let search_result: Value = response
            .json()
            .await
            .with_context(|| format!("Failed to parse workflow list page {page}"))?;
        let results = search_result
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let total = search_result.get("total").and_then(Value::as_u64);
        Ok((results, total))
    }

    async fn search_legacy_workflows(
        &self,
        query: Option<&str>,
        size: usize,
    ) -> Result<Vec<Value>> {
        let search_body = serde_json::json!({
            "size": size,
            "query": query.unwrap_or("")
        });
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
        Ok(search_result
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    /// Fetch one complete Workflow definition by ID.
    pub async fn fetch_workflow(&self, workflow_id: &str) -> Result<Value> {
        let version = self.client.server_version().await?;
        let path = workflow_resource_path_for_version(&version, workflow_id);
        let response = self
            .client
            .get_internal(&path)
            .await
            .with_context(|| format!("Failed to fetch workflow '{workflow_id}'"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        response
            .json()
            .await
            .with_context(|| format!("Failed to parse workflow '{workflow_id}' response"))
    }

    /// Fetch specific workflows by ID from manifest
    async fn fetch_manifest_workflows(
        &self,
        manifest: &super::WorkflowsManifest,
    ) -> Result<Vec<Value>> {
        let mut workflows = Vec::new();
        let mut set = JoinSet::new();
        let version = self.client.server_version().await?;

        for entry in &manifest.workflows {
            let client = self.client.clone();
            let workflow_id = entry.id.clone();
            let workflow_name = entry.name.clone();
            let path = workflow_resource_path_for_version(&version, &workflow_id);

            set.spawn(async move {
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
    async fn rejects_zero_search_size_before_version_detection() {
        let server = TestServer::new(Vec::new());
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), None);

        let error = extractor.search_workflows(None, Some(0)).await.unwrap_err();

        assert!(error.to_string().contains("greater than zero"));
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn fetches_manifest_workflow_with_documented_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.4.1"}}),
            },
            MockResponse {
                method: "GET",
                path: "/api/workflows/workflow/workflow-123",
                status: 200,
                body: json!({
                    "id": "workflow-123",
                    "name": "test-workflow",
                    "yaml": "name: test"
                }),
            },
        ]);
        let manifest = super::super::WorkflowsManifest::with_workflows(vec![
            super::super::WorkflowEntry::new("workflow-123", "test-workflow"),
        ]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), Some(manifest));

        let workflows = extractor.extract().await.unwrap();

        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0]["id"], "workflow-123");
        let requests = server.requests();
        assert_eq!(requests[1].method, "GET");
        assert_eq!(requests[1].path, "/api/workflows/workflow/workflow-123");
    }

    #[tokio::test]
    async fn searches_workflows_with_93_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.3.3"}}),
            },
            MockResponse {
                method: "POST",
                path: "/api/workflows/search",
                status: 200,
                body: json!({"results": [{"id": "workflow-123"}]}),
            },
        ]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), None);

        let workflows = extractor
            .search_workflows(Some("test workflow"), Some(25))
            .await
            .unwrap();

        assert_eq!(workflows[0]["id"], "workflow-123");
        let requests = server.requests();
        assert_eq!(requests[1].method, "POST");
        assert_eq!(requests[1].path, "/api/workflows/search");
        let body: Value = serde_json::from_str(&requests[1].body).unwrap();
        assert_eq!(body["query"], "test workflow");
        assert_eq!(body["size"], 25);
    }

    #[tokio::test]
    async fn lists_workflows_with_94_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.4.1"}}),
            },
            MockResponse {
                method: "GET",
                path: "/api/workflows?size=25&page=1&query=test+workflow",
                status: 200,
                body: json!({"results": [{"id": "workflow-123"}]}),
            },
        ]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), None);

        let workflows = extractor
            .search_workflows(Some("test workflow"), Some(25))
            .await
            .unwrap();

        assert_eq!(workflows[0]["id"], "workflow-123");
        let requests = server.requests();
        assert_eq!(requests[1].method, "GET");
        assert_eq!(
            requests[1].path,
            "/api/workflows?size=25&page=1&query=test+workflow"
        );
    }

    #[tokio::test]
    async fn lists_every_workflow_page_with_94_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/workflows?size=3&page=1",
                status: 200,
                body: json!({
                    "results": [
                        {"id": "workflow-1"},
                        {"id": "workflow-2"}
                    ],
                    "total": 3
                }),
            },
            MockResponse {
                method: "GET",
                path: "/api/workflows?size=3&page=2",
                status: 200,
                body: json!({
                    "results": [{"id": "workflow-3"}],
                    "total": 3
                }),
            },
        ]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), None);

        let workflows = extractor.list_all_current_workflows(None, 3).await.unwrap();

        assert_eq!(
            workflows
                .iter()
                .map(|workflow| workflow["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["workflow-1", "workflow-2", "workflow-3"]
        );
        let paths = server
            .requests()
            .into_iter()
            .map(|request| request.path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/api/workflows?size=3&page=1",
                "/api/workflows?size=3&page=2"
            ]
        );
    }

    #[tokio::test]
    async fn stops_workflow_pagination_at_reported_total() {
        let server = TestServer::new(vec![MockResponse {
            method: "GET",
            path: "/api/workflows?size=2&page=1",
            status: 200,
            body: json!({
                "results": [
                    {"id": "workflow-1"},
                    {"id": "workflow-2"}
                ],
                "total": 2
            }),
        }]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), None);

        let workflows = extractor.list_all_current_workflows(None, 2).await.unwrap();

        assert_eq!(workflows.len(), 2);
        assert_eq!(server.requests().len(), 1);
    }

    #[tokio::test]
    async fn search_size_limits_results_to_one_current_route_page() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.4.1"}}),
            },
            MockResponse {
                method: "GET",
                path: "/api/workflows?size=1&page=1",
                status: 200,
                body: json!({
                    "results": [{"id": "workflow-1"}],
                    "total": 3
                }),
            },
        ]);
        let extractor = WorkflowsExtractor::new(server.client().unwrap(), None);

        let workflows = extractor.search_workflows(None, Some(1)).await.unwrap();

        assert_eq!(workflows.len(), 1);
        assert_eq!(server.requests().len(), 2);
    }
}
