//! Saved Objects API loader
//!
//! Loads saved objects to Kibana via POST /api/saved_objects/_import

use crate::client::KibanaClient;
use crate::etl::Loader;

use crate::{Error, Result, ResultContext};
use serde::Deserialize;
use serde_json::Value;

const LEGACY_JSON_FIELDS: &[&str] = &[
    "attributes.panelsJSON",
    "attributes.fieldFormatMap",
    "attributes.controlGroupInput.ignoreParentSettingsJSON",
    "attributes.controlGroupInput.panelsJSON",
    "attributes.kibanaSavedObjectMeta.searchSourceJSON",
    "attributes.optionsJSON",
    "attributes.visState",
    "attributes.fieldAttrs",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    errors: Vec<Value>,
}

/// Loader for Kibana saved objects
///
/// Imports saved objects into Kibana using the import API.
/// Objects are sent as NDJSON in a multipart form.
///
/// # Example
/// ```no_run
/// use kibana_sync::kibana::saved_objects::SavedObjectsLoader;
/// use kibana_sync::client::{Auth, KibanaClient};
/// use kibana_sync::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_sync::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
/// let space_client = client.space("default")?;
/// let loader = SavedObjectsLoader::new(space_client);
///
/// let objects = vec![
///     json!({
///         "type": "dashboard",
///         "id": "my-dashboard",
///         "attributes": {"title": "My Dashboard"}
///     })
/// ];
///
/// let count = loader.load(objects).await?;
/// # Ok(())
/// # }
/// ```
pub struct SavedObjectsLoader {
    client: KibanaClient,
    overwrite: bool,
}

impl SavedObjectsLoader {
    /// Create a new saved objects loader
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    pub fn new(client: KibanaClient) -> Self {
        Self {
            client,
            overwrite: true,
        }
    }

    /// Set whether to overwrite existing objects (default: true)
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Import saved objects into Kibana
    ///
    /// Converts objects to NDJSON and uploads via multipart form.
    async fn import_objects(&self, objects: &[Value]) -> Result<()> {
        // Kibana requires legacy structured fields to be serialized JSON strings.
        let ndjson = objects
            .iter()
            .cloned()
            .map(serialize_legacy_json_fields)
            .collect::<Result<Vec<_>>>()?
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, serde_json::Error>>()
            .with_context(|| "Failed to serialize objects to NDJSON")?
            .join("\n");

        let path = format!("api/saved_objects/_import?overwrite={}", self.overwrite);

        tracing::debug!(
            "Importing {} object(s) to space '{}'",
            objects.len(),
            self.client.space_id()
        );

        let response = self
            .client
            .post_form(&path, ndjson.as_bytes())
            .await
            .with_context(|| "Failed to import saved objects to Kibana")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        let body = response
            .text()
            .await
            .with_context(|| "Failed to read saved object import response")?;
        let result: ImportResponse = serde_json::from_str(&body)
            .with_context(|| "Failed to parse saved object import response")?;
        if !result.success || !result.errors.is_empty() {
            return Err(Error::api_response(status, body));
        }

        tracing::info!("Imported {} object(s) to Kibana", objects.len());

        Ok(())
    }
}

fn serialize_legacy_json_fields(mut object: Value) -> Result<Value> {
    for field_path in LEGACY_JSON_FIELDS {
        if let Some(field) = get_nested_mut(&mut object, field_path)
            && (field.is_object() || field.is_array())
        {
            *field = Value::String(
                serde_json::to_string(field)
                    .with_context(|| format!("Failed to serialize legacy field: {field_path}"))?,
            );
        }
    }
    Ok(object)
}

fn get_nested_mut<'a>(object: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut current = object;
    for field in path.split('.') {
        current = current.get_mut(field)?;
    }
    Some(current)
}

impl Loader for SavedObjectsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        if items.is_empty() {
            tracing::info!("No saved objects to import");
            return Ok(0);
        }

        self.import_objects(&items).await?;

        tracing::info!("Loaded {} saved object(s) to Kibana", items.len());
        Ok(items.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Auth, KibanaClient};
    use crate::etl::Loader;
    use crate::test_support::{MockResponse, TestServer};
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = SavedObjectsLoader::new(space_client);
        assert!(loader.overwrite);
    }

    #[test]
    fn test_with_overwrite() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let space_client = client.space("default").unwrap();
        let loader = SavedObjectsLoader::new(space_client).with_overwrite(false);
        assert!(!loader.overwrite);
    }

    #[test]
    fn test_custom_space() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::builder(url)
            .spaces([
                ("default".to_string(), "Default".to_string()),
                ("marketing".to_string(), "Marketing".to_string()),
            ])
            .build()
            .unwrap();
        let space_client = client.space("marketing").unwrap();
        let loader = SavedObjectsLoader::new(space_client);
        assert_eq!(loader.client.space_id(), "marketing");
    }

    #[test]
    fn serializes_structured_legacy_fields_without_reescaping_strings() {
        let object = json!({
            "attributes": {
                "panelsJSON": [{"panelIndex": "1"}],
                "kibanaSavedObjectMeta": {
                    "searchSourceJSON": {"query": {"language": "kuery"}}
                },
                "visState": {"type": "pie"},
                "optionsJSON": "{\"useMargins\":true}"
            }
        });

        let serialized = serialize_legacy_json_fields(object).unwrap();
        let attributes = &serialized["attributes"];
        assert!(attributes["panelsJSON"].is_string());
        assert!(attributes["kibanaSavedObjectMeta"]["searchSourceJSON"].is_string());
        assert!(attributes["visState"].is_string());
        assert_eq!(attributes["optionsJSON"], "{\"useMargins\":true}");
        assert_eq!(
            serde_json::from_str::<Value>(attributes["panelsJSON"].as_str().unwrap()).unwrap(),
            json!([{"panelIndex": "1"}])
        );
    }

    #[tokio::test]
    async fn import_serializes_legacy_fields_in_ndjson() {
        let server = TestServer::new(vec![MockResponse {
            method: "POST",
            path: "/api/saved_objects/_import?overwrite=true",
            status: 200,
            body: json!({"success": true, "successCount": 1, "errors": []}),
        }]);
        let loader = SavedObjectsLoader::new(server.client().unwrap().space("default").unwrap());

        loader
            .load(vec![json!({
                "type": "dashboard",
                "id": "dashboard-1",
                "attributes": {
                    "panelsJSON": [{"panelIndex": "1"}],
                    "kibanaSavedObjectMeta": {
                        "searchSourceJSON": {"query": {"language": "kuery"}}
                    },
                    "visState": {"type": "pie"}
                }
            })])
            .await
            .unwrap();

        let request = &server.requests()[0];
        let (_, ndjson) = request.body.split_once("\r\n\r\n").unwrap();
        let (ndjson, _) = ndjson.split_once("\r\n--").unwrap();
        let imported: Value = serde_json::from_str(ndjson).unwrap();
        assert!(imported["attributes"]["panelsJSON"].is_string());
        assert!(imported["attributes"]["kibanaSavedObjectMeta"]["searchSourceJSON"].is_string());
        assert!(imported["attributes"]["visState"].is_string());
    }

    #[tokio::test]
    async fn import_fails_when_kibana_reports_item_errors() {
        let server = TestServer::new(vec![MockResponse {
            method: "POST",
            path: "/api/saved_objects/_import?overwrite=true",
            status: 200,
            body: json!({
                "success": true,
                "successCount": 1,
                "errors": [{"id": "dashboard-1", "type": "dashboard", "error": {"message": "invalid panelsJSON"}}]
            }),
        }]);
        let loader = SavedObjectsLoader::new(server.client().unwrap().space("default").unwrap());

        let error = loader
            .load(vec![json!({"type": "dashboard", "id": "dashboard-1"})])
            .await
            .unwrap_err();

        assert!(error.to_string().contains("invalid panelsJSON"));
    }

    #[tokio::test]
    async fn import_fails_when_kibana_reports_unsuccessful_response() {
        let server = TestServer::new(vec![MockResponse {
            method: "POST",
            path: "/api/saved_objects/_import?overwrite=true",
            status: 200,
            body: json!({"success": false, "successCount": 0, "errors": []}),
        }]);
        let loader = SavedObjectsLoader::new(server.client().unwrap().space("default").unwrap());

        let error = loader
            .load(vec![json!({"type": "dashboard", "id": "dashboard-1"})])
            .await
            .unwrap_err();

        assert!(error.to_string().contains("\"success\":false"));
    }
}
