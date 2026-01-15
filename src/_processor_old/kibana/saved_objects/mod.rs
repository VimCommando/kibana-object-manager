use super::Manifest;
use eyre::{Result, eyre};
use owo_colors::OwoColorize;
use serde_json::Value;

const ENDPOINT: &str = "{}/s/{}/api/saved_objects/_export";

#[derive(Clone)]
pub struct SavedObjects {
    list: Vec<Value>,
}

impl SavedObjects {
    pub fn new(client: KibanaClient) -> Self {
        Self { client }
    }

    pub fn export(&self, manifest: &Manifest) -> Result<Vec<Value>> {
        let client = reqwest::blocking::Client::new();
        let export_url = format!("{}/s/{}/api/saved_objects/_export", url, "default");
        log::debug!("Export URL: {}", export_url.bright_blue());
        let response = client
            .post(export_url)
            .header("Authorization", auth_header)
            .header(
                "Content-Type",
                "application/json; Elastic-Api-Version=2023-10-31",
            )
            .header("kbn-xsrf", "string")
            .json(&manifest)
            .send()?;
        match response.status() {
            StatusCode::OK => {
                log::debug!("Export response status: {}", response.status().cyan());
                let objects: Vec<Value> = response
                    .text()?
                    .lines()
                    .filter_map(|line| serde_json::from_str(&line).ok())
                    .collect();
                log::debug!("Exported {} saved objects", objects.len().cyan());
                log::trace!("{:?}", objects);
                Ok(objects)
            }
            _ => {
                log::debug!("Export response status: {}", response.status().magenta());
                let body = response.text()?;
                log::trace!("Export response body: {}", body.red());
                return Err(eyre!("Failed to export saved objects"));
            }
        }
    }
}
