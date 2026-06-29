//! Kibana client module
//!
//! Provides `KibanaClient` for making API requests to Kibana.
//! The client can be scoped to a specific space via `.space(id)`.

use super::Auth;
use crate::{Error, Result};
use base64::Engine;
use reqwest::{Client, Method, multipart};
use semver::Version;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, trace};
use url::Url;

/// Semantic version of a Kibana server.
pub type KibanaVersion = Version;

/// Parse Kibana version strings using semver with small compatibility fixes:
/// - optional leading `v` prefix
/// - missing patch in `major.minor` strings (normalized to `.0`)
pub fn parse_kibana_version(version: &str) -> Result<KibanaVersion> {
    let trimmed = version.trim().trim_start_matches('v');

    if let Ok(parsed) = KibanaVersion::parse(trimmed) {
        return Ok(parsed);
    }

    let dot_count = trimmed.matches('.').count();
    let normalized = match dot_count {
        0 => format!("{trimmed}.0.0"),
        1 => format!("{trimmed}.0"),
        _ => trimmed.to_string(),
    };

    KibanaVersion::parse(&normalized).map_err(Error::from)
}

/// Resolved Kibana version details from `/api/status`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KibanaVersionInfo {
    pub raw: String,
    pub parsed: KibanaVersion,
}

/// Caller-provided registry of known Kibana spaces.
pub type SpaceRegistry = HashMap<String, String>;

/// API families that are gated by minimum Kibana versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiCapability {
    Spaces,
    SavedObjects,
    Agents,
    Tools,
    Workflows,
}

impl ApiCapability {
    pub fn name(self) -> &'static str {
        match self {
            Self::Spaces => "spaces",
            Self::SavedObjects => "saved_objects",
            Self::Agents => "agents",
            Self::Tools => "tools",
            Self::Workflows => "workflows",
        }
    }

    pub fn minimum_version(self) -> KibanaVersion {
        match self {
            Self::Spaces | Self::SavedObjects => KibanaVersion::new(8, 0, 0),
            Self::Agents | Self::Tools => KibanaVersion::new(9, 2, 0),
            Self::Workflows => KibanaVersion::new(9, 3, 0),
        }
    }

    pub fn maturity_note(self) -> Option<&'static str> {
        match self {
            Self::Agents | Self::Tools => Some("Tech preview in 9.2, GA in 9.3"),
            Self::Workflows => Some("Tech preview in 9.3"),
            _ => None,
        }
    }
}

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
/// use kibana_sync::{Auth, KibanaClient};
/// use url::Url;
///
/// # async fn example() -> kibana_sync::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = KibanaClient::builder(url)
///     .auth(Auth::None)
///     .spaces([("marketing".to_string(), "Marketing".to_string())])
///     .build()?;
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
    semaphore: Arc<Semaphore>,       // Global concurrency limit
    version_info: Arc<RwLock<Option<KibanaVersionInfo>>>,
}

/// Builder for [`KibanaClient`].
#[derive(Clone, Debug)]
pub struct KibanaClientBuilder {
    url: Url,
    auth: Auth,
    max_concurrency: usize,
    spaces: SpaceRegistry,
}

impl KibanaClientBuilder {
    fn new(url: Url) -> Self {
        Self {
            url,
            auth: Auth::None,
            max_concurrency: 8,
            spaces: default_spaces(),
        }
    }

    /// Set authentication for requests.
    pub fn auth(mut self, auth: Auth) -> Self {
        self.auth = auth;
        self
    }

    /// Set the maximum number of concurrent requests shared by all cloned clients.
    pub fn max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency;
        self
    }

    /// Replace the space registry with caller-provided spaces.
    pub fn spaces(mut self, spaces: impl IntoIterator<Item = (String, String)>) -> Self {
        self.spaces = spaces.into_iter().collect();
        if self.spaces.is_empty() {
            self.spaces = default_spaces();
        }
        self
    }

    /// Build the root Kibana client.
    pub fn build(self) -> Result<KibanaClient> {
        if self.max_concurrency == 0 {
            return Err(Error::InvalidConfiguration(
                "max_concurrency must be greater than zero".to_string(),
            ));
        }

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("kbn-xsrf", "true".parse()?);
        match self.auth {
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
                // No authentication header.
            }
        }
        let client = Client::builder().default_headers(headers).build()?;
        let semaphore = Arc::new(Semaphore::new(self.max_concurrency));

        Ok(KibanaClient {
            client,
            url: self.url,
            spaces: self.spaces,
            space: None, // Root mode
            semaphore,
            version_info: Arc::new(RwLock::new(None)),
        })
    }
}

fn default_spaces() -> SpaceRegistry {
    let mut spaces = HashMap::new();
    spaces.insert("default".to_string(), "Default".to_string());
    spaces
}

impl KibanaClient {
    /// Start configuring a root Kibana client from explicit values.
    pub fn builder(url: Url) -> KibanaClientBuilder {
        KibanaClientBuilder::new(url)
    }

    /// Create a root client with default options and only the default space.
    pub fn new(url: Url, auth: Auth) -> Result<Self> {
        Self::builder(url).auth(auth).build()
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
    /// # use kibana_sync::{Auth, KibanaClient};
    /// # use url::Url;
    /// # fn example() -> kibana_sync::Result<()> {
    /// # let url = Url::parse("http://localhost:5601")?;
    /// # let client = KibanaClient::builder(url)
    /// #     .spaces([("marketing".to_string(), "Marketing".to_string())])
    /// #     .build()?;
    /// let marketing = client.space("marketing")?;
    /// assert_eq!(marketing.space_id(), "marketing");
    /// # Ok(())
    /// # }
    /// ```
    pub fn space(&self, id: &str) -> Result<KibanaClient> {
        if !self.spaces.contains_key(id) {
            let mut available = self.spaces.keys().cloned().collect::<Vec<_>>();
            available.sort();
            return Err(Error::InvalidSpace {
                id: id.to_string(),
                available,
            });
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
            semaphore: self.semaphore.clone(),
            version_info: self.version_info.clone(),
        })
    }

    /// Resolve and cache Kibana server version info from `/api/status`.
    pub async fn server_version_info(&self) -> Result<KibanaVersionInfo> {
        if let Some(cached) = self.version_info.read().await.clone() {
            return Ok(cached);
        }

        let response = self
            .request_raw(Method::GET, &HashMap::new(), "/api/status", None)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api_response(status, body));
        }

        let status: Value = response.json().await?;
        let raw = status
            .get("version")
            .and_then(|v| v.get("number"))
            .and_then(|v| v.as_str())
            .ok_or(Error::MissingField {
                field: "version.number",
            })?
            .to_string();
        let parsed = parse_kibana_version(&raw)?;
        let info = KibanaVersionInfo { raw, parsed };

        *self.version_info.write().await = Some(info.clone());
        Ok(info)
    }

    /// Resolve the normalized Kibana server version.
    pub async fn server_version(&self) -> Result<KibanaVersion> {
        Ok(self.server_version_info().await?.parsed)
    }

    /// Check if a capability is supported on a specific Kibana version.
    pub fn supports_capability(version: &KibanaVersion, capability: ApiCapability) -> bool {
        version >= &capability.minimum_version()
    }

    /// Build a user-facing unsupported message for a capability/version pair.
    pub fn unsupported_capability_reason(
        version: &KibanaVersion,
        capability: ApiCapability,
    ) -> String {
        let minimum = capability.minimum_version();
        match capability.maturity_note() {
            Some(note) => format!(
                "API '{}' requires Kibana {}+ (detected {}, {})",
                capability.name(),
                minimum,
                version,
                note
            ),
            None => format!(
                "API '{}' requires Kibana {}+ (detected {})",
                capability.name(),
                minimum,
                version
            ),
        }
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

        debug!(method = %method, path = %final_path, "sending Kibana request");

        self.request_raw(method, headers, &final_path, body).await
    }

    #[cfg(test)]
    pub(crate) fn prefixed_path_for_test(&self, path: &str) -> String {
        let path_stripped = path.strip_prefix('/').unwrap_or(path);
        match &self.space {
            Some(space) => format!("/s/{}/{}", space, path_stripped),
            None => format!("/{}", path_stripped),
        }
    }

    /// Send a request without space path prefixing (internal use).
    async fn request_raw(
        &self,
        method: Method,
        headers: &HashMap<String, String>,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| Error::SemaphoreClosed)?;

        let mut header_map = reqwest::header::HeaderMap::new();
        for (key, value) in headers {
            header_map.insert(
                key.parse::<reqwest::header::HeaderName>()?,
                value.parse::<reqwest::header::HeaderValue>()?,
            );
        }
        let use_form_data = match headers.get("Content-Type") {
            Some(content_type) => {
                trace!("Content-Type: {}", content_type);
                content_type.starts_with("multipart/form-data")
            }
            None => false,
        };

        if use_form_data {
            header_map.remove("Content-Type");
        }

        let client = match path.split_once('?') {
            Some((p, query)) => {
                let query: Vec<_> = query.split('&').filter_map(|s| s.split_once('=')).collect();
                self.client
                    .request(method, self.url.join(p)?)
                    .query(&query)
                    .headers(header_map)
            }
            None => self
                .client
                .request(method, self.url.join(path)?)
                .headers(header_map),
        };

        let response = match body {
            Some(body) if use_form_data => {
                trace!("sending Kibana request with form-data");
                let part = multipart::Part::bytes(body.to_vec())
                    .file_name("dashboards.ndjson")
                    .mime_str("application/x-ndjson")?;
                let form = multipart::Form::new().part("file", part);
                client.multipart(form).send().await
            }
            Some(body) => {
                trace!("sending Kibana request with body");
                client.body(body.to_vec()).send().await
            }
            None => client.send().await,
        };
        response.map_err(Error::from)
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

    /// Helper for PUT requests with JSON value for internal Kibana APIs.
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows).
    pub async fn put_json_value_internal(
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

    #[test]
    fn test_kibana_client_defaults_to_default_space_without_filesystem() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::builder(url).build().unwrap();

        assert_eq!(client.space_ids().len(), 1);
        assert!(client.has_space("default"));
        assert_eq!(client.space_name("default"), Some("Default"));
        assert!(client.is_root());
        assert_eq!(client.space_id(), "default");
    }

    #[test]
    fn test_kibana_client_with_explicit_space_registry() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::builder(url)
            .spaces([
                ("default".to_string(), "Default".to_string()),
                ("marketing".to_string(), "Marketing".to_string()),
            ])
            .build()
            .unwrap();

        assert_eq!(client.space_ids().len(), 2);
        assert!(client.has_space("default"));
        assert!(client.has_space("marketing"));
        assert_eq!(client.space_name("marketing"), Some("Marketing"));
    }

    #[test]
    fn test_space_scoped_client() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::builder(url)
            .spaces([
                ("default".to_string(), "Default".to_string()),
                ("marketing".to_string(), "Marketing".to_string()),
            ])
            .build()
            .unwrap();

        // Root client
        assert!(client.is_root());
        assert_eq!(client.space_id(), "default");

        // Default space should have None internally (no /s/ prefix)
        let default = client.space("default").unwrap();
        assert_eq!(default.space_id(), "default");
        assert!(default.space.is_none());
        assert_eq!(
            default.prefixed_path_for_test("/api/saved_objects"),
            "/api/saved_objects"
        );

        // Non-default space should have Some
        let marketing = client.space("marketing").unwrap();
        assert_eq!(marketing.space_id(), "marketing");
        assert!(!marketing.is_root());
        assert!(marketing.space.is_some());
        assert_eq!(
            marketing.prefixed_path_for_test("/api/saved_objects"),
            "/s/marketing/api/saved_objects"
        );

        // Space-scoped client should still have access to spaces map
        assert!(marketing.has_space("default"));
        assert!(marketing.has_space("marketing"));
    }

    #[test]
    fn test_invalid_space() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = KibanaClient::builder(url).build().unwrap();

        let result = client.space("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidSpace { .. }));
    }

    #[test]
    fn test_parse_kibana_version() {
        let parsed = parse_kibana_version("9.3.2").unwrap();
        assert_eq!(parsed, KibanaVersion::new(9, 3, 2));

        let snapshot = parse_kibana_version("9.4.0-SNAPSHOT").unwrap();
        assert_eq!(snapshot, KibanaVersion::parse("9.4.0-SNAPSHOT").unwrap());

        let prefixed = parse_kibana_version("v9.5.1").unwrap();
        assert_eq!(prefixed, KibanaVersion::new(9, 5, 1));

        let missing_patch = parse_kibana_version("9.6").unwrap();
        assert_eq!(missing_patch, KibanaVersion::new(9, 6, 0));
    }

    #[test]
    fn test_capability_thresholds() {
        let v92 = parse_kibana_version("9.2.1").unwrap();
        let v91 = parse_kibana_version("9.1.9").unwrap();
        let v93 = parse_kibana_version("9.3.0").unwrap();

        assert!(KibanaClient::supports_capability(
            &v92,
            ApiCapability::Agents
        ));
        assert!(KibanaClient::supports_capability(
            &v92,
            ApiCapability::Tools
        ));
        assert!(!KibanaClient::supports_capability(
            &v91,
            ApiCapability::Agents
        ));
        assert!(!KibanaClient::supports_capability(
            &v91,
            ApiCapability::Tools
        ));
        assert!(KibanaClient::supports_capability(
            &v93,
            ApiCapability::Workflows
        ));
        assert!(!KibanaClient::supports_capability(
            &v92,
            ApiCapability::Workflows
        ));
    }
}
