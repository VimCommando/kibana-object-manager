mod authorizer;
mod bundler;
mod exporter;
mod importer;
mod initializer;
mod manifest;
mod merger;
mod objects;

use authorizer::Authorizer;
use bundler::Bundler;
use exporter::Exporter;
use eyre::{OptionExt, Result, eyre};
use importer::Importer;
use initializer::Initializer;
use manifest::Manifest;
use merger::{ExportMerger, FileMerger};
use owo_colors::OwoColorize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug)]
pub struct KibanaObjectManagerBuilder {
    apikey: Option<String>,
    file_in: Option<PathBuf>,
    file_out: Option<PathBuf>,
    is_managed: bool,
    manifest: Option<PathBuf>,
    password: Option<String>,
    path: Option<PathBuf>,
    url: String,
    username: Option<String>,
}

impl KibanaObjectManagerBuilder {
    pub fn new(kibana_url: String) -> Self {
        Self {
            apikey: None,
            file_in: None,
            file_out: None,
            is_managed: false,
            manifest: None,
            password: None,
            path: None,
            url: kibana_url,
            username: None,
        }
    }

    pub fn build_file_merger(self) -> Result<FileMerger> {
        Ok(FileMerger {
            manifest: self.read_manifest()?,
            file_in: self.file_in.ok_or_eyre("No merge file provided")?,
            file_out: self.file_out.ok_or_eyre("No export file provided")?,
        })
    }

    pub fn build_export_merger(self) -> Result<ExportMerger> {
        Ok(ExportMerger {
            auth_header: self.format_auth_header(),
            manifest: self.read_manifest()?,
            file_in: self.file_in.ok_or_eyre("Merge file not provided")?,
            file_out: self.file_out.ok_or_eyre("Export file not provided")?,
            url: self.url,
            export_list: HashMap::new(),
        })
    }

    pub fn build_authorizer(self) -> Result<Authorizer> {
        Ok(Authorizer {
            auth_header: self.format_auth_header(),
            url: self.url,
        })
    }

    pub fn build_bundler(self) -> Result<Bundler> {
        Ok(Bundler {
            manifest: self.read_manifest()?,
            file: self.file_in.ok_or_eyre("Bundler file not provided")?,
            path: self.path.ok_or_eyre("Bundler path not provided")?,
            is_managed: self.is_managed,
        })
    }

    pub fn build_exporter(self) -> Result<Exporter> {
        Ok(Exporter {
            auth_header: self.format_auth_header(),
            manifest: self.read_manifest()?,
            file: self.file_out.ok_or_eyre("Export file not provided")?,
            path: self.path.ok_or_eyre("Export path not provided")?,
            url: self.url,
        })
    }

    pub fn build_importer(self) -> Result<Importer> {
        Ok(Importer {
            auth_header: self.format_auth_header(),
            manifest: self.read_manifest()?,
            file: self.file_in.ok_or_eyre("Import file not provided")?,
            path: self.path.ok_or_eyre("Import path not provided")?,
            url: self.url,
        })
    }

    pub fn build_initializer(self) -> Result<Initializer> {
        Ok(Initializer {
            file: self.file_out.ok_or_eyre("No export file provided")?,
            manifest: self.manifest.ok_or_eyre("No manifest file provided")?,
        })
    }

    pub fn username(self, username: Option<String>) -> Self {
        Self { username, ..self }
    }

    pub fn apikey(self, apikey: Option<String>) -> Self {
        Self { apikey, ..self }
    }

    pub fn password(self, password: Option<String>) -> Self {
        Self { password, ..self }
    }

    pub fn managed(self, is_managed: bool) -> Self {
        Self { is_managed, ..self }
    }

    pub fn export_path(self, export_path: PathBuf) -> Self {
        let (export_file, export_path) = match export_path.is_dir() {
            true => (export_path.join("export.ndjson"), export_path),
            false => (
                export_path.clone(),
                export_path
                    .parent()
                    .expect("Export path must have a parent directory")
                    .to_path_buf(),
            ),
        };
        log::debug!("Export file: {}", export_file.display().bright_black());
        Self {
            file_out: Some(export_file),
            path: Some(export_path),
            ..self
        }
    }

    pub fn manifest(self, manifest: PathBuf) -> Self {
        log::debug!("Self manifest {:?}", &self.manifest.bright_black());
        let manifest_path = match &self.path {
            _ if manifest.is_dir() => manifest.join("manifest.json"),
            Some(path) => path.join("manifest.json"),
            None => manifest,
        };

        Self {
            manifest: Some(manifest_path),
            ..self
        }
    }

    pub fn import_path(self, import_path: PathBuf) -> Self {
        let (import_file, import_path) = match import_path.is_dir() {
            true => (import_path.join("import.ndjson"), import_path),
            false => (
                import_path.clone(),
                import_path
                    .parent()
                    .expect("Import path must have a parent directory")
                    .to_path_buf(),
            ),
        };
        let manifest_file = import_path.join("manifest.json");
        log::debug!("Import file: {}", import_file.display().bright_black());
        Self {
            file_in: Some(import_file),
            path: Some(import_path),
            manifest: Some(manifest_file),
            ..self
        }
    }

    #[allow(deprecated)]
    fn format_auth_header(&self) -> String {
        match (&self.apikey, &self.username, &self.password) {
            (Some(apikey), _, _) => format!("Apikey {}", apikey),
            (_, Some(username), Some(password)) => format!(
                "Basic {}",
                base64::encode(format!("{}:{}", username, password))
            ),
            _ => String::from("None"),
        }
    }

    fn read_manifest(&self) -> Result<Manifest> {
        match self.manifest {
            Some(ref manifest) => Manifest::read(&manifest),
            None => Err(eyre!("Missing manifest file path")),
        }
    }
}
