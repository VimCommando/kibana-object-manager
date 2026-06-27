//! Saved Objects API loader
//!
//! Loads saved objects to Kibana via POST /api/saved_objects/_import

use crate::client::KibanaClient;
use crate::etl::Loader;

use crate::{Error, Result, ResultContext};
use serde_json::Value;

/// Loader for Kibana saved objects
///
/// Imports saved objects into Kibana using the import API.
/// Objects are sent as NDJSON in a multipart form.
///
/// # Example
/// ```no_run
/// use kibana_client::kibana::saved_objects::SavedObjectsLoader;
/// use kibana_client::client::{Auth, KibanaClient};
/// use kibana_client::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_client::Result<()> {
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
        // Convert objects to NDJSON
        let ndjson = objects
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

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        tracing::info!("Imported {} object(s) to Kibana", objects.len());

        Ok(())
    }
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
}
