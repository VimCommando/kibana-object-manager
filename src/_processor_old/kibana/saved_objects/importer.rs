use super::{Manifest, objects};
use eyre::Result;
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use std::path::PathBuf;

impl ToString for Importer {
    fn to_string(&self) -> String {
        format!("{}", self.path.display())
    }
}

pub struct Importer {
    pub auth_header: String,
    pub file: PathBuf,
    pub manifest: Manifest,
    pub path: PathBuf,
    pub url: String,
}

impl Importer {
    pub fn push(&self) -> Result<String> {
        let count = objects::bundle(&self.file, &self.path)?;
        self.import()?;
        Ok(format!("Pushed {} objects", count.cyan()))
    }

    fn import(&self) -> Result<()> {
        let client = reqwest::blocking::Client::new();
        let import_url = format!(
            "{}/s/{}/api/saved_objects/_import?overwrite=true",
            self.url, "default"
        );
        log::debug!("Import URL: {}", import_url);
        let form = reqwest::blocking::multipart::Form::new().file("file", &self.file)?;
        let response = client
            .post(import_url)
            .header("Authorization", &self.auth_header)
            .header("kbn-xsrf", "true")
            .multipart(form)
            .send()?;

        match response.status() {
            StatusCode::OK => Ok(()),
            status => {
                log::error!("{:?}", response.text());
                Err(eyre::eyre!("Import failed with status {}", status))
            }
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}
