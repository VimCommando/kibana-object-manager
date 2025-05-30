use super::{Kibana, Manifest, ObjectManager, objects};
use eyre::Result;
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use std::path::PathBuf;

impl ObjectManager for Importer {
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

impl Kibana<Importer> {
    pub fn push(&self) -> Result<String> {
        let count = objects::bundle(&self.objects.file, &self.objects.path)?;
        self.import()?;
        Ok(format!("Pushed {} objects", count.cyan()))
    }

    fn import(&self) -> Result<()> {
        let importer = &self.objects;
        let client = reqwest::blocking::Client::new();
        let import_url = format!(
            "{}/s/{}/api/saved_objects/_import?overwrite=true",
            importer.url, "default"
        );
        log::debug!("Import URL: {}", import_url);
        let form = reqwest::blocking::multipart::Form::new().file("file", &importer.file)?;
        let response = client
            .post(import_url)
            .header("Authorization", &importer.auth_header)
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
        &self.objects.url
    }
}
