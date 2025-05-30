use super::{Kibana, ObjectManager};
use eyre::{Result, eyre};
use owo_colors::OwoColorize;

impl ObjectManager for Authorizer {}

pub struct Authorizer {
    pub auth_header: String,
    pub url: String,
}

impl Kibana<Authorizer> {
    pub fn authorize(&self) -> Result<String> {
        let authorizer = &self.objects;
        let client = reqwest::blocking::Client::new();
        let response = client
            .get(format!("{}/api/spaces/space?", authorizer.url))
            .header("Authorization", &authorizer.auth_header)
            .send()?;

        if response.status().is_success() {
            let body = response.json::<serde_json::Value>()?;
            log::debug!("Response body: {}", body);
            let name = body[0]["name"].as_str().unwrap_or("Unknown");
            let description = body[0]["description"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string()
                .chars()
                .take(30)
                .collect::<String>();
            let description = match description.len() > 30 {
                true => format!("{}...", description),
                false => description,
            };
            Ok(format!(
                "Kibana's default space is {}: {}",
                name.cyan(),
                description.bright_black()
            ))
        } else {
            let body = response.text()?;
            log::debug!("Response body: {}", body);
            Err(eyre!("Authorization failed: {}", body))
        }
    }

    pub fn url(&self) -> &str {
        &self.objects.url
    }
}
