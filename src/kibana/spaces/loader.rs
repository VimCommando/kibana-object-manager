//! Spaces API loader
//!
//! Loads space definitions to Kibana via POST/PUT /api/spaces/space

use crate::client::Kibana;
use crate::etl::Loader;

use eyre::Result;
use owo_colors::OwoColorize;
use serde_json::Value;

/// Loader for Kibana spaces
///
/// Creates or updates spaces in Kibana. Uses PUT for updates to existing spaces
/// and POST for new spaces.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::spaces::SpacesLoader;
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Loader;
/// use serde_json::json;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None)?;
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
    client: Kibana,
    overwrite: bool,
}

impl SpacesLoader {
    /// Create a new spaces loader
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    pub fn new(client: Kibana) -> Self {
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

    /// Check if a space exists using HEAD request
    async fn space_exists(&self, space_id: &str) -> Result<bool> {
        let path = format!("/api/spaces/space/{}", space_id);

        log::debug!("{} {}", "HEAD".green(), path);

        let response = self.client.head(&path).await?;

        match response.status().as_u16() {
            200 => {
                log::debug!(
                    "{} {} - space exists, will update",
                    "200".green(),
                    space_id.cyan()
                );
                Ok(true)
            }
            404 => {
                log::debug!(
                    "{} {} - space not found, will create",
                    "404".yellow(),
                    space_id.cyan()
                );
                Ok(false)
            }
            status => {
                log::warn!(
                    "{} {} - unexpected status",
                    status.to_string().red(),
                    space_id.cyan()
                );
                Ok(false)
            }
        }
    }

    /// Create or update a single space
    async fn upsert_space(&self, space: &Value) -> Result<()> {
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        let exists = self.space_exists(space_id).await?;

        if exists && !self.overwrite {
            log::info!("Skipping existing space: {}", space_id.cyan());
            return Ok(());
        }

        let (method, path) = if exists {
            ("PUT", format!("/api/spaces/space/{}", space_id))
        } else {
            ("POST", "/api/spaces/space".to_string())
        };

        log::debug!("{} {}", method.green(), path);

        let response = if exists {
            // PUT for update
            self.client.put_json_value(&path, space).await?
        } else {
            // POST for create
            self.client.post_json_value(&path, space).await?
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to {} space {} ({}): {}",
                if exists { "update" } else { "create" },
                space_id.cyan(),
                status,
                body
            );
        }

        log::info!(
            "{} space: {}",
            if exists { "Updated" } else { "Created" },
            space_id.cyan()
        );

        Ok(())
    }
}

impl Loader for SpacesLoader {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        let mut count = 0;

        for space in items {
            self.upsert_space(&space).await?;
            count += 1;
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
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client);
        assert!(loader.overwrite);
    }

    #[test]
    fn test_with_overwrite() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client).with_overwrite(false);
        assert!(!loader.overwrite);
    }

    #[tokio::test]
    async fn test_missing_id_fails() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client);

        let space = json!({"name": "No ID"});

        let result = loader.upsert_space(&space).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing 'id' field")
        );
    }
}
