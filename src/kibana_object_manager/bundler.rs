use crate::kibana_object_manager::objects;

use super::{Kibana, Manifest, ObjectManager};
use eyre::Result;
use owo_colors::OwoColorize;
use std::path::PathBuf;

impl ObjectManager for Bundler {
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

impl Kibana<Bundler> {
    pub fn bundle(&self) -> Result<String> {
        let bundler = &self.objects;
        log::warn!("{} not implemented!", "Bundler".magenta());
        let count = objects::bundle(&bundler.file, &bundler.path)?;
        Ok(format!(
            "bundled {} saved objects into {}",
            count.cyan(),
            bundler.file.display().bright_black()
        ))
    }
}
