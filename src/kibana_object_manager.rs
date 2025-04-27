mod add;
mod init;
mod pull;
mod push;
mod togo;

use eyre::{Result, eyre};
use init::Manifest;
use init::{generate_manifest, update_gitignore};
use jsrmx::{input::JsonReaderInput, output::Output, processor::NdjsonUnbundler};
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

#[derive(Debug)]
pub struct KibanaObjectManagerBuilder {
    export_file: Option<PathBuf>,
    export_path: Option<PathBuf>,
    apikey: Option<String>,
    password: Option<String>,
    url: String,
    username: Option<String>,
    manifest_file: Option<PathBuf>,
}

impl KibanaObjectManagerBuilder {
    pub fn new(kibana_url: String) -> Self {
        Self {
            export_file: None,
            export_path: None,
            apikey: None,
            password: None,
            url: kibana_url,
            username: None,
            manifest_file: None,
        }
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
            export_file: Some(export_file),
            export_path: Some(export_path),
            manifest_file: Some(manifest_file),
            ..self
        }
    }

    pub fn manifest_file(self, manifest_file: PathBuf) -> Self {
        let manifest_file = match &self.export_path {
            Some(path) => path.join(manifest_file),
            None => manifest_file,
        };

        Self {
            manifest_file: Some(manifest_file),
            ..self
        }
    }

    #[allow(deprecated)]
    pub fn build(self) -> Result<KibanaObjectManager> {
        let auth_header = match (self.apikey, self.username, self.password) {
            (Some(apikey), _, _) => format!("Apikey {}", apikey),
            (_, Some(username), Some(password)) => format!(
                "Basic {}",
                base64::encode(format!("{}:{}", username, password))
            ),
            _ => String::from("None"),
        };

        Ok(KibanaObjectManager {
            auth_header,
            export_file: self.export_file.unwrap_or(PathBuf::from("export.json")),
            export_path: self.export_path.unwrap_or(PathBuf::from(".")),
            kibana_url: self.url,
            manifest_file: self.manifest_file.unwrap_or(PathBuf::from("manifest.json")),
        })
    }

    pub fn username(self, username: Option<String>) -> Self {
        Self { username, ..self }
    }

    pub fn password(self, password: Option<String>) -> Self {
        Self { password, ..self }
    }

    pub fn apikey(self, apikey: Option<String>) -> Self {
        Self { apikey, ..self }
    }
}

pub struct KibanaObjectManager {
    auth_header: String,
    export_file: PathBuf,
    export_path: PathBuf,
    kibana_url: String,
    manifest_file: PathBuf,
}

impl KibanaObjectManager {
    pub fn url(&self) -> &str {
        &self.kibana_url
    }

    pub fn initialize(&self) -> Result<()> {
        update_gitignore()?;
        generate_manifest(&self.manifest_file, &self.export_file)?;
        self.unbundle_objects()?;
        self.drop_fields()
    }

    fn unbundle_objects(&self) -> Result<()> {
        let export_str = self
            .export_file
            .to_str()
            .ok_or_else(|| eyre!("Failed to convert export path to string"))?;
        let input = JsonReaderInput::from_str(export_str)?;
        let output_str = self.export_path.join("objects");
        let output_str = output_str
            .to_str()
            .expect("Failed to convert output path to string");
        let mut output = Output::from_str(output_str).map_err(|e| eyre!(e.to_string()))?;
        output.set_pretty();

        let unescape_fields = Some(vec![
            String::from("attributes.panelsJSON"),
            String::from("attributes.fieldFormatMap"),
            String::from("attributes.controlGroupInput.ignoreParentSettingsJSON"),
            String::from("attributes.controlGroupInput.panelsJSON"),
            String::from("attributes.kibanaSavedObjectMeta.searchSourceJSON"),
            String::from("attributes.optionsJSON"),
            String::from("attributes.visState"),
            String::from("attributes.fieldAttrs"),
        ]);
        let name = Some(vec![
            String::from("attributes.title"),
            String::from("attributes.name"),
        ]);
        let type_field = Some(String::from("type"));

        NdjsonUnbundler::new(input, output, unescape_fields)
            .unbundle(name, type_field)
            .map_err(|e| eyre!(e.to_string()))
    }

    pub fn test_authorization(&self) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let response = client
            .get(format!("{}/api/spaces/space?", self.kibana_url))
            .header("Authorization", &self.auth_header)
            .send()?;

        if response.status().is_success() {
            let body = response.json::<serde_json::Value>()?;
            log::debug!("Response body: {}", body);
            let name = body[0]["name"].as_str().unwrap_or("Unknown");
            let description = body[0]["description"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string()
                .chars()
                .take(30)
                .collect::<String>();
            let description = match description.len() > 30 {
                true => format!("{}...", description),
                false => description,
            };
            Ok(format!(
                "Kibana's default space is {}: {}",
                name.cyan(),
                description.bright_black()
            ))
        } else {
            let body = response.text()?;
            log::debug!("Response body: {}", body);
            Err(eyre!("Authorization failed: {}", body))
        }
    }

    pub fn drop_fields(&self) -> Result<()> {
        log::debug!("Dropping untracked fields");
        let objects_dir = self.export_path.join("objects");
        for entry in std::fs::read_dir(objects_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)?;
                let mut obj: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&path)?)?;
                file.set_len(0)?;
                let mut writer = std::io::BufWriter::new(file);
                if let serde_json::Value::Object(ref mut map) = obj {
                    map.remove("created_at");
                    map.remove("created_by");
                    map.remove("count");
                    map.remove("managed");
                    map.remove("updated_at");
                    map.remove("updated_by");
                    map.remove("version");
                }
                writeln!(writer, "{}", serde_json::to_string_pretty(&obj)?)?;
                writer.flush()?;
            }
        }
        Ok(())
    }

    pub fn pull(&self) -> Result<String> {
        self.export_saved_objects()?;
        self.unbundle_objects()?;
        self.drop_fields()?;
        Ok(String::from("Pull"))
        // drop_fields
        // unbundle_saved_objects
    }

    pub fn read_manifest(&self) -> Result<Manifest> {
        log::debug!(
            "Reading file {}",
            self.manifest_file.display().bright_black()
        );
        serde_json::from_reader(File::open(&self.manifest_file)?)
            .map_err(|e| eyre!("Failed to read manifest file: {}", e))
    }

    fn export_saved_objects(&self) -> Result<()> {
        let client = reqwest::blocking::Client::new();
        let manifest = self.read_manifest()?;
        let export_url = format!(
            "{}/s/{}/api/saved_objects/_export",
            self.kibana_url, "default"
        );
        log::debug!("Export URL: {}", export_url.bright_blue());
        let response = client
            .post(export_url)
            .header("Authorization", &self.auth_header)
            .header(
                "Content-Type",
                "application/json; Elastic-Api-Version=2023-10-31",
            )
            .header("kbn-xsrf", "string")
            .json(&manifest)
            .send()?;
        match response.status() {
            StatusCode::OK => {
                log::debug!("Export response status: {}", response.status().cyan());
                let body = response.text()?;
                let lines = body.lines();
                let file = File::create(&self.export_file)?;
                let mut writer = BufWriter::new(file);
                let mut count = 0;
                for line in lines {
                    writeln!(writer, "{}", line)?;
                    count += 1;
                }
                log::debug!(
                    "Saved {} objects to file {}",
                    count.cyan(),
                    self.export_file.display().bright_black()
                );
                Ok(())
            }
            _ => {
                log::debug!("Export response status: {}", response.status().magenta());
                let body = response.text()?;
                log::debug!("Export response body: {}", body.red());
                return Err(eyre!("Failed to export saved objects"));
            }
        }
    }
}
