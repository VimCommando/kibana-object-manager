//! Skills API loader

use crate::client::KibanaClient;
use crate::etl::Loader;
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
}

impl Loader for SkillsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;
        let mut set = JoinSet::new();

        for skill in items {
            let client = self.client.clone();
            set.spawn(async move { upsert_skill(client, skill).await });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok(loaded)) => {
                    if loaded {
                        count += 1;
                    }
                }
                Ok(Err(err)) => tracing::error!("Failed to load skill: {}", err),
                Err(err) => tracing::error!("Task panicked: {}", err),
            }
        }

        Ok(count)
    }
}

async fn upsert_skill(client: KibanaClient, skill: Value) -> Result<bool> {
    let skill_id = skill
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or(Error::MissingResourceId { resource: "skill" })?;

    if is_readonly(&skill) {
        tracing::debug!("Skipping readonly skill: {}", skill_id);
        return Ok(false);
    }

    let exists = skill_exists(&client, skill_id).await?;

    if exists {
        let body = sanitized_skill_body(&skill, false);
        let path = format!("api/agent_builder/skills/{skill_id}");
        let response = client.put_json_value(&path, &body).await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }
        tracing::info!("Updated skill: {}", skill_id);
    } else {
        let body = sanitized_skill_body(&skill, true);
        let response = client
            .post_json_value("api/agent_builder/skills", &body)
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }
        tracing::info!("Created skill: {}", skill_id);
    }

    Ok(true)
}

async fn skill_exists(client: &KibanaClient, skill_id: &str) -> Result<bool> {
    let path = format!("api/agent_builder/skills/{skill_id}");
    let response = client.get(&path).await?;
    match response.status().as_u16() {
        200 => Ok(true),
        404 => Ok(false),
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

    object
        .entry("tool_ids".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    object
        .entry("referenced_content".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));

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
