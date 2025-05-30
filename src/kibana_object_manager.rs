mod adder;
mod authorizer;
mod bundler;
mod exporter;
mod importer;
mod initializer;
mod manifest;
mod objects;

use authorizer::Authorizer;
use bundler::Bundler;
use exporter::Exporter;
use eyre::{OptionExt, Result, eyre};
use importer::Importer;
use initializer::Initializer;
use manifest::Manifest;
use owo_colors::OwoColorize;
use std::{fs::File, path::PathBuf};

pub trait ObjectManager {
    fn to_string(&self) -> String;
}

pub struct Kibana<O: ObjectManager> {
    objects: O,
}

impl<O: ObjectManager> Kibana<O> {
    pub fn source(&self) -> String {
        self.objects.to_string()
    }
}

#[derive(Debug)]
pub struct KibanaObjectManagerBuilder {
    apikey: Option<String>,
    file: Option<PathBuf>,
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
            file: None,
            is_managed: false,
            manifest: None,
            password: None,
            path: None,
            url: kibana_url,
            username: None,
        }
    }

    pub fn build_authorizer(self) -> Result<Kibana<Authorizer>> {
        Ok(Kibana {
            objects: Authorizer {
                auth_header: self.format_auth_header(),
                url: self.url,
            },
        })
    }

    pub fn build_bundler(self) -> Result<Kibana<Bundler>> {
        let manifest = self.read_manifest()?;
        Ok(Kibana {
            objects: Bundler {
                file: self.file.ok_or_eyre("Bundler file not provided")?,
                manifest,
                path: self.path.ok_or_eyre("Bundler path not provided")?,
                is_managed: self.is_managed,
            },
        })
    }

    pub fn build_exporter(self) -> Result<Kibana<Exporter>> {
        let manifest = self.read_manifest()?;
        Ok(Kibana {
            objects: Exporter {
                auth_header: self.format_auth_header(),
                file: self.file.ok_or_eyre("Export file not provided")?,
                manifest,
                path: self.path.ok_or_eyre("Export path not provided")?,
                url: self.url,
            },
        })
    }

    pub fn build_importer(self) -> Result<Kibana<Importer>> {
        let manifest = self.read_manifest()?;
        Ok(Kibana {
            objects: Importer {
                auth_header: self.format_auth_header(),
                file: self.file.ok_or_eyre("Import file not provided")?,
                manifest,
                path: self.path.ok_or_eyre("Import path not provided")?,
                url: self.url,
            },
        })
    }

    pub fn build_initializer(self) -> Result<Kibana<Initializer>> {
        Ok(Kibana {
            objects: Initializer {
                file: self.file.ok_or_eyre("Init file not provided")?,
                manifest: self.manifest.ok_or_eyre("Init manifest not provided")?,
            },
        })
    }

    pub fn manifest(self, path: PathBuf) -> Self {
        log::debug!("Self manifest {:?}", &self.manifest.bright_black());
        let manifest_path = match self.manifest {
            Some(_) if path.is_file() => path,
            Some(manifest) if path.is_dir() => path.join(manifest),
            Some(manifest) => manifest,
            None if path.is_dir() => path.join("manifest.json"),
            None if path.is_file() => path,
            None => path,
        };

        Self {
            manifest: Some(manifest_path),
            ..self
        }
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
        let manifest_file = export_path.join("manifest.json");
        log::debug!("Export file: {}", export_file.display().bright_black());
        Self {
            file: Some(export_file),
            path: Some(export_path),
            manifest: Some(manifest_file),
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
            file: Some(import_file),
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
        let manifest = match &self.manifest {
            Some(manifest) => {
                log::debug!("Reading file {}", manifest.display().bright_black());
                serde_json::from_reader(File::open(&manifest)?)
                    .map_err(|e| eyre!("Failed to read manifest file: {}", e))
            }
            None => Err(eyre!("Manifest file not given")),
        }?;

        Ok(manifest)
    }
}
