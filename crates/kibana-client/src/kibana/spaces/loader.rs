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
                let response = client.get(&path).await?;
                let status = response.status();
                let exists = match status.as_u16() {
                    200 => true,
                    404 => false,
                    _ => {
                        let body = response.text().await.unwrap_or_default();
                        return Err(Error::api_response(status, body));
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
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(Error::message(format!("space load task panicked: {e}"))),
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
    use reqwest::StatusCode;
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
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

        assert!(matches!(
            result,
            Err(Error::MissingResourceId { resource: "space" })
        ));
    }

    #[tokio::test]
    async fn test_unexpected_existence_status_fails_with_response() {
        let (url, request_rx, server) = start_one_response_server(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 11\r\n\r\nserver down",
        );
        let client = KibanaClient::new(url, Auth::None).unwrap();
        let loader = SpacesLoader::new(client);

        let result = loader
            .load(vec![json!({
                "id": "marketing",
                "name": "Marketing"
            })])
            .await;

        let request_line = request_rx.recv().unwrap();
        server.join().unwrap();

        assert_eq!(request_line, "GET /api/spaces/space/marketing HTTP/1.1");
        assert!(matches!(
            result,
            Err(Error::ApiResponse {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                body
            }) if body == "server down"
        ));
    }

    fn start_one_response_server(
        response: &'static str,
    ) -> (Url, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let (request_tx, request_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_request(stream, response, request_tx);
        });

        (url, request_rx, server)
    }

    fn handle_request(stream: TcpStream, response: &'static str, request_tx: mpsc::Sender<String>) {
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        request_tx
            .send(request_line.trim_end().to_string())
            .unwrap();

        let mut header = String::new();
        loop {
            header.clear();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }

        let mut stream = reader.into_inner();
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
}
