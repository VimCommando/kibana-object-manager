use super::{Kibana, Manifest, ObjectManager, objects};
use eyre::{Result, eyre};
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

impl ObjectManager for Exporter {
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

impl Kibana<Exporter> {
    pub fn pull(&self) -> Result<String> {
        export(
            &self.objects.url,
            &self.objects.auth_header,
            &self.objects.manifest,
            &self.objects.file,
        )?;
        objects::unbundle(&self.objects.file, &self.objects.path)?;
        Ok(String::from("Pull"))
    }

    pub fn url(&self) -> &str {
        &self.objects.url
    }
}

pub fn export(
    url: &String,
    auth_header: &String,
    manifest: &Manifest,
    path: &PathBuf,
) -> Result<()> {
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
            let body = response.text()?;
            let lines = body.lines();
            let file = File::create(&path)?;
            let mut writer = BufWriter::new(file);
            let mut count = 0;
            for line in lines {
                writeln!(writer, "{}", line)?;
                count += 1;
            }
            log::debug!(
                "Saved {} objects to file {}",
                count.cyan(),
                path.display().bright_black()
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
