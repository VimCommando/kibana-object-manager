//! Skills API loader

use crate::client::KibanaClient;
use crate::etl::Loader;
use crate::standalone::{
    ResourceBatchReport, ResourceFamily, ResourceOperation, ResourceOutcome, ResourceOutcomeStatus,
};
use crate::{Error, Result};
use reqwest::Method;
use serde_json::Value;
use std::collections::HashMap;
use tokio::task::JoinSet;

pub struct SkillsLoader {
    client: KibanaClient,
}

impl SkillsLoader {
    pub fn new(client: KibanaClient) -> Self {
        Self { client }
    }

    pub async fn delete_skill(&self, skill_id: &str, force: bool) -> Result<()> {
        let path = if force {
            format!("api/agent_builder/skills/{skill_id}?force=true")
        } else {
            format!("api/agent_builder/skills/{skill_id}")
        };
        let response = self
            .client
            .request(Method::DELETE, &HashMap::new(), &path, None)
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }
        Ok(())
    }

    /// Apply Skills while preserving a structured outcome for every input item.
    pub async fn load_report(&self, items: Vec<Value>) -> ResourceBatchReport {
        let mut set = JoinSet::new();
        let mut outcomes = items
            .iter()
            .map(|skill| {
                ResourceOutcome::failed(
                    ResourceFamily::Skills,
                    resource_id(skill),
                    None,
                    "loader task did not complete",
                )
            })
            .collect::<Vec<_>>();

        for (index, skill) in items.into_iter().enumerate() {
            let client = self.client.clone();
            set.spawn(async move { (index, upsert_skill(client, skill).await) });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok((index, outcome)) => outcomes[index] = outcome,
                Err(error) => tracing::error!("Skill loader task panicked: {error}"),
            }
        }

        ResourceBatchReport::new(outcomes)
    }
}

impl Loader for SkillsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let report = self.load_report(items).await;
        for outcome in report.outcomes() {
            if outcome.status() == ResourceOutcomeStatus::Failed {
                tracing::error!(
                    "Failed to load skill '{}': {}",
                    outcome.id(),
                    outcome.detail().unwrap_or("unknown failure")
                );
            }
        }

        Ok(report.counts().applied)
    }
}

async fn upsert_skill(client: KibanaClient, skill: Value) -> ResourceOutcome {
    let Some(skill_id) = skill
        .get("id")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
    else {
        return ResourceOutcome::failed(
            ResourceFamily::Skills,
            "<missing>",
            None,
            Error::MissingResourceId { resource: "skill" }.to_string(),
        );
    };

    if is_readonly(&skill) {
        tracing::debug!("Skipping readonly skill: {}", skill_id);
        return ResourceOutcome::skipped(
            ResourceFamily::Skills,
            skill_id,
            "local Skill is readonly",
        );
    }

    let existing = match existing_skill(&client, &skill_id).await {
        Ok(existing) => existing,
        Err(error) => {
            return ResourceOutcome::failed(
                ResourceFamily::Skills,
                skill_id,
                None,
                error.to_string(),
            );
        }
    };

    if let Some(existing) = existing {
        if is_readonly(&existing) {
            tracing::debug!("Skipping server-side readonly skill: {}", skill_id);
            return ResourceOutcome::failed(
                ResourceFamily::Skills,
                skill_id,
                None,
                "server-side Skill is readonly",
            );
        }

        let body = sanitized_skill_body(&skill, false);
        let path = format!("api/agent_builder/skills/{skill_id}");
        let response = match client.put_json_value(&path, &body).await {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Skills,
                    skill_id,
                    Some(ResourceOperation::Update),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Skills,
                skill_id,
                Some(ResourceOperation::Update),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Updated skill: {}", skill_id);
        ResourceOutcome::applied(ResourceFamily::Skills, skill_id, ResourceOperation::Update)
    } else {
        let body = sanitized_skill_body(&skill, true);
        let response = match client
            .post_json_value("api/agent_builder/skills", &body)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return ResourceOutcome::failed(
                    ResourceFamily::Skills,
                    skill_id,
                    Some(ResourceOperation::Create),
                    error.to_string(),
                );
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return ResourceOutcome::failed(
                ResourceFamily::Skills,
                skill_id,
                Some(ResourceOperation::Create),
                Error::api_response(status, body).to_string(),
            );
        }
        tracing::info!("Created skill: {}", skill_id);
        ResourceOutcome::applied(ResourceFamily::Skills, skill_id, ResourceOperation::Create)
    }
}

fn resource_id(resource: &Value) -> &str {
    resource
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<missing>")
}

async fn existing_skill(client: &KibanaClient, skill_id: &str) -> Result<Option<Value>> {
    let path = format!("api/agent_builder/skills/{skill_id}");
    let response = client.get(&path).await?;
    match response.status().as_u16() {
        200 => Ok(Some(response.json().await?)),
        404 => Ok(None),
        _ => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Error::api_response(status, body))
        }
    }
}

fn is_readonly(skill: &Value) -> bool {
    skill.get("readonly").and_then(|value| value.as_bool()) == Some(true)
}

fn sanitized_skill_body(skill: &Value, include_id: bool) -> Value {
    let mut body = skill.clone();
    let Some(object) = body.as_object_mut() else {
        return body;
    };

    if !include_id {
        object.remove("id");
    }

    for field in [
        "readonly",
        "schema",
        "type",
        "built_in",
        "source",
        "created_at",
        "updated_at",
        "experimental",
    ] {
        object.remove(field);
    }

    if !matches!(object.get("tool_ids"), Some(Value::Array(_))) {
        object.insert("tool_ids".to_string(), Value::Array(Vec::new()));
    }
    if !matches!(object.get("referenced_content"), Some(Value::Array(_))) {
        object.insert("referenced_content".to_string(), Value::Array(Vec::new()));
    }

    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;

    #[test]
    fn update_body_removes_id_and_readonly() {
        let body = sanitized_skill_body(
            &json!({
                "id": "skill-a",
                "name": "Skill A",
                "readonly": true,
                "experimental": true,
                "tool_ids": [],
                "referenced_content": []
            }),
            false,
        );

        assert!(body.get("id").is_none());
        assert!(body.get("readonly").is_none());
        assert!(body.get("experimental").is_none());
        assert_eq!(body["tool_ids"], json!([]));
        assert_eq!(body["referenced_content"], json!([]));
    }

    #[test]
    fn create_body_keeps_id_and_adds_empty_arrays() {
        let body = sanitized_skill_body(
            &json!({
                "id": "skill-a",
                "name": "Skill A"
            }),
            true,
        );

        assert_eq!(body["id"], "skill-a");
        assert_eq!(body["tool_ids"], json!([]));
        assert_eq!(body["referenced_content"], json!([]));
    }

    #[test]
    fn body_replaces_non_array_fields_with_empty_arrays() {
        let body = sanitized_skill_body(
            &json!({
                "id": "skill-a",
                "name": "Skill A",
                "tool_ids": null,
                "referenced_content": null
            }),
            true,
        );

        assert_eq!(body["tool_ids"], json!([]));
        assert_eq!(body["referenced_content"], json!([]));
    }

    #[tokio::test]
    async fn creates_missing_skill_with_post_body() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/s/esdiag/api/agent_builder/skills/skill-a",
                status: 404,
                body: json!({"error": "not found"}),
            },
            MockResponse {
                method: "POST",
                path: "/s/esdiag/api/agent_builder/skills",
                status: 200,
                body: json!({"id": "skill-a"}),
            },
        ]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        let count = loader
            .load(vec![json!({
                "id": "skill-a",
                "name": "Skill A",
                "readonly": false,
                "schema": "system"
            })])
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.requests();
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[1].method, "POST");
        assert_eq!(
            requests[1].headers.get("kbn-xsrf").map(String::as_str),
            Some("true")
        );
        let body: Value = serde_json::from_str(&requests[1].body).unwrap();
        assert_eq!(body["id"], "skill-a");
        assert!(body.get("readonly").is_none());
        assert!(body.get("schema").is_none());
        assert_eq!(body["tool_ids"], json!([]));
        assert_eq!(body["referenced_content"], json!([]));
    }

    #[tokio::test]
    async fn updates_existing_skill_without_id() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/s/esdiag/api/agent_builder/skills/skill-a",
                status: 200,
                body: json!({"id": "skill-a"}),
            },
            MockResponse {
                method: "PUT",
                path: "/s/esdiag/api/agent_builder/skills/skill-a",
                status: 200,
                body: json!({"id": "skill-a"}),
            },
        ]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        let count = loader
            .load(vec![json!({
                "id": "skill-a",
                "name": "Skill A",
                "readonly": false
            })])
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.requests();
        assert_eq!(requests[1].method, "PUT");
        let body: Value = serde_json::from_str(&requests[1].body).unwrap();
        assert!(body.get("id").is_none());
        assert!(body.get("readonly").is_none());
        assert_eq!(body["tool_ids"], json!([]));
        assert_eq!(body["referenced_content"], json!([]));
    }

    #[tokio::test]
    async fn skips_existing_server_side_readonly_skill() {
        let server = TestServer::new(vec![MockResponse {
            method: "GET",
            path: "/s/esdiag/api/agent_builder/skills/system-skill",
            status: 200,
            body: json!({"id": "system-skill", "readonly": true}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        let report = loader
            .load_report(vec![json!({
                "id": "system-skill",
                "name": "System Skill"
            })])
            .await;

        assert_eq!(report.counts().failed, 1);
        assert_eq!(report.outcomes()[0].status(), ResourceOutcomeStatus::Failed);
        assert!(
            report.outcomes()[0]
                .detail()
                .unwrap()
                .contains("server-side Skill is readonly")
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
    }

    #[tokio::test]
    async fn reports_partial_failure_in_validated_input_order() {
        let server = TestServer::new(vec![
            MockResponse {
                method: "GET",
                path: "/s/esdiag/api/agent_builder/skills/skill-a",
                status: 404,
                body: json!({"error": "not found"}),
            },
            MockResponse {
                method: "POST",
                path: "/s/esdiag/api/agent_builder/skills",
                status: 409,
                body: json!({"message": "create conflict"}),
            },
        ]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        let report = loader
            .load_report(vec![
                json!({"id": "skill-a", "name": "Skill A"}),
                json!({"readonly": true, "name": "Missing ID"}),
            ])
            .await;

        assert_eq!(
            report
                .outcomes()
                .iter()
                .map(ResourceOutcome::id)
                .collect::<Vec<_>>(),
            vec!["skill-a", "<missing>"]
        );
        assert_eq!(
            report.outcomes()[0].operation(),
            Some(ResourceOperation::Create)
        );
        assert!(
            report.outcomes()[0]
                .detail()
                .unwrap()
                .contains("create conflict")
        );
        assert_eq!(
            report.counts(),
            crate::standalone::ResourceBatchCounts {
                attempted: 2,
                applied: 0,
                skipped: 0,
                failed: 2,
            }
        );
    }

    #[tokio::test]
    async fn reports_skipped_items_in_original_order_without_requests() {
        let server = TestServer::new(Vec::new());
        let loader = SkillsLoader::new(server.client().unwrap());

        let report = loader
            .load_report(vec![
                json!({"id": "z-skill", "readonly": true}),
                json!({"id": "a-skill", "readonly": true}),
            ])
            .await;

        assert_eq!(
            report
                .outcomes()
                .iter()
                .map(ResourceOutcome::id)
                .collect::<Vec<_>>(),
            vec!["z-skill", "a-skill"]
        );
        assert_eq!(report.counts().skipped, 2);
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn deletes_skill_without_force() {
        let server = TestServer::new(vec![MockResponse {
            method: "DELETE",
            path: "/s/esdiag/api/agent_builder/skills/skill-a",
            status: 200,
            body: json!({}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        loader.delete_skill("skill-a", false).await.unwrap();

        assert_eq!(server.requests()[0].method, "DELETE");
    }

    #[tokio::test]
    async fn deletes_skill_with_force_query() {
        let server = TestServer::new(vec![MockResponse {
            method: "DELETE",
            path: "/s/esdiag/api/agent_builder/skills/skill-a?force=true",
            status: 200,
            body: json!({}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        loader.delete_skill("skill-a", true).await.unwrap();

        assert_eq!(
            server.requests()[0].path,
            "/s/esdiag/api/agent_builder/skills/skill-a?force=true"
        );
    }

    #[tokio::test]
    async fn delete_conflict_preserves_response_details() {
        let server = TestServer::new(vec![MockResponse {
            method: "DELETE",
            path: "/s/esdiag/api/agent_builder/skills/skill-a",
            status: 409,
            body: json!({"message": "skill is referenced by agents"}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let loader = SkillsLoader::new(client);

        let err = loader.delete_skill("skill-a", false).await.unwrap_err();

        let message = err.to_string();
        assert!(message.contains("409 Conflict"));
        assert!(message.contains("skill is referenced by agents"));
    }
}
