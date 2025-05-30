use super::{Kibana, Manifest, ObjectManager};
use owo_colors::OwoColorize;
use std::path::PathBuf;

impl ObjectManager for Bundler {}

pub struct Bundler {
    pub file: PathBuf,
    pub manifest: Manifest,
    pub path: PathBuf,
}

impl Kibana<Bundler> {
    pub fn bundle(&self) {
        let bundler = &self.objects;
        log::warn!("{} not implemented!", "Bundler".magenta());
        log::warn!(
            "From {} to {}",
            bundler.path.display(),
            bundler.file.display(),
        );
        todo!()
        // Implementation for bundling objects
    }
}
