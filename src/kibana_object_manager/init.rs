use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs::File, io::Write, path::PathBuf};

pub fn update_gitignore() -> Result<()> {
    log::info!("Updating .gitignore");
    let git_ignore = PathBuf::from(".gitignore");
    let mut file = File::options()
        .create(true)
        .append(true)
        .open(&git_ignore)?;
    let lines = vec![
        "# Added by --{kibob}-> (Kibana Object Manager)\n",
        ".env*\n",
        "export.ndjson\n",
        "import.ndjson\n",
        "import/\n",
        "response.json\n",
        "manifest_patch.json\n",
    ];

    let existing_content = std::fs::read_to_string(&git_ignore)?;

    for line in lines {
        if !existing_content.contains(line) {
            write!(file, "{}", line)?;
        }
    }
    Ok(())
}

pub fn generate_manifest(manifest_file: &PathBuf, export_file: &PathBuf) -> Result<Manifest> {
    log::debug!("Reading export file: {}", export_file.display());
    let export_ndjson: Vec<serde_json::Value> = std::fs::read_to_string(&export_file)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    log::debug!("Manifest NDJSON objects: {:?}", export_ndjson.len());
    log::debug!("Generating manifest: {}", manifest_file.display());
    let file = File::create(&manifest_file)?;
    let mut file = std::io::BufWriter::new(file);
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
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    Ok(manifest)
}

#[derive(Clone, Deserialize, Serialize)]
struct Object {
    #[serde(rename = "type")]
    object_type: String,
    id: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Manifest {
    objects: Vec<Object>,
    exclude_export_details: bool,
    include_references_deep: bool,
}

impl Manifest {
    fn new() -> Self {
        Manifest {
            objects: Vec::new(),
            exclude_export_details: false,
            include_references_deep: false,
        }
    }

    fn push(&mut self, object: Object) {
        self.objects.push(object);
    }

    fn sort(&mut self) {
        self.objects
            .sort_by(|a, b| a.object_type.cmp(&b.object_type).then(a.id.cmp(&b.id)));
    }
}
