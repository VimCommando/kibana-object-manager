use super::{Kibana, Manifest, ObjectManager, objects};
use eyre::Result;
use owo_colors::OwoColorize;
use std::path::PathBuf;

impl ObjectManager for Importer {}

pub struct Importer {
    pub auth_header: String,
    pub file: PathBuf,
    pub manifest: Manifest,
    pub path: PathBuf,
    pub url: String,
}

impl Kibana<Importer> {
    pub fn push(&self) -> Result<String> {
        objects::bundle(&self.objects.file, &self.objects.path)?;
        self.import()?;
        Ok(String::from("Push"))
    }

    fn import(&self) -> Result<()> {
        log::warn!("{} not implemented!", "Importer".magenta());
        // TODO: implement
        Ok(())
    }

    pub fn url(&self) -> &str {
        &self.objects.url
    }
}
