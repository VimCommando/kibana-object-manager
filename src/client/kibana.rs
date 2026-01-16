use super::Auth;
use base64::Engine;
use eyre::{Result, eyre};
use reqwest::{Client, Method, multipart};
use std::collections::HashMap;
use url::Url;

/// A client that communicates with the Kibana API.
#[derive(Clone, Debug)]
pub struct Kibana {
    client: Client,
    url: Url,
}

/// A reqwest-based client with authentication for Kibana
impl Kibana {
    /// Create a new KibanaExporter from a URL and Auth
    pub fn try_new(url: Url, auth: Auth) -> Result<Self> {
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

        Ok(Self { client, url })
    }

    /// Send a request to a given path on the Kibana client
    ///
    /// # Arguments
    /// * `method` - HTTP method
    /// * `space_id` - Optional space ID to scope the request to
    /// * `headers` - Additional headers
    /// * `path` - API path
    /// * `body` - Optional request body
    pub async fn request(
        &self,
        method: Method,
        space_id: Option<&str>,
        headers: &HashMap<String, String>,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response> {
        // Strip leading slash from path if present, to avoid double slashes
        let path_stripped = path.strip_prefix('/').unwrap_or(path);

        // Build final path with space prefix if provided
        let final_path = match space_id {
            Some(space) if space != "default" => format!("/s/{}/{}", space, path_stripped),
            _ => format!("/{}", path_stripped),
        };

        let mut headers: reqwest::header::HeaderMap = headers
            .iter()
            .map(|(k, v)| (k.parse().unwrap(), v.parse().unwrap()))
            .collect();
        let use_form_data = match headers.get("Content-Type") {
            Some(content_type) => {
                log::debug!("Content-Type: {}", content_type.to_str()?);
                content_type.to_str()?.starts_with("multipart/form-data")
            }
            None => false,
        };

        if use_form_data {
            // Reqwest inserts its own multipart Content-Type headers,
            // this removal prevents conflicts
            headers.remove("Content-Type");
        }

        let client = match final_path.split_once('?') {
            Some((p, query)) => {
                let query: Vec<_> = query.split('&').filter_map(|s| s.split_once('=')).collect();
                self.client
                    .request(method, self.url.join(p)?)
                    .query(&query)
                    .headers(headers)
            }
            None => self
                .client
                .request(method, self.url.join(&final_path)?)
                .headers(headers),
        };

        let response = match body {
            Some(body) if use_form_data => {
                // As of October 2025 we're using a static filename, as the only
                // use of form data is dashboards.ndjson for the saved objects API
                log::debug!("Sending request with form-data");
                let part = multipart::Part::bytes(body.to_vec())
                    .file_name("dashboards.ndjson")
                    .mime_str("application/x-ndjson")?;
                let form = multipart::Form::new().part("file", part);
                client.multipart(form).send().await
            }
            Some(body) => {
                log::debug!("Sending request with body");
                client.body(body.to_vec()).send().await
            }
            None => client.send().await,
        };
        response.map_err(|e| eyre!("Failed to send request: {}", e))
    }

    /// Verify the connection and authentication to Kibana
    pub async fn test_connection(&self) -> Result<reqwest::Response> {
        self.request(Method::GET, None, &HashMap::new(), "/api/status", None)
            .await
    }

    /// Get the base URL
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Helper for GET requests (no space scoping)
    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.request(Method::GET, None, &HashMap::new(), path, None)
            .await
    }

    /// Helper for GET requests with space scoping
    pub async fn get_with_space(&self, space_id: &str, path: &str) -> Result<reqwest::Response> {
        self.request(Method::GET, Some(space_id), &HashMap::new(), path, None)
            .await
    }

    /// Helper for GET requests for internal Kibana APIs (no space scoping)
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows)
    pub async fn get_internal(&self, path: &str) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "X-Elastic-Internal-Origin".to_string(),
            "Kibana".to_string(),
        );
        self.request(Method::GET, None, &headers, path, None).await
    }

    /// Helper for GET requests for internal Kibana APIs with space scoping
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows)
    pub async fn get_internal_with_space(
        &self,
        space_id: &str,
        path: &str,
    ) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "X-Elastic-Internal-Origin".to_string(),
            "Kibana".to_string(),
        );
        self.request(Method::GET, Some(space_id), &headers, path, None)
            .await
    }

    /// Helper for POST requests with JSON body (no space scoping)
    pub async fn post_json(&self, path: &str, body: &[u8]) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::POST, None, &headers, path, Some(body))
            .await
    }

    /// Helper for POST requests with JSON body with space scoping
    pub async fn post_json_with_space(
        &self,
        space_id: &str,
        path: &str,
        body: &[u8],
    ) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::POST, Some(space_id), &headers, path, Some(body))
            .await
    }

    /// Helper for POST requests with JSON value (no space scoping)
    pub async fn post_json_value(
        &self,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        self.post_json(path, &body).await
    }

    /// Helper for POST requests with JSON value with space scoping
    pub async fn post_json_value_with_space(
        &self,
        space_id: &str,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        self.post_json_with_space(space_id, path, &body).await
    }

    /// Helper for POST requests with JSON value for internal Kibana APIs (no space scoping)
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows)
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
        self.request(Method::POST, None, &headers, path, Some(&body))
            .await
    }

    /// Helper for POST requests with JSON value for internal Kibana APIs with space scoping
    /// Adds X-Elastic-Internal-Origin header required by some APIs (e.g., workflows)
    pub async fn post_json_value_internal_with_space(
        &self,
        space_id: &str,
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
        self.request(Method::POST, Some(space_id), &headers, path, Some(&body))
            .await
    }

    /// Helper for PUT requests with JSON value (no space scoping)
    pub async fn put_json_value(
        &self,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::PUT, None, &headers, path, Some(&body))
            .await
    }

    /// Helper for PUT requests with JSON value with space scoping
    pub async fn put_json_value_with_space(
        &self,
        space_id: &str,
        path: &str,
        value: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(value)?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::PUT, Some(space_id), &headers, path, Some(&body))
            .await
    }

    /// Helper for POST requests with multipart form data (no space scoping)
    pub async fn post_form(&self, path: &str, body: &[u8]) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "multipart/form-data".to_string(),
        );
        self.request(Method::POST, None, &headers, path, Some(body))
            .await
    }

    /// Helper for POST requests with multipart form data with space scoping
    pub async fn post_form_with_space(
        &self,
        space_id: &str,
        path: &str,
        body: &[u8],
    ) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "multipart/form-data".to_string(),
        );
        self.request(Method::POST, Some(space_id), &headers, path, Some(body))
            .await
    }
}

impl std::fmt::Display for Kibana {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}
