use super::objects;
use super::{Manifest, exporter};
use eyre::{OptionExt, Result};
use owo_colors::OwoColorize;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

pub struct Merger<T> {
    pub export_ndjson: PathBuf,
    pub manifest: Manifest,
    pub data: T,
}

pub trait ObjectReader {
    fn read(self) -> Result<Values>;
}

impl<T: ObjectReader> Merger<T> {
    pub fn read(self) -> Result<Merger<Values>> {
        Ok(Merger {
            export_ndjson: self.export_ndjson,
            manifest: self.manifest,
            data: self.data.read()?,
        })
    }
}

impl Merger<Values> {
    pub fn merge(self) -> Result<()> {
        log::debug!(
            "Merging {} saved objects into {} existing",
            self.data.objects.len().cyan(),
            self.manifest.len().cyan()
        );
        exporter::write_ndjson(self.data.objects, &self.export_ndjson)?;
        let export_path = self
            .export_ndjson
            .parent()
            .ok_or_eyre("Dafuq?")?
            .to_path_buf();
        objects::unbundle(&self.export_ndjson, &export_path)?;
        self.manifest
            .merge(self.data.manifest)?
            .write(&export_path.join("manifest.json"))?;
        Ok(())
    }
}

impl<S> ToString for Merger<S>
where
    S: ObjectReader + ToString,
{
    fn to_string(&self) -> String {
        format!("{}", self.data.to_string())
    }
}

pub struct Values {
    pub manifest: Manifest,
    pub objects: Vec<Value>,
}

pub struct Ndjson {
    pub merge_ndjson: PathBuf,
}

impl ObjectReader for Ndjson {
    fn read(self) -> Result<Values> {
        let manifest = Manifest::from_export(&self.merge_ndjson)?;
        let file = File::open(self.merge_ndjson)?;
        let reader = BufReader::new(file);
        let objects: Vec<Value> = reader
            .lines()
            .filter_map(|line| line.ok().map(|line| serde_json::from_str(&line).ok()))
            .flatten()
            .collect();

        Ok(Values { manifest, objects })
    }
}

impl ToString for Ndjson {
    fn to_string(&self) -> String {
        format!("{}", self.merge_ndjson.display())
    }
}

pub struct Kibana {
    pub auth_header: String,
    pub url: String,
    pub manifest: Manifest,
}

impl ObjectReader for Kibana {
    fn read(self) -> Result<Values> {
        Ok(Values {
            objects: exporter::export_saved_objects(&self.url, &self.auth_header, &self.manifest)?,
            manifest: self.manifest,
        })
    }
}

impl ToString for Kibana {
    fn to_string(&self) -> String {
        format!("{}", self.url)
    }
}
