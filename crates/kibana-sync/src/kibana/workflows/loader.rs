//! Workflows API loader
//!
//! Creates or updates workflow definitions using version-aware GET/POST/PUT routes.

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
    let exists = match writable_workflow_exists(&client, &path).await {
        Ok(exists) => exists,
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
    if !exists {
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
        if response.status().is_success() {
            tracing::info!("Created workflow: {} (id: {})", workflow_name, workflow_id);
            return ResourceOutcome::applied(
                ResourceFamily::Workflows,
                workflow_id,
                ResourceOperation::Create,
            );
        }
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|error| format!("Failed to read response body: {error}"));
        let create_error = Error::api_response(status, body);
        if status != reqwest::StatusCode::CONFLICT {
            return ResourceOutcome::failed(
                ResourceFamily::Workflows,
                workflow_id,
                Some(ResourceOperation::Create),
                create_error.to_string(),
            );
        }
        // Another writer may have created the workflow after our initial GET.
        match writable_workflow_exists(&client, &path).await {
            Ok(true) => {}
            Ok(false) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    workflow_id,
                    Some(ResourceOperation::Create),
                    create_error.to_string(),
                );
            }
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Workflows,
                    workflow_id,
                    Some(ResourceOperation::Create),
                    format!("{create_error}; failed to confirm writable Workflow: {error}"),
                );
            }
        }
    }

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
        let body = response
            .text()
            .await
            .unwrap_or_else(|error| format!("Failed to read response body: {error}"));
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
}

async fn writable_workflow_exists(client: &KibanaClient, path: &str) -> Result<bool> {
    let response = client.get_internal(path).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(false);
    }
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(Error::api_response(status, body));
    }
    let existing = response
        .json::<Value>()
        .await
        .map_err(|error| Error::message(format!("Failed to parse existing Workflow: {error}")))?;
    if !existing.is_object() {
        return Err(Error::message(
            "Failed to parse existing Workflow: expected a JSON object",
        ));
    }
    if existing.get("readonly").and_then(Value::as_bool) == Some(true) {
        return Err(Error::message("server-side Workflow is readonly"));
    }
    Ok(true)
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

    struct WorkflowApi {
        version: &'static str,
        space: &'static str,
        collection: &'static str,
        item: &'static str,
    }

    const WORKFLOW_APIS: [WorkflowApi; 4] = [
        WorkflowApi {
            version: "9.3.3",
            space: "default",
            collection: "/api/workflows",
            item: "/api/workflows/workflow-123",
        },
        WorkflowApi {
            version: "9.3.3",
            space: "esdiag",
            collection: "/s/esdiag/api/workflows",
            item: "/s/esdiag/api/workflows/workflow-123",
        },
        WorkflowApi {
            version: "9.4.1",
            space: "default",
            collection: "/api/workflows/workflow",
            item: "/api/workflows/workflow/workflow-123",
        },
        WorkflowApi {
            version: "9.4.1",
            space: "esdiag",
            collection: "/s/esdiag/api/workflows/workflow",
            item: "/s/esdiag/api/workflows/workflow/workflow-123",
        },
    ];

    impl WorkflowApi {
        fn response(&self, method: &'static str, status: u16, body: Value) -> MockResponse {
            MockResponse {
                method,
                path: if method == "POST" {
                    self.collection
                } else {
                    self.item
                },
                status,
                body,
            }
        }

        fn server(&self, responses: Vec<MockResponse>) -> TestServer {
            let mut all = vec![MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": self.version}}),
            }];
            all.extend(responses);
            TestServer::new(all)
        }

        fn loader(&self, server: &TestServer) -> WorkflowsLoader {
            WorkflowsLoader::new(server.client().unwrap().space(self.space).unwrap())
        }
    }

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
    async fn repeated_sync_updates_existing_workflow_without_head_request() {
        let existing = json!({
            "id": "workflow-123",
            "name": "old-workflow",
            "yaml": "name: old",
            "definition": {"name": "old"},
            "readonly": false
        });
        let desired = json!({
            "id": "workflow-123",
            "name": "desired-workflow",
            "yaml": "name: desired",
            "definition": {"name": "desired"}
        });
        for api in WORKFLOW_APIS {
            let mut responses = Vec::new();
            for workflow in [existing.clone(), desired.clone()] {
                responses.extend([
                    api.response("GET", 200, workflow),
                    api.response("PUT", 200, desired.clone()),
                ]);
            }
            let server = api.server(responses);
            let loader = api.loader(&server);

            for _ in 0..2 {
                let report = loader.load_report(vec![desired.clone()]).await;
                assert_eq!(report.counts().applied, 1, "{report:?}");
                assert_eq!(report.counts().failed, 0);
                assert_eq!(report.outcomes()[0].id(), "workflow-123");
                assert_eq!(report.outcomes()[0].family(), ResourceFamily::Workflows);
                assert_eq!(
                    report.outcomes()[0].operation(),
                    Some(ResourceOperation::Update)
                );
            }

            let requests = server.requests();
            assert!(requests.iter().all(|request| request.method != "HEAD"));
            assert!(requests.iter().all(|request| request.method != "POST"));
            let updates = requests
                .iter()
                .filter(|request| request.method == "PUT")
                .collect::<Vec<_>>();
            assert_eq!(updates.len(), 2);
            for update in updates {
                assert_eq!(update.path, api.item);
                assert_eq!(
                    serde_json::from_str::<Value>(&update.body).unwrap(),
                    desired
                );
            }
        }
    }

    #[tokio::test]
    async fn create_conflict_converges_by_updating_confirmed_workflow() {
        let desired = json!({
            "id": "workflow-123",
            "name": "desired-workflow",
            "yaml": "name: desired",
            "definition": {"name": "desired"}
        });
        for api in WORKFLOW_APIS {
            let server = api.server(vec![
                api.response("GET", 404, json!({})),
                api.response("POST", 409, json!({"message": "Workflow already exists"})),
                api.response(
                    "GET",
                    200,
                    json!({
                        "id": "workflow-123",
                        "readonly": false,
                        "definition": {"name": "created by another client"}
                    }),
                ),
                api.response("PUT", 200, desired.clone()),
            ]);
            let loader = api.loader(&server);

            let report = loader.load_report(vec![desired.clone()]).await;

            assert_eq!(report.counts().applied, 1, "{report:?}");
            assert_eq!(report.counts().failed, 0);
            assert_eq!(
                report.outcomes()[0].status(),
                ResourceOutcomeStatus::Applied
            );
            assert_eq!(
                report.outcomes()[0].operation(),
                Some(ResourceOperation::Update)
            );
            let requests = server.requests();
            assert_eq!(
                requests
                    .iter()
                    .map(|request| request.method.as_str())
                    .collect::<Vec<_>>(),
                ["GET", "GET", "POST", "GET", "PUT"]
            );
            assert_eq!(requests[4].path, api.item);
            assert_eq!(
                serde_json::from_str::<Value>(&requests[4].body).unwrap(),
                desired
            );
        }
    }

    #[tokio::test]
    async fn creates_missing_workflow_across_versions_and_spaces() {
        let desired = json!({
            "id": "workflow-123",
            "name": "desired-workflow",
            "yaml": "name: desired",
            "definition": {"name": "desired"}
        });
        for api in WORKFLOW_APIS {
            let server = api.server(vec![
                api.response("GET", 404, json!({})),
                api.response("POST", 201, desired.clone()),
            ]);

            let report = api.loader(&server).load_report(vec![desired.clone()]).await;

            assert_eq!(report.counts().applied, 1, "{report:?}");
            assert_eq!(report.counts().failed, 0);
            assert_eq!(
                report.outcomes()[0].operation(),
                Some(ResourceOperation::Create)
            );
            let requests = server.requests();
            assert_eq!(requests.len(), 3);
            assert_eq!(requests[2].method, "POST");
            assert_eq!(requests[2].path, api.collection);
            assert_eq!(
                serde_json::from_str::<Value>(&requests[2].body).unwrap(),
                desired
            );
        }
    }

    #[tokio::test]
    async fn rejects_readonly_workflow_across_versions_and_spaces() {
        for api in WORKFLOW_APIS {
            let server = api.server(vec![api.response(
                "GET",
                200,
                json!({"id": "workflow-123", "readonly": true}),
            )]);

            let report = api
                .loader(&server)
                .load_report(vec![json!({"id": "workflow-123", "readonly": false})])
                .await;

            assert_eq!(report.counts().applied, 0);
            assert_eq!(report.counts().failed, 1);
            assert_eq!(report.outcomes()[0].operation(), None);
            assert_eq!(
                report.outcomes()[0].detail(),
                Some("server-side Workflow is readonly")
            );
            assert_eq!(server.requests().len(), 2);
            assert!(
                server
                    .requests()
                    .iter()
                    .all(|request| request.method == "GET")
            );
        }
    }

    #[tokio::test]
    async fn create_conflict_fails_without_confirmed_writable_workflow() {
        for api in WORKFLOW_APIS {
            for (status, body, detail) in [
                (404, json!({}), "409 Conflict"),
                (403, json!({"message": "Forbidden"}), "403 Forbidden"),
                (
                    500,
                    json!({"message": "Unavailable"}),
                    "500 Internal Server Error",
                ),
                (204, json!({}), "Failed to parse existing Workflow"),
                (
                    200,
                    json!({"id": "workflow-123", "readonly": true}),
                    "server-side Workflow is readonly",
                ),
            ] {
                let server = api.server(vec![
                    api.response("GET", 404, json!({})),
                    api.response("POST", 409, json!({"message": "Workflow already exists"})),
                    api.response("GET", status, body),
                ]);

                let report = api
                    .loader(&server)
                    .load_report(vec![json!({"id": "workflow-123", "readonly": false})])
                    .await;

                assert_eq!(report.counts().applied, 0, "{report:?}");
                assert_eq!(report.counts().failed, 1);
                assert_eq!(report.outcomes()[0].status(), ResourceOutcomeStatus::Failed);
                assert_eq!(
                    report.outcomes()[0].operation(),
                    Some(ResourceOperation::Create)
                );
                let actual = report.outcomes()[0].detail().unwrap();
                assert!(actual.contains("409 Conflict"), "{actual}");
                assert!(actual.contains(detail), "{actual}");
                let requests = server.requests();
                assert_eq!(requests.len(), 4);
                assert!(requests.iter().all(|request| request.method != "PUT"));
            }
        }
    }

    #[tokio::test]
    async fn workflow_lookup_errors_do_not_trigger_mutation() {
        for api in WORKFLOW_APIS {
            for (status, body, detail) in [
                (
                    200,
                    json!(null),
                    "Failed to parse existing Workflow: expected a JSON object",
                ),
                (
                    200,
                    json!(["workflow-123"]),
                    "Failed to parse existing Workflow: expected a JSON object",
                ),
                (403, json!({}), "403 Forbidden"),
                (500, json!({}), "500 Internal Server Error"),
                (204, json!({}), "Failed to parse existing Workflow"),
            ] {
                let server = api.server(vec![api.response("GET", status, body)]);

                let report = api
                    .loader(&server)
                    .load_report(vec![json!({"id": "workflow-123"})])
                    .await;

                assert_eq!(report.counts().applied, 0);
                assert_eq!(report.counts().failed, 1);
                assert_eq!(report.outcomes()[0].operation(), None);
                assert!(report.outcomes()[0].detail().unwrap().contains(detail));
                assert_eq!(server.requests().len(), 2);
                assert!(
                    server
                        .requests()
                        .iter()
                        .all(|request| request.method == "GET")
                );
            }
        }
    }

    #[tokio::test]
    async fn failed_updates_remain_failed_after_create_conflict() {
        for api in WORKFLOW_APIS {
            for status in [403, 404, 409, 500] {
                let server = api.server(vec![
                    api.response("GET", 404, json!({})),
                    api.response("POST", 409, json!({"message": "Workflow already exists"})),
                    api.response("GET", 200, json!({"id": "workflow-123", "readonly": false})),
                    api.response("PUT", status, json!({"message": "Update rejected"})),
                ]);

                let report = api
                    .loader(&server)
                    .load_report(vec![json!({"id": "workflow-123"})])
                    .await;

                assert_eq!(report.counts().applied, 0);
                assert_eq!(report.counts().failed, 1);
                assert_eq!(
                    report.outcomes()[0].operation(),
                    Some(ResourceOperation::Update)
                );
                let detail = report.outcomes()[0].detail().unwrap();
                assert!(detail.contains(&status.to_string()), "{detail}");
                assert!(detail.contains("Update rejected"), "{detail}");
                assert_eq!(server.requests().len(), 5);
            }
        }
    }

    #[tokio::test]
    async fn non_conflict_create_errors_are_not_retried() {
        for api in WORKFLOW_APIS {
            let server = api.server(vec![
                api.response("GET", 404, json!({})),
                api.response("POST", 500, json!({"message": "Create failed"})),
            ]);

            let report = api
                .loader(&server)
                .load_report(vec![json!({"id": "workflow-123"})])
                .await;

            assert_eq!(report.counts().applied, 0);
            assert_eq!(report.counts().failed, 1);
            assert_eq!(
                report.outcomes()[0].operation(),
                Some(ResourceOperation::Create)
            );
            assert!(
                report.outcomes()[0]
                    .detail()
                    .unwrap()
                    .contains("Create failed")
            );
            assert_eq!(server.requests().len(), 3);
        }
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
                method: "GET",
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
        assert_eq!(requests[1].method, "GET");
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
                method: "GET",
                path: "/api/workflows/workflow/workflow-123",
                status: 200,
                body: json!({"id": "workflow-123", "readonly": false}),
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

        assert_eq!(report.counts().applied, 1);
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
                    "GET".to_string(),
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
    async fn fails_existing_server_side_readonly_workflow_without_mutation() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/api/status",
                status: 200,
                body: json!({"version": {"number": "9.4.1"}}),
            },
            MockResponse {
                method: "GET",
                path: "/api/workflows/workflow/system-workflow",
                status: 200,
                body: json!({"id": "system-workflow", "readonly": true}),
            },
        ]);
        let loader = WorkflowsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![json!({
                "id": "system-workflow",
                "name": "System Workflow",
                "yaml": "name: system"
            })])
            .await;

        assert_eq!(report.counts().failed, 1);
        assert_eq!(report.outcomes()[0].status(), ResourceOutcomeStatus::Failed);
        assert_eq!(
            report.outcomes()[0].detail(),
            Some("server-side Workflow is readonly")
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[1].method, "GET");
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
                method: "GET",
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
                ("GET".to_string(), "/api/workflows/workflow-123".to_string()),
                ("POST".to_string(), "/api/workflows".to_string())
            ]
        );
    }
}
