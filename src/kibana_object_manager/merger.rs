use super::{Manifest, exporter::export};
use eyre::Result;
use std::{collections::HashMap, path::PathBuf};

pub struct FileMerger {
    pub file_in: PathBuf,
    pub file_out: PathBuf,
    pub manifest: Manifest,
}

impl ToString for FileMerger {
    fn to_string(&self) -> String {
        format!("{}", self.file_in.display())
    }
}

impl FileMerger {
    pub fn merge(&self) -> Result<()> {
        Ok(())
    }
}

impl ToString for ExportMerger {
    fn to_string(&self) -> String {
        format!("{}", self.url)
    }
}

pub struct ExportMerger {
    pub auth_header: String,
    pub file_in: PathBuf,
    pub file_out: PathBuf,
    pub manifest: Manifest,
    pub url: String,
    pub export_list: HashMap<String, String>,
}

impl ExportMerger {
    pub fn merge(&self) -> Result<()> {
        export(&self.url, &self.auth_header, &self.manifest, &self.file_in)?;
        Ok(())
    }
}
