//! Workflows API loader
//!
//! Loads workflow definitions to Kibana via POST /api/workflows/workflow

use crate::client::KibanaClient;
use crate::etl::Loader;
use crate::kibana::workflows::workflow_create_path_for_version;
use crate::standalone::{
    ResourceBatchReport, ResourceFamily, ResourceOperation, ResourceOutcome, ResourceOutcomeStatus,
};

use crate::{Error, Result};
use serde_json::Value;
use tokio::task::JoinSet;

/// Loader for Kibana workflows
///
/// Creates or updates workflows in Kibana using the Workflows API.
///
/// # Example
/// ```no_run
/// use kibana_sync::kibana::workflows::WorkflowsLoader;
/// use kibana_sync::client::{Auth, KibanaClient};
/// use kibana_sync::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_sync::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
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

    /// Apply Workflows while preserving a structured outcome for every input item.
    pub async fn load_report(&self, items: Vec<Value>) -> ResourceBatchReport {
        let mut set = JoinSet::new();
        let mut outcomes = items
            .iter()
            .map(|workflow| {
                ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    resource_id(workflow),
                    None,
                    "loader task did not complete",
                )
            })
            .collect::<Vec<_>>();

        let mut pending = Vec::new();
        for (index, workflow) in items.into_iter().enumerate() {
            if workflow.get("id").and_then(Value::as_str).is_none() {
                outcomes[index] = ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    "<missing>",
                    None,
                    Error::MissingResourceId {
                        resource: "workflow",
                    }
                    .to_string(),
                );
                continue;
            }
            pending.push((index, workflow));
        }

        if pending.is_empty() {
            return ResourceBatchReport::new(outcomes);
        }

        let version = match self.client.server_version().await {
            Ok(version) => version,
            Err(error) => {
                for (index, workflow) in pending {
                    outcomes[index] = ResourceOutcome::failed(
                        ResourceFamily::Workflows,
                        resource_id(&workflow),
                        None,
                        format!("Failed to detect Kibana version for Workflow routing: {error}"),
                    );
                }
                return ResourceBatchReport::new(outcomes);
            }
        };
        let create_path = workflow_create_path_for_version(&version);

        for (index, workflow) in pending {
            let client = self.client.clone();
            set.spawn(async move { (index, upsert_workflow(client, workflow, create_path).await) });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok((index, outcome)) => outcomes[index] = outcome,
                Err(error) => tracing::error!("Workflow loader task panicked: {error}"),
            }
        }

        ResourceBatchReport::new(outcomes)
    }
}

impl Loader for WorkflowsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let report = self.load_report(items).await;
        for outcome in report.outcomes() {
            if outcome.status() == ResourceOutcomeStatus::Failed {
                tracing::error!(
                    "Failed to load workflow '{}': {}",
                    outcome.id(),
                    outcome.detail().unwrap_or("unknown failure")
                );
            }
        }

        Ok(report.counts().applied)
    }
}

async fn upsert_workflow(
    client: KibanaClient,
    workflow: Value,
    create_path: &'static str,
) -> ResourceOutcome {
    let Some(workflow_id) = workflow
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return ResourceOutcome::failed(
            ResourceFamily::Workflows,
            "<missing>",
            None,
            Error::MissingResourceId {
                resource: "workflow",
            }
            .to_string(),
        );
    };

    let workflow_name = workflow
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let path = format!("{create_path}/{workflow_id}");
    let exists = match client.head_internal(&path).await {
        Ok(response) => match response.status().as_u16() {
            200 => true,
            404 => false,
            status => {
                return ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    workflow_id,
                    None,
                    format!("Failed to check workflow existence: {status}"),
                );
            }
        },
        Err(error) => {
            return ResourceOutcome::failed(
                ResourceFamily::Workflows,
                workflow_id,
                None,
                error.to_string(),
            );
        }
    };

    let sanitized = WorkflowsLoader::sanitize_workflow(&workflow);
    if exists {
        let response = match client.put_json_value_internal(&path, &sanitized).await {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    workflow_id,
                    Some(ResourceOperation::Update),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Workflows,
                workflow_id,
                Some(ResourceOperation::Update),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Updated workflow: {} (id: {})", workflow_name, workflow_id);
        ResourceOutcome::applied(
            ResourceFamily::Workflows,
            workflow_id,
            ResourceOperation::Update,
        )
    } else {
        let response = match client
            .post_json_value_internal(create_path, &sanitized)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    workflow_id,
                    Some(ResourceOperation::Create),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Workflows,
                workflow_id,
                Some(ResourceOperation::Create),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Created workflow: {} (id: {})", workflow_name, workflow_id);
        ResourceOutcome::applied(
            ResourceFamily::Workflows,
            workflow_id,
            ResourceOperation::Create,
        )
    }
}

fn resource_id(resource: &Value) -> &str {
    resource
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<missing>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let _loader = WorkflowsLoader::new(space_client);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
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

    #[tokio::test]
    async fn creates_workflow_with_documented_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.4.1"}}),
            },
            MockResponse {
                method: "HEAD",
                path: "/s/esdiag/api/workflows/workflow/workflow-123",
                status: 404,
                body: json!({}),
            },
            MockResponse {
                method: "POST",
                path: "/s/esdiag/api/workflows/workflow",
                status: 200,
                body: json!({"id": "workflow-123"}),
            },
        ]);
        let loader = WorkflowsLoader::new(server.client().unwrap().space("esdiag").unwrap());
        let workflow = json!({
            "id": "workflow-123",
            "name": "test-workflow",
            "createdAt": "2023-01-01T00:00:00Z",
            "yaml": "name: test"
        });

        let report = loader.load_report(vec![workflow]).await;

        assert_eq!(report.counts().applied, 1);
        assert_eq!(
            report.outcomes()[0].operation(),
            Some(ResourceOperation::Create)
        );
        let requests = server.requests();
        assert_eq!(requests[1].method, "HEAD");
        assert_eq!(
            requests[1].path,
            "/s/esdiag/api/workflows/workflow/workflow-123"
        );
        assert_eq!(requests[2].method, "POST");
        assert_eq!(requests[2].path, "/s/esdiag/api/workflows/workflow");
        assert!(!requests[2].body.contains("createdAt"));
        for request in &requests[1..] {
            assert_eq!(
                request
                    .headers
                    .get("x-elastic-internal-origin")
                    .map(String::as_str),
                Some("Kibana")
            );
        }
        assert_eq!(
            requests[2].headers.get("kbn-xsrf").map(String::as_str),
            Some("true")
        );
    }

    #[tokio::test]
    async fn updates_workflow_with_documented_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.4.1"}}),
            },
            MockResponse {
                method: "HEAD",
                path: "/api/workflows/workflow/workflow-123",
                status: 200,
                body: json!({}),
            },
            MockResponse {
                method: "PUT",
                path: "/api/workflows/workflow/workflow-123",
                status: 200,
                body: json!({"id": "workflow-123"}),
            },
        ]);
        let loader = WorkflowsLoader::new(server.client().unwrap());
        let workflow = json!({
            "id": "workflow-123",
            "name": "test-workflow",
            "yaml": "name: test"
        });

        let report = loader.load_report(vec![workflow]).await;

        assert_eq!(
            report.outcomes()[0].operation(),
            Some(ResourceOperation::Update)
        );
        let requests = server.requests();
        assert!(requests[1..].iter().all(|request| {
            request
                .headers
                .get("x-elastic-internal-origin")
                .map(String::as_str)
                == Some("Kibana")
        }));
        let paths = requests
            .into_iter()
            .skip(1)
            .map(|request| (request.method, request.path))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                (
                    "HEAD".to_string(),
                    "/api/workflows/workflow/workflow-123".to_string()
                ),
                (
                    "PUT".to_string(),
                    "/api/workflows/workflow/workflow-123".to_string()
                )
            ]
        );
    }

    #[tokio::test]
    async fn creates_workflow_with_93_endpoint() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.3.3"}}),
            },
            MockResponse {
                method: "HEAD",
                path: "/api/workflows/workflow-123",
                status: 404,
                body: json!({}),
            },
            MockResponse {
                method: "POST",
                path: "/api/workflows",
                status: 200,
                body: json!({"id": "workflow-123"}),
            },
        ]);
        let loader = WorkflowsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({
                "id": "workflow-123",
                "name": "test-workflow",
                "yaml": "name: test"
            })])
            .await;

        assert_eq!(report.counts().applied, 1);
        let paths = server
            .requests()
            .into_iter()
            .map(|request| (request.method, request.path))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                ("GET".to_string(), "/api/status".to_string()),
                (
                    "HEAD".to_string(),
                    "/api/workflows/workflow-123".to_string()
                ),
                ("POST".to_string(), "/api/workflows".to_string())
            ]
        );
    }
}
