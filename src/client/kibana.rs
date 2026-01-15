use super::Auth;
use base64::Engine;
use eyre::{Result, eyre};
use reqwest::{Client, Method, multipart};
use std::collections::HashMap;
use url::Url;

/// A client that communicates with the Kibana API.
#[derive(Clone)]
pub struct Kibana {
    client: Client,
    space: Option<String>,
    url: Url,
}

/// A reqwest-based client with authentication for Kibana
impl Kibana {
    /// Create a new KibanaExporter from a URL and Auth
    pub fn try_new(url: Url, auth: Auth, space: Option<String>) -> Result<Self> {
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

        Ok(Self { client, space, url })
    }

    /// Send a request to a given path on the Kibana client
    pub async fn request(
        &self,
        method: Method,
        headers: &HashMap<String, String>,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response> {
        let path = match self.space {
            Some(ref space) => &format!("s/{}/{}", space, path),
            None => path,
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
        self.request(Method::GET, &HashMap::new(), "/api/status", None)
            .await
    }

    /// Get the base URL
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Get the space (if configured)
    pub fn space(&self) -> Option<&str> {
        self.space.as_deref()
    }

    /// Helper for GET requests
    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.request(Method::GET, &HashMap::new(), path, None).await
    }

    /// Helper for POST requests with JSON body
    pub async fn post_json(&self, path: &str, body: &[u8]) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request(Method::POST, &headers, path, Some(body)).await
    }

    /// Helper for POST requests with multipart form data
    pub async fn post_form(&self, path: &str, body: &[u8]) -> Result<reqwest::Response> {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "multipart/form-data".to_string(),
        );
        self.request(Method::POST, &headers, path, Some(body)).await
    }
}

impl std::fmt::Display for Kibana {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}
