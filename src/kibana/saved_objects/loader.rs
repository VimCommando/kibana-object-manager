//! Saved Objects API loader
//!
//! Loads saved objects to Kibana via POST /api/saved_objects/_import

use crate::client::Kibana;
use crate::etl::Loader;
use async_trait::async_trait;
use eyre::{Context, Result};
use serde_json::Value;

/// Loader for Kibana saved objects
///
/// Imports saved objects into Kibana using the import API.
/// Objects are sent as NDJSON in a multipart form.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::saved_objects::SavedObjectsLoader;
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None, None)?;
/// let loader = SavedObjectsLoader::new(client, "default");
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
    client: Kibana,
    space: String,
    overwrite: bool,
}

impl SavedObjectsLoader {
    /// Create a new saved objects loader
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    /// * `space` - Space ID to import into (default: "default")
    pub fn new(client: Kibana, space: impl Into<String>) -> Self {
        Self {
            client,
            space: space.into(),
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
            .map(|obj| serde_json::to_string(obj))
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| "Failed to serialize objects to NDJSON")?
            .join("\n");

        // The client will automatically add the space prefix
        let path = format!("/api/saved_objects/_import?overwrite={}", self.overwrite);

        log::debug!(
            "Importing {} object(s) to space '{}'",
            objects.len(),
            self.space
        );

        let response = self
            .client
            .post_form(&path, ndjson.as_bytes())
            .await
            .with_context(|| "Failed to import saved objects to Kibana")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!("Failed to import saved objects ({}): {}", status, body);
        }

        log::info!("Imported {} object(s) to Kibana", objects.len());

        Ok(())
    }
}

#[async_trait]
impl Loader for SavedObjectsLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        if items.is_empty() {
            log::info!("No saved objects to import");
            return Ok(0);
        }

        self.import_objects(&items).await?;

        log::info!("Loaded {} saved object(s) to Kibana", items.len());
        Ok(items.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Auth;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let loader = SavedObjectsLoader::new(client, "default");
        assert_eq!(loader.space, "default");
        assert!(loader.overwrite);
    }

    #[test]
    fn test_with_overwrite() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let loader = SavedObjectsLoader::new(client, "default").with_overwrite(false);
        assert!(!loader.overwrite);
    }

    #[test]
    fn test_custom_space() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let loader = SavedObjectsLoader::new(client, "marketing");
        assert_eq!(loader.space, "marketing");
    }
}
