//! Saved Objects API extractor
//!
//! Extracts saved objects from Kibana via POST /api/saved_objects/_export

use crate::client::Kibana;
use crate::etl::Extractor;
use async_trait::async_trait;
use eyre::{Context, Result};
use serde_json::Value;

/// Extractor for Kibana saved objects
///
/// Exports saved objects from Kibana using the manifest as the export specification.
/// The manifest defines which objects to export and is sent as the POST body.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::saved_objects::{SavedObjectsExtractor, SavedObjectsManifest, SavedObject};
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Extractor;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None, None)?;
/// let mut manifest = SavedObjectsManifest::new();
/// manifest.add_object(SavedObject::new("dashboard", "my-dashboard-id"));
///
/// let extractor = SavedObjectsExtractor::new(client, manifest, "default");
/// let objects = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct SavedObjectsExtractor {
    client: Kibana,
    manifest: super::SavedObjectsManifest,
    space: String,
}

impl SavedObjectsExtractor {
    /// Create a new saved objects extractor
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    /// * `manifest` - Manifest defining which objects to export
    /// * `space` - Space ID to export from (default: "default")
    pub fn new(
        client: Kibana,
        manifest: super::SavedObjectsManifest,
        space: impl Into<String>,
    ) -> Self {
        Self {
            client,
            manifest,
            space: space.into(),
        }
    }

    /// Export saved objects from Kibana
    ///
    /// POSTs the manifest to /api/saved_objects/_export and receives
    /// NDJSON response containing the exported objects.
    async fn export_objects(&self) -> Result<Vec<Value>> {
        // The client will automatically add the space prefix
        let path = "/api/saved_objects/_export";

        log::debug!(
            "Exporting {} object(s) from space '{}'",
            self.manifest.count(),
            self.space
        );

        let response = self
            .client
            .post_json_value(path, &serde_json::to_value(&self.manifest)?)
            .await
            .with_context(|| "Failed to export saved objects from Kibana")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!("Failed to export saved objects ({}): {}", status, body);
        }

        // Kibana returns NDJSON format
        let body = response
            .text()
            .await
            .with_context(|| "Failed to read export response")?;

        let objects: Vec<Value> = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line))
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| "Failed to parse NDJSON response")?;

        log::info!("Exported {} object(s) from Kibana", objects.len());

        Ok(objects)
    }
}

#[async_trait]
impl Extractor for SavedObjectsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        let objects = self.export_objects().await?;

        log::info!("Extracted {} saved object(s)", objects.len());

        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Auth;
    use crate::kibana::saved_objects::SavedObjectsManifest;
    use crate::kibana::saved_objects::manifest::SavedObject;
    use url::Url;

    #[test]
    fn test_extractor_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let manifest = SavedObjectsManifest::new();
        let extractor = SavedObjectsExtractor::new(client, manifest, "default");
        assert_eq!(extractor.space, "default");
    }

    #[test]
    fn test_extractor_with_manifest() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let mut manifest = SavedObjectsManifest::new();
        manifest.add_object(SavedObject::new("dashboard", "test-dashboard"));
        manifest.add_object(SavedObject::new("visualization", "test-viz"));

        let extractor = SavedObjectsExtractor::new(client, manifest.clone(), "marketing");
        assert_eq!(extractor.space, "marketing");
        assert_eq!(extractor.manifest.count(), 2);
    }
}
