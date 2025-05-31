use crate::kibana_object_manager::objects;

use super::Manifest;
use eyre::Result;
use owo_colors::OwoColorize;
use std::path::PathBuf;

impl ToString for Bundler {
    fn to_string(&self) -> String {
        format!("{}", self.file.display())
    }
}

pub struct Bundler {
    pub file: PathBuf,
    pub manifest: Manifest,
    pub path: PathBuf,
    pub is_managed: bool,
}

impl Bundler {
    pub fn bundle(&self) -> Result<String> {
        log::warn!("{} not implemented!", "Bundler".magenta());
        let count = objects::bundle(&self.file, &self.path)?;
        Ok(format!(
            "bundled {} saved objects into {}",
            count.cyan(),
            self.file.display().bright_black()
        ))
    }
}
