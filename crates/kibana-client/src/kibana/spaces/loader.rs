//! Spaces API loader
//!
//! Loads space definitions to Kibana via POST/PUT /api/spaces/space

use crate::client::KibanaClient;
use crate::etl::Loader;

use crate::{Error, Result};
use serde_json::Value;
use tokio::task::JoinSet;

/// Loader for Kibana spaces
///
/// Creates or updates spaces in Kibana. Uses PUT for updates to existing spaces
/// and POST for new spaces.
///
/// # Example
/// ```no_run
/// use kibana_client::kibana::spaces::SpacesLoader;
/// use kibana_client::client::{Auth, KibanaClient};
/// use kibana_client::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> kibana_client::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::new(url, Auth::None)?;
/// let loader = SpacesLoader::new(client);
///
/// let spaces = vec![
///     json!({
///         "id": "marketing",
///         "name": "Marketing",
///         "description": "Marketing team space"
///     })
/// ];
///
/// let count = loader.load(spaces).await?;
/// # Ok(())
/// # }
/// ```
pub struct SpacesLoader {
    client: KibanaClient,
    overwrite: bool,
}

impl SpacesLoader {
    /// Create a new spaces loader
    ///
    /// # Arguments
    /// * `client` - Kibana client (spaces are global, not space-scoped)
    pub fn new(client: KibanaClient) -> Self {
        Self {
            client,
            overwrite: true,
        }
    }

    /// Set whether to overwrite existing spaces (default: true)
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}

impl Loader for SpacesLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;
        let mut set = JoinSet::new();

        for space in items {
            let client = self.client.clone();
            let overwrite = self.overwrite;

            set.spawn(async move {
                let space_id = space
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or(Error::MissingResourceId { resource: "space" })?;

                // Check existence
                let path = format!("/api/spaces/space/{}", space_id);
                let exists = match client.get(&path).await?.status().as_u16() {
                    200 => true,
                    404 => false,
                    status => {
                        tracing::warn!(
                            "{} {} - unexpected status when checking existence",
                            status.to_string(),
                            space_id
                        );
                        false
                    }
                };

                if exists && !overwrite {
                    tracing::info!("Skipping existing space: {}", space_id);
                    return Ok::<bool, Error>(false);
                }

                if exists {
                    // Update
                    let path = format!("/api/spaces/space/{}", space_id);
                    let response = client.put_json_value(&path, &space).await?;
                    if !response.status().is_success() {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        return Err(Error::api_response(status, body));
                    }
                    tracing::info!("Updated space: {}", space_id);
                } else {
                    // Create
                    let path = "/api/spaces/space";
                    let response = client.post_json_value(path, &space).await?;
                    if !response.status().is_success() {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        return Err(Error::api_response(status, body));
                    }
                    tracing::info!("Created space: {}", space_id);
                }

                Ok::<bool, Error>(true)
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(loaded)) => {
                    if loaded {
                        count += 1;
                    }
                }
                Ok(Err(e)) => tracing::error!("Failed to load space: {}", e),
                Err(e) => tracing::error!("Task panicked: {}", e),
            }
        }

        // Don't log summary here - let caller handle it
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Auth;
    use serde_json::json;
    use url::Url;

    #[test]
    fn test_loader_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client);
        assert!(loader.overwrite);
    }

    #[test]
    fn test_with_overwrite() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client).with_overwrite(false);
        assert!(!loader.overwrite);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client);

        let space = json!({"name": "No ID"});

        let result = loader.load(vec![space]).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
