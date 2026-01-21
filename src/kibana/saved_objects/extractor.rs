//! Saved Objects API extractor
//!
//! Extracts saved objects from Kibana via POST /api/saved_objects/_export

use crate::client::KibanaClient;
use crate::etl::Extractor;

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
/// use kibana_object_manager::client::{Auth, KibanaClient};
/// use kibana_object_manager::etl::Extractor;
/// use url::Url;
/// use std::path::Path;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::try_new(url, Auth::None, Path::new("."))?;
/// let space_client = client.space("default")?;
/// let mut manifest = SavedObjectsManifest::new();
/// manifest.add_object(SavedObject::new("dashboard", "my-dashboard-id"));
///
/// let extractor = SavedObjectsExtractor::new(space_client, manifest);
/// let objects = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct SavedObjectsExtractor {
    client: KibanaClient,
    manifest: super::SavedObjectsManifest,
}

impl SavedObjectsExtractor {
    /// Create a new saved objects extractor
    ///
    /// # Arguments
    /// * `client` - Space-scoped Kibana client
    /// * `manifest` - Manifest defining which objects to export
    pub fn new(client: KibanaClient, manifest: super::SavedObjectsManifest) -> Self {
        Self { client, manifest }
    }

    /// Export saved objects from Kibana
    ///
    /// POSTs the manifest to /api/saved_objects/_export and receives
    /// NDJSON response containing the exported objects.
    async fn export_objects(&self) -> Result<Vec<Value>> {
        let path = "api/saved_objects/_export";

        log::debug!(
            "Exporting {} object(s) from space '{}'",
            self.manifest.count(),
            self.client.space_id()
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
    use crate::client::{Auth, KibanaClient};
    use crate::kibana::saved_objects::SavedObjectsManifest;
    use crate::kibana::saved_objects::manifest::SavedObject;
    use tempfile::TempDir;
    use url::Url;

    #[test]
    fn test_extractor_creation() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();
        let space_client = client.space("default").unwrap();
        let manifest = SavedObjectsManifest::new();
        let extractor = SavedObjectsExtractor::new(space_client, manifest);
        assert_eq!(extractor.client.space_id(), "default");
    }

    #[test]
    fn test_extractor_with_manifest() {
        let temp_dir = TempDir::new().unwrap();
        // Create spaces.yml with marketing space
        let manifest = crate::kibana::spaces::SpacesManifest::with_spaces(vec![
            crate::kibana::spaces::SpaceEntry::new("default".to_string(), "Default".to_string()),
            crate::kibana::spaces::SpaceEntry::new(
                "marketing".to_string(),
                "Marketing".to_string(),
            ),
        ]);
        manifest.write(temp_dir.path().join("spaces.yml")).unwrap();

        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();
        let space_client = client.space("marketing").unwrap();

        let mut so_manifest = SavedObjectsManifest::new();
        so_manifest.add_object(SavedObject::new("dashboard", "test-dashboard"));
        so_manifest.add_object(SavedObject::new("visualization", "test-viz"));

        let extractor = SavedObjectsExtractor::new(space_client, so_manifest.clone());
        assert_eq!(extractor.client.space_id(), "marketing");
        assert_eq!(extractor.manifest.count(), 2);
    }
}
