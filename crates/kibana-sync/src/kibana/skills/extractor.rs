//! Skills API extractor

use crate::client::KibanaClient;
use crate::etl::Extractor;
use crate::{Error, Result, ResultContext};
use serde_json::Value;
use tokio::task::JoinSet;

pub struct SkillsExtractor {
    client: KibanaClient,
    skill_ids: Option<Vec<String>>,
}

impl SkillsExtractor {
    pub fn new(client: KibanaClient, skill_ids: Option<Vec<String>>) -> Self {
        Self { client, skill_ids }
    }

    pub async fn search_skills(&self, include_plugins: bool) -> Result<Vec<Value>> {
        let path = if include_plugins {
            "api/agent_builder/skills?include_plugins=true"
        } else {
            "api/agent_builder/skills"
        };
        let response = self
            .client
            .get(path)
            .await
            .context("Failed to fetch skills")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        let value: Value = response
            .json()
            .await
            .context("Failed to parse skills response")?;
        parse_skills_response(value)
    }

    pub async fn fetch_skill(&self, skill_id: &str) -> Result<Value> {
        let path = format!("api/agent_builder/skills/{skill_id}");
        let response = self
            .client
            .get(&path)
            .await
            .with_context(|| format!("Failed to fetch skill '{skill_id}'"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        response
            .json()
            .await
            .with_context(|| format!("Failed to parse skill '{skill_id}' response"))
    }

    async fn fetch_selected_skills(&self, skill_ids: &[String]) -> Result<Vec<Value>> {
        let mut skills = Vec::new();
        let mut set = JoinSet::new();

        for skill_id in skill_ids {
            let client = self.client.clone();
            let skill_id = skill_id.clone();
            set.spawn(async move {
                let path = format!("api/agent_builder/skills/{skill_id}");
                let response = client
                    .get(&path)
                    .await
                    .with_context(|| format!("Failed to fetch skill '{skill_id}'"))?;
                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Err(Error::api_response(status, body));
                }
                Ok::<Value, Error>(response.json().await?)
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok(skill)) => skills.push(skill),
                Ok(Err(err)) => tracing::warn!("{}", err),
                Err(err) => tracing::error!("Task panicked: {}", err),
            }
        }

        Ok(skills)
    }
}

impl Extractor for SkillsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        if let Some(skill_ids) = &self.skill_ids {
            self.fetch_selected_skills(skill_ids).await
        } else {
            tracing::warn!("No skill IDs provided - use search_skills to discover skills");
            Ok(Vec::new())
        }
    }
}

pub fn parse_skills_response(value: Value) -> Result<Vec<Value>> {
    if let Some(results) = value.get("results").and_then(|value| value.as_array()) {
        return Ok(results.clone());
    }

    if let Some(results) = value.as_array() {
        return Ok(results.clone());
    }

    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;

    #[test]
    fn parses_results_array_response() {
        let parsed = parse_skills_response(json!({
            "results": [
                {"id": "skill-a"},
                {"id": "skill-b"}
            ]
        }))
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["id"], "skill-a");
    }

    #[test]
    fn parses_top_level_array_response() {
        let parsed = parse_skills_response(json!([
            {"id": "skill-a"},
            {"id": "skill-b"}
        ]))
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1]["id"], "skill-b");
    }

    #[tokio::test]
    async fn lists_skills_from_space_endpoint() {
        let server = TestServer::new(vec![MockResponse {
            method: "GET",
            path: "/s/esdiag/api/agent_builder/skills",
            status: 200,
            body: json!({"results": [{"id": "skill-a"}]}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let extractor = SkillsExtractor::new(client, None);

        let skills = extractor.search_skills(false).await.unwrap();

        assert_eq!(skills, vec![json!({"id": "skill-a"})]);
        assert_eq!(
            server.requests()[0].headers.get("kbn-xsrf").unwrap(),
            "true"
        );
    }

    #[tokio::test]
    async fn lists_plugin_skills_with_include_plugins_query() {
        let server = TestServer::new(vec![MockResponse {
            method: "GET",
            path: "/s/esdiag/api/agent_builder/skills?include_plugins=true",
            status: 200,
            body: json!({"results": [{"id": "plugin-skill", "readonly": true}]}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let extractor = SkillsExtractor::new(client, None);

        let skills = extractor.search_skills(true).await.unwrap();

        assert_eq!(
            skills,
            vec![json!({"id": "plugin-skill", "readonly": true})]
        );
    }

    #[tokio::test]
    async fn fetches_skill_by_id() {
        let server = TestServer::new(vec![MockResponse {
            method: "GET",
            path: "/s/esdiag/api/agent_builder/skills/skill-a",
            status: 200,
            body: json!({"id": "skill-a", "content": "Body"}),
        }]);
        let client = server.client().unwrap().space("esdiag").unwrap();
        let extractor = SkillsExtractor::new(client, None);

        let skill = extractor.fetch_skill("skill-a").await.unwrap();

        assert_eq!(skill, json!({"id": "skill-a", "content": "Body"}));
    }
}
