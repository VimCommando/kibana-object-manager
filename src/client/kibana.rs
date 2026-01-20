//! Kibana client module
//!
//! Provides `KibanaClient` for making API requests to Kibana.
//! The client can be scoped to a specific space via `.space(id)`.

use super::Auth;
use crate::kibana::spaces::SpacesManifest;
use base64::Engine;
use eyre::{Context, Result, eyre};
use reqwest::{Client, Method, multipart};
use std::collections::HashMap;
use std::path::Path;
use url::Url;

/// Kibana client for making API requests.
///
/// The client can operate in two modes:
/// - **Root mode** (`space: None`): For global operations like managing spaces
/// - **Space mode** (`space: Some(id)`): For space-scoped operations
///
/// Use `.space(id)` to create a space-scoped client from a root client.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::client::{Auth, KibanaClient};
/// use url::Url;
/// use std::path::Path;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::try_new(url, Auth::None, Path::new("."))?;
///
/// // Root client for global operations (e.g., managing spaces)
/// let response = client.get("/api/spaces/space").await?;
///
/// // Space-scoped client for space-specific operations
/// let marketing = client.space("marketing")?;
/// let objects = marketing.get("/api/saved_objects/_find").await?;
///
/// // Get the current space ID
/// assert_eq!(marketing.space_id(), "marketing");
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct KibanaClient {
    client: Client,
    url: Url,
    spaces: HashMap<String, String>, // id -> name (for validation)
    space: Option<String>,           // Current space context (None = root/default)
}

impl KibanaClient {
    /// Create a new root KibanaClient from a URL, Auth, and project directory.
    ///
    /// Loads the spaces manifest from `{project_dir}/spaces.yml`. If the manifest
    /// doesn't exist, defaults to a single "default" space.
    ///
    /// The returned client is in root mode (no space scoping). Use `.space(id)`
    /// to create a space-scoped client.
    ///
    /// # Arguments
    /// * `url` - Base Kibana URL
    /// * `auth` - Authentication method
    /// * `project_dir` - Project directory containing spaces.yml
    ///
    /// # Returns
    /// A new KibanaClient instance in root mode
    ///
    /// # Errors
    /// Returns an error if:
    /// - The HTTP client cannot be built
    /// - The spaces manifest exists but cannot be parsed
    pub fn try_new(url: Url, auth: Auth, project_dir: impl AsRef<Path>) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("kbn-xsrf", "true".parse()?);
        match auth {
            Auth::Basic(username, password) => {
                let credentials = base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", username, password));
                headers.append(
                    reqwest::header::AUTHORIZATION,
                    format!("Basic {}", credentials).parse()?,
                );
            }
            Auth::Apikey(apikey) => {
                headers.append(
                    reqwest::header::AUTHORIZATION,
                    format!("ApiKey {}", apikey).parse()?,
                );
            }
            Auth::None => {
                headers.append(reqwest::header::AUTHORIZATION, "None".parse()?);
            }
        }
        let client = Client::builder().default_headers(headers).build()?;

        // Load spaces from manifest
        let spaces_manifest_path = project_dir.as_ref().join("spaces.yml");
        let spaces = if spaces_manifest_path.exists() {
            log::debug!("Loading spaces from {}", spaces_manifest_path.display());
            let manifest = SpacesManifest::read(&spaces_manifest_path)
                .with_context(|| "Failed to load spaces manifest")?;
            manifest
                .spaces
                .into_iter()
                .map(|s| (s.id, s.name))
                .collect()
        } else {
            log::debug!("No spaces manifest found, defaulting to 'default' space");
            let mut spaces = HashMap::new();
            spaces.insert("default".to_string(), "Default".to_string());
            spaces
        };

        Ok(Self {
            client,
            url,
            spaces,
            space: None, // Root mode
        })
    }

    /// Create a space-scoped client for the given space ID.
    ///
    /// Returns a new client that will automatically scope all API requests
    /// to the specified space (prefixing paths with `/s/{space}/`).
    ///
    /// # Arguments
    /// * `id` - Space ID to scope to
    ///
    /// # Returns
    /// A new KibanaClient scoped to the specified space
    ///
    /// # Errors
    /// Returns an error if the space ID is not in the loaded manifest
    ///
    /// # Example
    /// ```no_run
    /// # use kibana_object_manager::client::{Auth, KibanaClient};
    /// # use url::Url;
    /// # use std::path::Path;
    /// # fn example() -> eyre::Result<()> {
    /// # let url = Url::parse("http://localhost:5601")?;
    /// # let client = KibanaClient::try_new(url, Auth::None, Path::new("."))?;
    /// let marketing = client.space("marketing")?;
    /// assert_eq!(marketing.space_id(), "marketing");
    /// # Ok(())
    /// # }
    /// ```
    pub fn space(&self, id: &str) -> Result<KibanaClient> {
        if !self.spaces.contains_key(id) {
            eyre::bail!(
                "Space '{}' not found in manifest. Available spaces: {}",
                id,
                self.spaces.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }

        let space = if id == "default" {
            None
        } else {
            Some(id.to_string())
        };

        Ok(KibanaClient {
            client: self.client.clone(),
            url: self.url.clone(),
            spaces: self.spaces.clone(),
            space,
        })
    }

    /// Get the current space ID.
    ///
    /// Returns "default" for root mode or the default space.
    pub fn space_id(&self) -> &str {
        self.space.as_deref().unwrap_or("default")
    }

    /// Check if this client is in root mode (no space scoping).
    pub fn is_root(&self) -> bool {
        self.space.is_none()
    }

    /// Get all available space IDs from the manifest.
    pub fn space_ids(&self) -> Vec<&str> {
        self.spaces.keys().map(|s| s.as_str()).collect()
    }

    /// Get the name of a space by ID.
    ///
    /// # Returns
    /// The space name if found, None otherwise
    pub fn space_name(&self, id: &str) -> Option<&str> {
        self.spaces.get(id).map(|s| s.as_str())
    }

    /// Check if a space exists in the manifest.
    pub fn has_space(&self, id: &str) -> bool {
        self.spaces.contains_key(id)
    }

    /// Get the base URL.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Verify the connection and authentication to Kibana.
    ///
    /// Makes a GET request to /api/status to verify connectivity.
    pub async fn test_connection(&self) -> Result<reqwest::Response> {
        // Always use root path for status check
        self.request_raw(Method::GET, &HashMap::new(), "/api/status", None)
            .await
    }

    /// Send a request to a given path.
    ///
    /// If this client is scoped to a space, the path will be prefixed with `/s/{space}/`.
    /// For root mode, the path is used as-is.
    ///
    /// # Arguments
    /// * `method` - HTTP method
    /// * `headers` - Additional headers
    /// * `path` - API path
    /// * `body` - Optional request body
    pub async fn request(
        &self,
        method: Method,
        headers: &HashMap<String, String>,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response> {
        // Strip leading slash from path if present, to avoid double slashes
        let path_stripped = path.strip_prefix('/').unwrap_or(path);

        // Build final path with space prefix if scoped to a space
        let final_path = match &self.space {
            Some(space) => format!("/s/{}/{}", space, path_stripped),
            None => format!("/{}", path_stripped),
        };

        self.request_raw(method, headers, &final_path, body).await
    }

    /// Send a request without space path prefixing (internal use).
    async fn request_raw(
        &self,
        method: Method,
        headers: &HashMap<String, String>,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response> {
        let mut headers: reqwest::header::HeaderMap = headers
            .iter()
            .map(|(k, v)| (k.parse().unwrap(), v.parse().unwrap()))
            .collect();
        let use_form_data = match headers.get("Content-Type") {
            Some(content_type) => {
                log::trace!("Content-Type: {}", content_type.to_str()?);
                content_type.to_str()?.starts_with("multipart/form-data")
            }
            None => false,
        };

        if use_form_data {
            headers.remove("Content-Type");
        }

        let client = match path.split_once('?') {
            Some((p, query)) => {
                let query: Vec<_> = query.split('&').filter_map(|s| s.split_once('=')).collect();
                self.client
                    .request(method, self.url.join(p)?)
                    .query(&query)
                    .headers(headers)
            }
            None => self
                .client
                .request(method, self.url.join(path)?)
                .headers(headers),
        };

        let response = match body {
            Some(body) if use_form_data => {
                log::trace!("Sending request with form-data");
                let part = multipart::Part::bytes(body.to_vec())
                    .file_name("dashboards.ndjson")
                    .mime_str("application/x-ndjson")?;
                let form = multipart::Form::new().part("file", part);
                client.multipart(form).send().await
            }
            Some(body) => {
                log::trace!("Sending request with body");
                client.body(body.to_vec()).send().await
            }
            None => client.send().await,
        };
        response.map_err(|e| eyre!("Failed to send request: {}", e))
    }

    /// Helper for GET requests.
    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.request(Method::GET, &HashMap::new(), path, None).await
    }

    /// Helper for GET requests for internal Kibana APIs.
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows).
    pub async fn get_internal(&self, path: &str) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "X-Elastic-Internal-Origin".to_string(),
            "Kibana".to_string(),
        );
        self.request(Method::GET, &headers, path, None).await
    }

    /// Helper for HEAD requests.
    pub async fn head(&self, path: &str) -> Result<reqwest::Response> {
        self.request(Method::HEAD, &HashMap::new(), path, None)
            .await
    }

    /// Helper for HEAD requests for internal Kibana APIs.
    /// Adds X-Elastic-Internal-Origin header required by some APIs.
    pub async fn head_internal(&self, path: &str) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "X-Elastic-Internal-Origin".to_string(),
            "Kibana".to_string(),
        );
        self.request(Method::HEAD, &headers, path, None).await
    }

    /// Helper for POST requests with JSON body.
    pub async fn post_json(&self, path: &str, body: &[u8]) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::POST, &headers, path, Some(body)).await
    }

    /// Helper for POST requests with JSON value.
    pub async fn post_json_value(
        &self,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        self.post_json(path, &body).await
    }

    /// Helper for POST requests with JSON value for internal Kibana APIs.
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows).
    pub async fn post_json_value_internal(
        &self,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert(
            "X-Elastic-Internal-Origin".to_string(),
            "Kibana".to_string(),
        );
        self.request(Method::POST, &headers, path, Some(&body))
            .await
    }

    /// Helper for PUT requests with JSON value.
    pub async fn put_json_value(
        &self,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::PUT, &headers, path, Some(&body)).await
    }

    /// Helper for POST requests with multipart form data.
    pub async fn post_form(&self, path: &str, body: &[u8]) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "multipart/form-data".to_string(),
        );
        self.request(Method::POST, &headers, path, Some(body)).await
    }
}

impl std::fmt::Display for KibanaClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.space {
            Some(space) => write!(f, "{} (space: {})", self.url, space),
            None => write!(f, "{}", self.url),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_kibana_client_no_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();

        assert_eq!(client.space_ids().len(), 1);
        assert!(client.has_space("default"));
        assert_eq!(client.space_name("default"), Some("Default"));
        assert!(client.is_root());
        assert_eq!(client.space_id(), "default");
    }

    #[test]
    fn test_kibana_client_with_manifest() {
        let temp_dir = TempDir::new().unwrap();

        // Create spaces.yml
        let manifest = SpacesManifest::with_spaces(vec![
            crate::kibana::spaces::SpaceEntry::new("default".to_string(), "Default".to_string()),
            crate::kibana::spaces::SpaceEntry::new(
                "marketing".to_string(),
                "Marketing".to_string(),
            ),
        ]);
        manifest.write(temp_dir.path().join("spaces.yml")).unwrap();

        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();

        assert_eq!(client.space_ids().len(), 2);
        assert!(client.has_space("default"));
        assert!(client.has_space("marketing"));
        assert_eq!(client.space_name("marketing"), Some("Marketing"));
    }

    #[test]
    fn test_space_scoped_client() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = SpacesManifest::with_spaces(vec![
            crate::kibana::spaces::SpaceEntry::new("default".to_string(), "Default".to_string()),
            crate::kibana::spaces::SpaceEntry::new(
                "marketing".to_string(),
                "Marketing".to_string(),
            ),
        ]);
        manifest.write(temp_dir.path().join("spaces.yml")).unwrap();

        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();

        // Root client
        assert!(client.is_root());
        assert_eq!(client.space_id(), "default");

        // Default space should have None internally (no /s/ prefix)
        let default = client.space("default").unwrap();
        assert_eq!(default.space_id(), "default");
        assert!(default.space.is_none());
        // Note: default space is still "root-like" in terms of path prefixing

        // Non-default space should have Some
        let marketing = client.space("marketing").unwrap();
        assert_eq!(marketing.space_id(), "marketing");
        assert!(!marketing.is_root());
        assert!(marketing.space.is_some());

        // Space-scoped client should still have access to spaces map
        assert!(marketing.has_space("default"));
        assert!(marketing.has_space("marketing"));
    }

    #[test]
    fn test_invalid_space() {
        let temp_dir = TempDir::new().unwrap();
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::try_new(url, Auth::None, temp_dir.path()).unwrap();

        let result = client.space("nonexistent");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not found in manifest")
        );
    }
}
