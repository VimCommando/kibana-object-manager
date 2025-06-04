use eyre::{Result, eyre};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs::File, path::PathBuf};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    objects: Vec<Object>,
    exclude_export_details: bool,
    include_references_deep: bool,
}

impl Manifest {
    pub fn len(&self) -> usize {
        self.objects.len()
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Object {
    #[serde(rename = "type")]
    object_type: String,
    id: String,
}

impl Manifest {
    /// Creates a new manifest.
    pub fn new() -> Self {
        Manifest {
            objects: Vec::new(),
            exclude_export_details: true,
            include_references_deep: true,
        }
    }

    /// Generate a manifest from a saved objects export file
    pub fn from_export(export_file: &PathBuf) -> Result<Self> {
        log::debug!(
            "Reading export file: {}",
            export_file.display().bright_black()
        );

        let export_ndjson: Vec<serde_json::Value> = std::fs::read_to_string(&export_file)?
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        log::debug!("Manifest NDJSON objects: {:?}", export_ndjson.len().cyan());

        let mut manifest = Manifest::new();
        for object in &export_ndjson {
            let id = object["id"].as_str().unwrap_or_default();
            match id {
                "" => continue,
                _ => manifest.push(Object {
                    object_type: object["type"].as_str().unwrap_or_default().to_string(),
                    id: id.to_string(),
                }),
            }
        }
        manifest.sort();
        Ok(manifest)
    }

    pub fn from_export_list(export_list: HashMap<String, String>) -> Result<Self> {
        let mut manifest = Manifest::new();
        for (object_type, id) in export_list {
            manifest.push(Object { object_type, id });
        }
        manifest.sort();
        Ok(manifest)
    }

    pub fn merge(mut self, other: Self) -> Result<Self> {
        self.objects.extend(other.objects);
        self.objects
            .dedup_by(|a, b| a.object_type == b.object_type && a.id == b.id);
        self.sort();
        Ok(self)
    }

    /// Reads a manifest file from the given path.
    pub fn read(path: &PathBuf) -> Result<Self> {
        log::debug!("Reading manifest file: {}", path.display().bright_black());
        serde_json::from_reader(File::open(&path)?)
            .map_err(|e| eyre!("Failed to read manifest file: {}", e))
    }

    /// Writes the manifest to the given path.
    pub fn write(&self, path: &PathBuf) -> Result<()> {
        log::debug!(
            "Writing {} entries to manifest file: {}",
            self.objects.len().cyan(),
            path.display().bright_black()
        );
        let file = File::create(&path)?;
        let mut file = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut file, self)
            .map_err(|e| eyre!("Failed to write manifest file: {}", e))
    }

    /// Adds an object to the manifest.
    pub fn push(&mut self, object: Object) {
        self.objects.push(object);
    }

    /// Sorts the objects in the manifest by type then ID.
    pub fn sort(&mut self) {
        self.objects
            .sort_by(|a, b| a.object_type.cmp(&b.object_type).then(a.id.cmp(&b.id)));
    }
}
