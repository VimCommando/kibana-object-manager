use super::{Manifest, objects};
use eyre::{Result, eyre};
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

impl ToString for Exporter {
    fn to_string(&self) -> String {
        format!("{}", self.url)
    }
}

pub struct Exporter {
    pub auth_header: String,
    pub file: PathBuf,
    pub manifest: Manifest,
    pub path: PathBuf,
    pub url: String,
}

impl Exporter {
    pub fn pull(&self) -> Result<String> {
        let objects = export_saved_objects(&self.url, &self.auth_header, &self.manifest)?;
        write_ndjson(objects, &self.path)?;
        objects::unbundle(&self.file, &self.path)?;
        Ok(String::from("Pull"))
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

pub fn export_saved_objects(
    url: &String,
    auth_header: &String,
    manifest: &Manifest,
) -> Result<Vec<Value>> {
    let client = reqwest::blocking::Client::new();
    let export_url = format!("{}/s/{}/api/saved_objects/_export", url, "default");
    log::debug!("Export URL: {}", export_url.bright_blue());
    let response = client
        .post(export_url)
        .header("Authorization", auth_header)
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
            let objects: Vec<Value> = response
                .text()?
                .lines()
                .filter_map(|line| serde_json::from_str(&line).ok())
                .collect();
            log::debug!("Exported {} saved objects", objects.len().cyan());
            log::trace!("{:?}", objects);
            Ok(objects)
        }
        _ => {
            log::debug!("Export response status: {}", response.status().magenta());
            let body = response.text()?;
            log::trace!("Export response body: {}", body.red());
            return Err(eyre!("Failed to export saved objects"));
        }
    }
}

pub fn write_ndjson(objects: Vec<Value>, path: &PathBuf) -> Result<()> {
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    let mut count = 0;
    for object in objects {
        writeln!(writer, "{}", serde_json::to_string(&object)?)?;
        count += 1;
    }
    log::debug!(
        "Saved {} objects to file {}",
        count.cyan(),
        path.display().bright_black()
    );
    Ok(())
}
