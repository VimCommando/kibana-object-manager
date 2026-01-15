//! Spaces API extractor
//!
//! Extracts space definitions from Kibana via GET /api/spaces/space

use crate::client::Kibana;
use crate::etl::Extractor;
use async_trait::async_trait;
use eyre::{Context, Result};
use serde_json::Value;

/// Extractor for Kibana spaces
///
/// Fetches all spaces from Kibana and filters them based on the manifest.
/// If no manifest is provided, all spaces are extracted.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::spaces::{SpacesExtractor, SpacesManifest};
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Extractor;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None, None)?;
/// let manifest = SpacesManifest::with_spaces(vec!["default".to_string(), "marketing".to_string()]);
///
/// let extractor = SpacesExtractor::new(client, Some(manifest));
/// let spaces = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct SpacesExtractor {
    client: Kibana,
    manifest: Option<super::SpacesManifest>,
}

impl SpacesExtractor {
    /// Create a new spaces extractor
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    /// * `manifest` - Optional manifest to filter which spaces to extract
    pub fn new(client: Kibana, manifest: Option<super::SpacesManifest>) -> Self {
        Self { client, manifest }
    }

    /// Create an extractor that fetches all spaces
    pub fn all(client: Kibana) -> Self {
        Self::new(client, None)
    }

    /// Fetch all spaces from Kibana
    async fn fetch_all_spaces(&self) -> Result<Vec<Value>> {
        let path = "/api/spaces/space";

        log::debug!("Fetching spaces from {}", path);

        let response = self
            .client
            .get(path)
            .await
            .with_context(|| "Failed to fetch spaces from Kibana")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!("Failed to fetch spaces ({}): {}", status, body);
        }

        let spaces: Vec<Value> = response
            .json()
            .await
            .with_context(|| "Failed to parse spaces response")?;

        log::info!("Fetched {} spaces from Kibana", spaces.len());

        Ok(spaces)
    }

    /// Fetch a single space by ID from Kibana
    async fn fetch_space(&self, space_id: &str) -> Result<Value> {
        let path = format!("/api/spaces/space/{}", space_id);

        log::debug!("Fetching space '{}' from {}", space_id, path);

        let response = self
            .client
            .get(&path)
            .await
            .with_context(|| format!("Failed to fetch space '{}'", space_id))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to fetch space '{}' ({}): {}",
                space_id,
                status,
                body
            );
        }

        let space: Value = response
            .json()
            .await
            .with_context(|| format!("Failed to parse space '{}' response", space_id))?;

        log::debug!("Fetched space: {}", space_id);

        Ok(space)
    }

    /// Fetch specific spaces by ID from manifest
    async fn fetch_manifest_spaces(&self, manifest: &super::SpacesManifest) -> Result<Vec<Value>> {
        let mut spaces = Vec::new();

        for space_id in &manifest.spaces {
            match self.fetch_space(space_id).await {
                Ok(space) => spaces.push(space),
                Err(e) => {
                    log::warn!("Failed to fetch space '{}': {}", space_id, e);
                    // Continue with other spaces instead of failing completely
                }
            }
        }

        log::info!("Fetched {} space(s) from manifest", spaces.len());

        Ok(spaces)
    }

    /// Filter spaces based on manifest
    fn filter_spaces(&self, spaces: Vec<Value>) -> Vec<Value> {
        if let Some(manifest) = &self.manifest {
            spaces
                .into_iter()
                .filter(|space| {
                    if let Some(id) = space.get("id").and_then(|v| v.as_str()) {
                        manifest.contains(id)
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            spaces
        }
    }
}

#[async_trait]
impl Extractor for SpacesExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        let spaces = if let Some(manifest) = &self.manifest {
            // Fetch only spaces from manifest by ID
            self.fetch_manifest_spaces(manifest).await?
        } else {
            // Fetch all spaces and filter
            let all_spaces = self.fetch_all_spaces().await?;
            self.filter_spaces(all_spaces)
        };

        log::info!(
            "Extracted {} space(s){}",
            spaces.len(),
            if self.manifest.is_some() {
                " (from manifest)"
            } else {
                ""
            }
        );

        Ok(spaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Auth;
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_filter_with_manifest() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let manifest = super::super::SpacesManifest::with_spaces(vec![
            "default".to_string(),
            "marketing".to_string(),
        ]);

        let extractor = SpacesExtractor::new(client, Some(manifest));

        let spaces = vec![
            json!({"id": "default", "name": "Default"}),
            json!({"id": "marketing", "name": "Marketing"}),
            json!({"id": "engineering", "name": "Engineering"}),
        ];

        let filtered = extractor.filter_spaces(spaces);
        assert_eq!(filtered.len(), 2);

        let ids: Vec<&str> = filtered
            .iter()
            .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
            .collect();

        assert!(ids.contains(&"default"));
        assert!(ids.contains(&"marketing"));
        assert!(!ids.contains(&"engineering"));
    }

    #[test]
    fn test_filter_without_manifest() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let extractor = SpacesExtractor::all(client);

        let spaces = vec![
            json!({"id": "default", "name": "Default"}),
            json!({"id": "marketing", "name": "Marketing"}),
        ];

        let filtered = extractor.filter_spaces(spaces.clone());
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered, spaces);
    }
}
