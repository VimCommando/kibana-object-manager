//! CLI helper functions

use crate::{
    client::{Auth, Kibana},
    etl::{Extractor, Loader, Transformer},
    kibana::saved_objects::{SavedObjectsExtractor, SavedObjectsLoader},
    kibana::spaces::{SpacesExtractor, SpacesLoader, SpacesManifest},
    migration::load_saved_objects_manifest,
    storage::{DirectoryReader, DirectoryWriter},
    transform::{FieldDropper, FieldEscaper, FieldUnescaper, ManagedFlagAdder},
};
use eyre::{Context, Result};
use std::path::Path;
use url::Url;

/// Load Kibana client from environment variables
///
/// Expected environment variables:
/// - KIBANA_URL: Kibana base URL (required)
/// - KIBANA_USERNAME: Username for basic auth (optional)
/// - KIBANA_PASSWORD: Password for basic auth (optional)
/// - KIBANA_APIKEY: API key for auth (optional, conflicts with username/password)
/// - KIBANA_SPACE: Default space (optional, defaults to None for global)
pub fn load_kibana_client() -> Result<Kibana> {
    let url_str = std::env::var("KIBANA_URL").context("KIBANA_URL environment variable not set")?;
    let url = Url::parse(&url_str).with_context(|| format!("Invalid KIBANA_URL: {}", url_str))?;

    let auth = if let Ok(apikey) = std::env::var("KIBANA_APIKEY") {
        Auth::Apikey(apikey)
    } else if let (Ok(username), Ok(password)) = (
        std::env::var("KIBANA_USERNAME"),
        std::env::var("KIBANA_PASSWORD"),
    ) {
        Auth::Basic(username, password)
    } else {
        Auth::None
    };

    let space = std::env::var("KIBANA_SPACE").ok();

    Kibana::try_new(url, auth, space).context("Failed to create Kibana client")
}

/// Pull saved objects from Kibana to local directory
///
/// Pipeline: SavedObjectsExtractor → FieldDropper → FieldUnescaper → DirectoryWriter
/// Also pulls spaces if manifest/spaces.yml exists
pub async fn pull_saved_objects(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading manifest from {}", project_dir.display());
    let manifest = load_saved_objects_manifest(project_dir)?;
    log::info!("Manifest loaded: {} objects", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    // Get space from client or use "default" (need to do this before moving client)
    let space = client
        .space()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default".to_string());
    log::info!("Using space: {}", space);

    // Build the extract pipeline
    log::info!("Extracting objects from Kibana...");
    let extractor = SavedObjectsExtractor::new(client.clone(), manifest, &space);

    // Transform: Drop metadata fields and unescape JSON strings
    let drop_fields = FieldDropper::default_kibana_fields();
    let unescape = FieldUnescaper::default_kibana_fields();

    // Load to directory
    let objects_dir = project_dir.join("objects");
    let writer = DirectoryWriter::new_with_options(&objects_dir, true)?;

    // Clear directory before writing
    writer.clear()?;

    // Extract → Drop → Unescape → Load
    let objects = extractor.extract().await?;
    let dropped = drop_fields.transform_many(objects)?;
    let unescaped = unescape.transform_many(dropped)?;
    let count = writer.load(unescaped).await?;

    log::info!("✓ Pulled {} object(s) to {}", count, objects_dir.display());

    // Also pull spaces if manifest exists
    let spaces_manifest_path = project_dir.join("manifest/spaces.yml");
    if spaces_manifest_path.exists() {
        log::info!("Spaces manifest found, pulling spaces...");
        match pull_spaces_internal(project_dir, client).await {
            Ok(space_count) => {
                log::info!("✓ Pulled {} space(s)", space_count);
            }
            Err(e) => {
                log::warn!("Failed to pull spaces: {}", e);
            }
        }
    }

    Ok(count)
}

/// Push saved objects from local directory to Kibana
///
/// Pipeline: DirectoryReader → FieldEscaper → ManagedFlagAdder → SavedObjectsLoader
/// Also pushes spaces if manifest/spaces.yml exists
pub async fn push_saved_objects(project_dir: impl AsRef<Path>, managed: bool) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading objects from {}", project_dir.display());
    let objects_dir = project_dir.join("objects");

    if !objects_dir.exists() {
        eyre::bail!("Objects directory not found: {}", objects_dir.display());
    }

    let reader = DirectoryReader::new(&objects_dir);

    // Transform: Escape JSON strings and add managed flag
    let escaper = FieldEscaper::default_kibana_fields();
    let managed_flag = ManagedFlagAdder::new(managed);

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    // Get space from client or use "default" (need to do this before moving client)
    let space = client
        .space()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default".to_string());
    log::info!("Using space: {}", space);
    log::info!("Managed flag: {}", managed);

    let loader = SavedObjectsLoader::new(client.clone(), &space);

    // Read → Escape → Add Managed Flag → Load
    log::info!("Loading objects...");
    let objects = reader.extract().await?;
    log::info!("Read {} object(s) from disk", objects.len());

    let escaped = escaper.transform_many(objects)?;
    let flagged = managed_flag.transform_many(escaped)?;
    let count = loader.load(flagged).await?;

    log::info!("✓ Pushed {} object(s) to Kibana", count);

    // Also push spaces if manifest exists
    let spaces_manifest_path = project_dir.join("manifest/spaces.yml");
    if spaces_manifest_path.exists() {
        log::info!("Spaces manifest found, pushing spaces...");
        match push_spaces_internal(project_dir, client).await {
            Ok(space_count) => {
                log::info!("✓ Pushed {} space(s)", space_count);
            }
            Err(e) => {
                log::warn!("Failed to push spaces: {}", e);
            }
        }
    }

    Ok(count)
}

/// Bundle saved objects to NDJSON file for distribution
///
/// Pipeline: DirectoryReader → FieldEscaper → ManagedFlagAdder → Write to NDJSON
/// Also bundles spaces to bundle/spaces.ndjson if manifest/spaces.yml exists
pub async fn bundle_to_ndjson(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
    managed: bool,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::info!("Loading objects from {}", project_dir.display());
    let objects_dir = project_dir.join("objects");

    if !objects_dir.exists() {
        eyre::bail!("Objects directory not found: {}", objects_dir.display());
    }

    let reader = DirectoryReader::new(&objects_dir);

    // Transform: Escape JSON strings and add managed flag
    let escaper = FieldEscaper::default_kibana_fields();
    let managed_flag = ManagedFlagAdder::new(managed);

    // Read → Escape → Add Managed Flag
    log::info!("Loading objects...");
    let objects = reader.extract().await?;
    log::info!("Read {} object(s) from disk", objects.len());

    let escaped = escaper.transform_many(objects)?;
    let flagged = managed_flag.transform_many(escaped)?;

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for obj in &flagged {
        let json_line = serde_json::to_string(obj)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} object(s) to {}",
        flagged.len(),
        output_file.display()
    );

    // Also bundle spaces if manifest exists
    let spaces_manifest_path = project_dir.join("manifest/spaces.yml");
    if spaces_manifest_path.exists() {
        log::info!("Spaces manifest found, bundling spaces...");
        // Get bundle directory from output_file path
        let bundle_dir = output_file
            .parent()
            .ok_or_else(|| eyre::eyre!("Could not determine bundle directory"))?;
        let spaces_output = bundle_dir.join("spaces.ndjson");
        match bundle_spaces_to_ndjson_internal(project_dir, &spaces_output).await {
            Ok(space_count) => {
                log::info!("✓ Bundled {} space(s)", space_count);
            }
            Err(e) => {
                log::warn!("Failed to bundle spaces: {}", e);
            }
        }
    }

    Ok(flagged.len())
}

/// Initialize a new manifest from an export.ndjson file
///
/// Reads an NDJSON export file and generates a manifest
pub async fn init_from_export(
    export_path: impl AsRef<Path>,
    manifest_dir: impl AsRef<Path>,
) -> Result<usize> {
    use crate::kibana::saved_objects::{SavedObject, SavedObjectsManifest};
    use std::io::{BufRead, BufReader};

    let export_path = export_path.as_ref();
    let manifest_dir = manifest_dir.as_ref();

    log::info!("Reading export from {}", export_path.display());

    // Read NDJSON file
    let file = std::fs::File::open(export_path)?;
    let reader = BufReader::new(file);

    let mut objects = Vec::new();
    let mut saved_objects = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let obj: serde_json::Value = serde_json::from_str(&line)?;

        // Extract type and id for manifest
        if let (Some(obj_type), Some(obj_id)) = (
            obj.get("type").and_then(|v| v.as_str()),
            obj.get("id").and_then(|v| v.as_str()),
        ) {
            saved_objects.push(SavedObject::new(obj_type, obj_id));
        }

        objects.push(obj);
    }

    log::info!("Read {} object(s) from export", objects.len());

    // Create manifest from objects
    let manifest = SavedObjectsManifest::with_objects(saved_objects);

    // Create manifest directory
    let manifest_path = manifest_dir.join("manifest");
    std::fs::create_dir_all(&manifest_path)?;

    // Write manifest
    let manifest_file = manifest_path.join("saved_objects.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_file, manifest_json)?;

    log::info!("✓ Created manifest with {} object(s)", manifest.count());
    log::info!("  Manifest: {}", manifest_file.display());

    // Write objects to disk using DirectoryWriter
    let objects_dir = manifest_dir.join("objects");
    let writer = DirectoryWriter::new_with_options(&objects_dir, true)?;

    // Transform: Drop metadata and unescape
    let drop_fields = FieldDropper::default_kibana_fields();
    let unescape = FieldUnescaper::default_kibana_fields();

    let dropped = drop_fields.transform_many(objects)?;
    let unescaped = unescape.transform_many(dropped)?;

    // Load to directory
    use crate::etl::Loader;
    let count = writer.load(unescaped).await?;

    log::info!("  Objects: {}", objects_dir.display());
    log::info!("✓ Wrote {} object files", count);

    Ok(count)
}

/// Add objects to an existing manifest
///
/// Can add from Kibana or from a file
pub async fn add_objects_to_manifest(
    project_dir: impl AsRef<Path>,
    objects_to_add: Option<Vec<String>>,
    file_path: Option<impl AsRef<Path>>,
) -> Result<usize> {
    use crate::kibana::saved_objects::{SavedObject, SavedObjectsExtractor, SavedObjectsManifest};

    let project_dir = project_dir.as_ref();

    // Load existing manifest
    log::info!("Loading existing manifest from {}", project_dir.display());
    let mut manifest = load_saved_objects_manifest(project_dir)?;
    log::info!("Current manifest has {} objects", manifest.count());

    let new_objects = if let Some(file) = file_path {
        // Read from file
        let file_path = file.as_ref();
        log::info!("Reading objects from {}", file_path.display());

        use std::io::{BufRead, BufReader};
        let file = std::fs::File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut objs = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let obj: serde_json::Value = serde_json::from_str(&line)?;
            objs.push(obj);
        }
        objs
    } else if let Some(object_specs) = objects_to_add {
        // Fetch from Kibana
        log::info!("Fetching {} object(s) from Kibana", object_specs.len());
        let client = load_kibana_client()?;
        let space = client
            .space()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "default".to_string());

        // Parse object specs (format: "type=id" or "type:id")
        let mut saved_objects = Vec::new();
        for spec in &object_specs {
            let parts: Vec<&str> = spec.split(['=', ':']).collect();
            if parts.len() != 2 {
                eyre::bail!(
                    "Invalid object spec: {}. Expected format: type=id or type:id",
                    spec
                );
            }
            saved_objects.push(SavedObject::new(parts[0], parts[1]));
        }

        // Build a temporary manifest with just the objects we want to fetch
        let temp_manifest = SavedObjectsManifest::with_objects(saved_objects);

        // Use extractor to fetch these specific objects
        let extractor = SavedObjectsExtractor::new(client, temp_manifest, &space);
        extractor.extract().await?
    } else {
        eyre::bail!("Must specify either --objects or --file");
    };

    log::info!("Adding {} new object(s)", new_objects.len());

    // Add objects to manifest
    for obj in &new_objects {
        if let (Some(obj_type), Some(obj_id)) = (
            obj.get("type").and_then(|v| v.as_str()),
            obj.get("id").and_then(|v| v.as_str()),
        ) {
            manifest.add_object(SavedObject::new(obj_type, obj_id));
        }
    }

    // Save updated manifest
    let manifest_path = project_dir.join("manifest/saved_objects.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, manifest_json)?;

    log::info!("✓ Updated manifest now has {} objects", manifest.count());

    // Write new objects to disk
    let objects_dir = project_dir.join("objects");
    let writer = DirectoryWriter::new_with_options(&objects_dir, true)?;

    // Transform: Drop metadata and unescape
    let drop_fields = FieldDropper::default_kibana_fields();
    let unescape = FieldUnescaper::default_kibana_fields();

    let dropped = drop_fields.transform_many(new_objects)?;
    let unescaped = unescape.transform_many(dropped)?;
    let count = writer.load(unescaped).await?;

    log::info!("✓ Wrote {} new object file(s)", count);

    Ok(count)
}

/// Load spaces manifest from project directory
///
/// Reads `manifest/spaces.yml` from the project directory
fn load_spaces_manifest(project_dir: impl AsRef<Path>) -> Result<SpacesManifest> {
    let manifest_path = project_dir.as_ref().join("manifest/spaces.yml");

    if !manifest_path.exists() {
        eyre::bail!("Spaces manifest not found: {}", manifest_path.display());
    }

    SpacesManifest::read(&manifest_path)
}

/// Pull spaces from Kibana to local directory (internal)
///
/// Pipeline: SpacesExtractor → Write to spaces/<space_id>.json files
async fn pull_spaces_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading spaces manifest from {}", project_dir.display());
    let manifest = load_spaces_manifest(project_dir)?;
    log::debug!("Manifest loaded: {} space(s)", manifest.count());

    // Build the extract pipeline
    log::debug!("Extracting spaces from Kibana...");
    let extractor = SpacesExtractor::new(client, Some(manifest));

    // Extract spaces
    let spaces = extractor.extract().await?;

    // Write each space to its own JSON file
    let spaces_dir = project_dir.join("spaces");
    std::fs::create_dir_all(&spaces_dir)?;

    let mut count = 0;
    for space in &spaces {
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        let space_file = spaces_dir.join(format!("{}.json", space_id));
        let json = serde_json::to_string_pretty(space)?;
        std::fs::write(&space_file, json)?;

        log::debug!("Wrote space: {}", space_file.display());
        count += 1;
    }

    Ok(count)
}

/// Pull spaces from Kibana to local directory
///
/// Pipeline: SpacesExtractor → Write to spaces/<space_id>.json files
pub async fn pull_spaces(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading spaces manifest from {}", project_dir.display());
    let manifest = load_spaces_manifest(project_dir)?;
    log::info!("Manifest loaded: {} space(s)", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    // Build the extract pipeline
    log::info!("Extracting spaces from Kibana...");
    let extractor = SpacesExtractor::new(client, Some(manifest));

    // Extract spaces
    let spaces = extractor.extract().await?;

    // Write each space to its own JSON file
    let spaces_dir = project_dir.join("spaces");
    std::fs::create_dir_all(&spaces_dir)?;

    let mut count = 0;
    for space in &spaces {
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        let space_file = spaces_dir.join(format!("{}.json", space_id));
        let json = serde_json::to_string_pretty(space)?;
        std::fs::write(&space_file, json)?;

        log::debug!("Wrote space: {}", space_file.display());
        count += 1;
    }

    log::info!("✓ Pulled {} space(s) to {}", count, spaces_dir.display());

    Ok(count)
}

/// Push spaces from local directory to Kibana (internal)
///
/// Pipeline: Read from spaces/<space_id>.json → SpacesLoader
async fn push_spaces_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading spaces from {}", project_dir.display());
    let spaces_dir = project_dir.join("spaces");

    if !spaces_dir.exists() {
        eyre::bail!("Spaces directory not found: {}", spaces_dir.display());
    }

    // Read all JSON files from spaces directory
    let mut spaces = Vec::new();
    for entry in std::fs::read_dir(&spaces_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let space: serde_json::Value = serde_json::from_str(&content)?;
            spaces.push(space);
        }
    }

    log::debug!("Read {} space(s) from disk", spaces.len());

    let loader = SpacesLoader::new(client);

    // Load spaces to Kibana
    let count = loader.load(spaces).await?;

    Ok(count)
}

/// Push spaces from local directory to Kibana
///
/// Pipeline: Read from spaces/<space_id>.json → SpacesLoader
pub async fn push_spaces(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading spaces from {}", project_dir.display());
    let spaces_dir = project_dir.join("spaces");

    if !spaces_dir.exists() {
        eyre::bail!("Spaces directory not found: {}", spaces_dir.display());
    }

    // Read all JSON files from spaces directory
    let mut spaces = Vec::new();
    for entry in std::fs::read_dir(&spaces_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let space: serde_json::Value = serde_json::from_str(&content)?;
            spaces.push(space);
        }
    }

    log::info!("Read {} space(s) from disk", spaces.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    let loader = SpacesLoader::new(client);

    // Load spaces to Kibana
    let count = loader.load(spaces).await?;

    log::info!("✓ Pushed {} space(s) to Kibana", count);

    Ok(count)
}

/// Bundle spaces to NDJSON file for distribution (internal)
///
/// Pipeline: Read from spaces/<space_id>.json → Write to spaces.ndjson
async fn bundle_spaces_to_ndjson_internal(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::debug!("Loading spaces from {}", project_dir.display());
    let spaces_dir = project_dir.join("spaces");

    if !spaces_dir.exists() {
        eyre::bail!("Spaces directory not found: {}", spaces_dir.display());
    }

    // Read all JSON files from spaces directory
    let mut spaces = Vec::new();
    for entry in std::fs::read_dir(&spaces_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let space: serde_json::Value = serde_json::from_str(&content)?;
            spaces.push(space);
        }
    }

    log::debug!("Read {} space(s) from disk", spaces.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for space in &spaces {
        let json_line = serde_json::to_string(space)?;
        writeln!(file, "{}", json_line)?;
    }

    Ok(spaces.len())
}

/// Bundle spaces to NDJSON file for distribution
///
/// Pipeline: Read from spaces/<space_id>.json → Write to spaces.ndjson
pub async fn bundle_spaces_to_ndjson(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::info!("Loading spaces from {}", project_dir.display());
    let spaces_dir = project_dir.join("spaces");

    if !spaces_dir.exists() {
        eyre::bail!("Spaces directory not found: {}", spaces_dir.display());
    }

    // Read all JSON files from spaces directory
    let mut spaces = Vec::new();
    for entry in std::fs::read_dir(&spaces_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let space: serde_json::Value = serde_json::from_str(&content)?;
            spaces.push(space);
        }
    }

    log::info!("Read {} space(s) from disk", spaces.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for space in &spaces {
        let json_line = serde_json::to_string(space)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} space(s) to {}",
        spaces.len(),
        output_file.display()
    );

    Ok(spaces.len())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn test_load_kibana_client_no_url() {
        // Clear any existing env vars
        unsafe {
            std::env::remove_var("KIBANA_URL");
            std::env::remove_var("KIBANA_USERNAME");
            std::env::remove_var("KIBANA_PASSWORD");
            std::env::remove_var("KIBANA_APIKEY");
        }

        let result = load_kibana_client();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("KIBANA_URL"));
    }

    #[test]
    #[serial_test::serial]
    fn test_load_kibana_client_with_url() {
        unsafe {
            std::env::set_var("KIBANA_URL", "http://localhost:5601");
            std::env::remove_var("KIBANA_USERNAME");
            std::env::remove_var("KIBANA_PASSWORD");
            std::env::remove_var("KIBANA_APIKEY");
        }

        let result = load_kibana_client();
        assert!(result.is_ok());

        unsafe {
            std::env::remove_var("KIBANA_URL");
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_load_kibana_client_invalid_url() {
        unsafe {
            std::env::set_var("KIBANA_URL", "not-a-valid-url");
        }

        let result = load_kibana_client();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid KIBANA_URL")
        );

        unsafe {
            std::env::remove_var("KIBANA_URL");
        }
    }
}
