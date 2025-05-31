use super::{Kibana, Manifest, ObjectManager, exporter::export};
use eyre::Result;
use std::{collections::HashMap, path::PathBuf};

pub struct FileMerger {
    pub file_in: PathBuf,
    pub file_out: PathBuf,
    pub manifest: Manifest,
}

impl ObjectManager for FileMerger {
    fn to_string(&self) -> String {
        format!("{}", self.file_in.display())
    }
}

impl Kibana<FileMerger> {
    pub fn merge(&self) -> Result<()> {
        Ok(())
    }
}

impl ObjectManager for ExportMerger {
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

impl Kibana<ExportMerger> {
    pub fn merge(&self) -> Result<()> {
        export(
            &self.objects.url,
            &self.objects.auth_header,
            &self.objects.manifest,
            &self.objects.file_in,
        )?;
        Ok(())
    }
}
