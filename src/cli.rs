//! CLI helper functions

use crate::{
    client::{Auth, Kibana},
    etl::{Extractor, Loader, Transformer},
    kibana::agents::{AgentsExtractor, AgentsLoader, AgentsManifest},
    kibana::saved_objects::{SavedObjectsExtractor, SavedObjectsLoader},
    kibana::spaces::{SpacesExtractor, SpacesLoader, SpacesManifest},
    kibana::tools::{ToolsExtractor, ToolsLoader, ToolsManifest},
    kibana::workflows::{WorkflowsExtractor, WorkflowsLoader, WorkflowsManifest},
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
        match pull_spaces_internal(project_dir, client.clone()).await {
            Ok(space_count) => {
                log::info!("✓ Pulled {} space(s)", space_count);
            }
            Err(e) => {
                log::warn!("Failed to pull spaces: {}", e);
            }
        }
    }

    // Also pull workflows if manifest exists
    let workflows_manifest_path = project_dir.join("manifest/workflows.yml");
    if workflows_manifest_path.exists() {
        log::info!("Workflows manifest found, pulling workflows...");
        match pull_workflows_internal(project_dir, client.clone()).await {
            Ok(workflow_count) => {
                log::info!("✓ Pulled {} workflow(s)", workflow_count);
            }
            Err(e) => {
                log::warn!("Failed to pull workflows: {}", e);
            }
        }
    }

    // Also pull agents if manifest exists
    let agents_manifest_path = project_dir.join("manifest/agents.yml");
    if agents_manifest_path.exists() {
        log::info!("Agents manifest found, pulling agents...");
        match pull_agents_internal(project_dir, client.clone()).await {
            Ok(agent_count) => {
                log::info!("✓ Pulled {} agent(s)", agent_count);
            }
            Err(e) => {
                log::warn!("Failed to pull agents: {}", e);
            }
        }
    }

    // Also pull tools if manifest exists
    let tools_manifest_path = project_dir.join("manifest/tools.yml");
    if tools_manifest_path.exists() {
        log::info!("Tools manifest found, pulling tools...");
        match pull_tools_internal(project_dir, client).await {
            Ok(tool_count) => {
                log::info!("✓ Pulled {} tool(s)", tool_count);
            }
            Err(e) => {
                log::warn!("Failed to pull tools: {}", e);
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
        match push_spaces_internal(project_dir, client.clone()).await {
            Ok(space_count) => {
                log::info!("✓ Pushed {} space(s)", space_count);
            }
            Err(e) => {
                log::warn!("Failed to push spaces: {}", e);
            }
        }
    }

    // Also push workflows if manifest exists
    let workflows_manifest_path = project_dir.join("manifest/workflows.yml");
    if workflows_manifest_path.exists() {
        log::info!("Workflows manifest found, pushing workflows...");
        match push_workflows_internal(project_dir, client.clone()).await {
            Ok(workflow_count) => {
                log::info!("✓ Pushed {} workflow(s)", workflow_count);
            }
            Err(e) => {
                log::warn!("Failed to push workflows: {}", e);
            }
        }
    }

    // Also push agents if manifest exists
    let agents_manifest_path = project_dir.join("manifest/agents.yml");
    if agents_manifest_path.exists() {
        log::info!("Agents manifest found, pushing agents...");
        match push_agents_internal(project_dir, client.clone()).await {
            Ok(agent_count) => {
                log::info!("✓ Pushed {} agent(s)", agent_count);
            }
            Err(e) => {
                log::warn!("Failed to push agents: {}", e);
            }
        }
    }

    // Also push tools if manifest exists
    let tools_manifest_path = project_dir.join("manifest/tools.yml");
    if tools_manifest_path.exists() {
        log::info!("Tools manifest found, pushing tools...");
        match push_tools_internal(project_dir, client).await {
            Ok(tool_count) => {
                log::info!("✓ Pushed {} tool(s)", tool_count);
            }
            Err(e) => {
                log::warn!("Failed to push tools: {}", e);
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

    // Also bundle workflows if manifest exists
    let workflows_manifest_path = project_dir.join("manifest/workflows.yml");
    if workflows_manifest_path.exists() {
        log::info!("Workflows manifest found, bundling workflows...");
        // Get bundle directory from output_file path
        let bundle_dir = output_file
            .parent()
            .ok_or_else(|| eyre::eyre!("Could not determine bundle directory"))?;
        let workflows_output = bundle_dir.join("workflows.ndjson");
        match bundle_workflows_to_ndjson_internal(project_dir, &workflows_output).await {
            Ok(workflow_count) => {
                log::info!("✓ Bundled {} workflow(s)", workflow_count);
            }
            Err(e) => {
                log::warn!("Failed to bundle workflows: {}", e);
            }
        }
    }

    // Also bundle agents if manifest exists
    let agents_manifest_path = project_dir.join("manifest/agents.yml");
    if agents_manifest_path.exists() {
        log::info!("Agents manifest found, bundling agents...");
        // Get bundle directory from output_file path
        let bundle_dir = output_file
            .parent()
            .ok_or_else(|| eyre::eyre!("Could not determine bundle directory"))?;
        let agents_output = bundle_dir.join("agents.ndjson");
        match bundle_agents_to_ndjson_internal(project_dir, &agents_output).await {
            Ok(agent_count) => {
                log::info!("✓ Bundled {} agent(s)", agent_count);
            }
            Err(e) => {
                log::warn!("Failed to bundle agents: {}", e);
            }
        }
    }

    // Also bundle tools if manifest exists
    let tools_manifest_path = project_dir.join("manifest/tools.yml");
    if tools_manifest_path.exists() {
        log::info!("Tools manifest found, bundling tools...");
        // Get bundle directory from output_file path
        let bundle_dir = output_file
            .parent()
            .ok_or_else(|| eyre::eyre!("Could not determine bundle directory"))?;
        let tools_output = bundle_dir.join("tools.ndjson");
        match bundle_tools_to_ndjson_internal(project_dir, &tools_output).await {
            Ok(tool_count) => {
                log::info!("✓ Bundled {} tool(s)", tool_count);
            }
            Err(e) => {
                log::warn!("Failed to bundle tools: {}", e);
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

/// Add workflows to an existing manifest
///
/// Can add from Kibana search API or from a file (.json or .ndjson)
/// Optionally filter results by name using regex patterns (--include, --exclude)
pub async fn add_workflows_to_manifest(
    project_dir: impl AsRef<Path>,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    use crate::kibana::workflows::WorkflowEntry;

    let project_dir = project_dir.as_ref();

    // Load or create manifest
    log::info!("Loading workflows manifest from {}", project_dir.display());
    let manifest_path = project_dir.join("manifest/workflows.yml");
    let mut manifest = if manifest_path.exists() {
        WorkflowsManifest::read(&manifest_path)?
    } else {
        log::info!("No existing manifest found, will create new one");
        WorkflowsManifest::new()
    };
    log::info!("Current manifest has {} workflow(s)", manifest.count());

    // Fetch workflows from API or file
    let new_workflows: Vec<serde_json::Value> = if let Some(file) = file_path {
        // Read from file
        log::info!("Reading workflows from {}", file);
        let file_path = std::path::Path::new(&file);

        if !file_path.exists() {
            eyre::bail!("File not found: {}", file_path.display());
        }

        // Detect format by extension
        let extension = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match extension {
            "ndjson" => {
                // Parse NDJSON format (one workflow per line)
                use std::io::{BufRead, BufReader};
                let file = std::fs::File::open(file_path)?;
                let reader = BufReader::new(file);

                let mut workflows = Vec::new();
                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let workflow: serde_json::Value = serde_json::from_str(&line)?;
                    workflows.push(workflow);
                }
                log::info!("Read {} workflow(s) from NDJSON file", workflows.len());
                workflows
            }
            "json" => {
                // Parse JSON format (API response or array)
                let content = std::fs::read_to_string(file_path)?;
                let parsed: serde_json::Value = serde_json::from_str(&content)?;

                // Check if it's an API response with "results" field
                if let Some(results) = parsed.get("results").and_then(|v| v.as_array()) {
                    log::info!("Read {} workflow(s) from JSON API response", results.len());
                    results.iter().cloned().collect()
                } else if let Some(arr) = parsed.as_array() {
                    // Direct array of workflows
                    log::info!("Read {} workflow(s) from JSON array", arr.len());
                    arr.iter().cloned().collect()
                } else {
                    // Single workflow object
                    log::info!("Read 1 workflow from JSON file");
                    vec![parsed]
                }
            }
            _ => {
                eyre::bail!(
                    "Unsupported file format: {}. Expected .json or .ndjson",
                    extension
                );
            }
        }
    } else {
        // Search via API
        log::info!("Searching workflows via API...");
        let client = load_kibana_client()?;
        let extractor = WorkflowsExtractor::new(client, None);

        extractor.search_workflows(query.as_deref(), None).await?
    };

    log::info!("Found {} workflow(s) before filtering", new_workflows.len());

    // Apply regex filters: include first, then exclude
    let filtered_workflows: Vec<serde_json::Value> = {
        let mut workflows = new_workflows;

        // Apply include filter (if specified)
        if let Some(include_pattern) = &include {
            let regex = regex::Regex::new(include_pattern)
                .with_context(|| format!("Invalid include regex pattern: {}", include_pattern))?;

            workflows = workflows
                .into_iter()
                .filter(|w| {
                    w.get("name")
                        .and_then(|v| v.as_str())
                        .map(|name| regex.is_match(name))
                        .unwrap_or(false)
                })
                .collect();

            log::info!(
                "After include filter '{}': {} workflow(s)",
                include_pattern,
                workflows.len()
            );
        }

        // Apply exclude filter (if specified)
        if let Some(exclude_pattern) = &exclude {
            let regex = regex::Regex::new(exclude_pattern)
                .with_context(|| format!("Invalid exclude regex pattern: {}", exclude_pattern))?;

            workflows = workflows
                .into_iter()
                .filter(|w| {
                    w.get("name")
                        .and_then(|v| v.as_str())
                        .map(|name| !regex.is_match(name))
                        .unwrap_or(true)
                })
                .collect();

            log::info!(
                "After exclude filter '{}': {} workflow(s)",
                exclude_pattern,
                workflows.len()
            );
        }

        workflows
    };

    log::info!(
        "Adding {} workflow(s) after filtering",
        filtered_workflows.len()
    );

    // Add workflows to manifest and write files
    let workflows_dir = project_dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;

    let mut added_count = 0;
    for workflow in &filtered_workflows {
        // Extract id and name
        let workflow_id = workflow
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'id' field"))?;

        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'name' field"))?;

        // Add to manifest (will skip if already exists)
        if manifest.add_workflow(WorkflowEntry::new(workflow_id, workflow_name)) {
            log::debug!(
                "Added workflow to manifest: {} ({})",
                workflow_name,
                workflow_id
            );

            // Write workflow file
            let workflow_file = workflows_dir.join(format!("{}.json", workflow_name));
            let json = serde_json::to_string_pretty(workflow)?;
            std::fs::write(&workflow_file, json)?;

            log::debug!("Wrote workflow file: {}", workflow_file.display());
            added_count += 1;
        } else {
            log::debug!("Workflow already in manifest, skipping: {}", workflow_name);
        }
    }

    // Create manifest directory if it doesn't exist
    let manifest_dir = project_dir.join("manifest");
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!(
        "✓ Updated manifest now has {} workflow(s)",
        manifest.count()
    );
    log::info!("✓ Added {} new workflow(s)", added_count);

    Ok(added_count)
}

/// Add spaces to an existing manifest
///
/// Can add from Kibana search API or from a file (.json or .ndjson)
/// Optionally filter results by name using regex patterns (--include, --exclude)
pub async fn add_spaces_to_manifest(
    project_dir: impl AsRef<Path>,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    // Load or create manifest
    log::info!("Loading spaces manifest from {}", project_dir.display());
    let manifest_path = project_dir.join("manifest/spaces.yml");
    let mut manifest = if manifest_path.exists() {
        SpacesManifest::read(&manifest_path)?
    } else {
        log::info!("No existing manifest found, will create new one");
        SpacesManifest::new()
    };
    log::info!("Current manifest has {} space(s)", manifest.count());

    // Fetch spaces from API or file
    let new_spaces: Vec<serde_json::Value> = if let Some(file) = file_path {
        // Read from file
        log::info!("Reading spaces from {}", file);
        let file_path = std::path::Path::new(&file);

        if !file_path.exists() {
            eyre::bail!("File not found: {}", file_path.display());
        }

        // Detect format by extension
        let extension = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match extension {
            "ndjson" => {
                // Parse NDJSON format (one space per line)
                use std::io::{BufRead, BufReader};
                let file = std::fs::File::open(file_path)?;
                let reader = BufReader::new(file);

                let mut spaces = Vec::new();
                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let space: serde_json::Value = serde_json::from_str(&line)?;
                    spaces.push(space);
                }
                log::info!("Read {} space(s) from NDJSON file", spaces.len());
                spaces
            }
            "json" => {
                // Parse JSON format (array of spaces)
                let content = std::fs::read_to_string(file_path)?;
                let parsed: serde_json::Value = serde_json::from_str(&content)?;

                // Spaces API returns an array directly
                if let Some(arr) = parsed.as_array() {
                    log::info!("Read {} space(s) from JSON array", arr.len());
                    arr.iter().cloned().collect()
                } else {
                    // Single space object
                    log::info!("Read 1 space from JSON file");
                    vec![parsed]
                }
            }
            _ => {
                eyre::bail!(
                    "Unsupported file format: {}. Expected .json or .ndjson",
                    extension
                );
            }
        }
    } else {
        // Search via API (note: query parameter is ignored for spaces)
        if query.is_some() {
            log::warn!("Spaces API doesn't support query filtering - fetching all spaces");
        }
        log::info!("Fetching spaces via API...");
        let client = load_kibana_client()?;
        let extractor = SpacesExtractor::new(client, None);

        extractor.search_spaces(query.as_deref()).await?
    };

    log::info!("Found {} space(s) before filtering", new_spaces.len());

    // Apply regex filters: include first, then exclude
    let filtered_spaces: Vec<serde_json::Value> = {
        let mut spaces = new_spaces;

        // Apply include filter (if specified)
        if let Some(include_pattern) = &include {
            let regex = regex::Regex::new(include_pattern)
                .with_context(|| format!("Invalid include regex pattern: {}", include_pattern))?;

            spaces = spaces
                .into_iter()
                .filter(|s| {
                    s.get("name")
                        .and_then(|v| v.as_str())
                        .map(|name| regex.is_match(name))
                        .unwrap_or(false)
                })
                .collect();

            log::info!(
                "After include filter '{}': {} space(s)",
                include_pattern,
                spaces.len()
            );
        }

        // Apply exclude filter (if specified)
        if let Some(exclude_pattern) = &exclude {
            let regex = regex::Regex::new(exclude_pattern)
                .with_context(|| format!("Invalid exclude regex pattern: {}", exclude_pattern))?;

            spaces = spaces
                .into_iter()
                .filter(|s| {
                    s.get("name")
                        .and_then(|v| v.as_str())
                        .map(|name| !regex.is_match(name))
                        .unwrap_or(true)
                })
                .collect();

            log::info!(
                "After exclude filter '{}': {} space(s)",
                exclude_pattern,
                spaces.len()
            );
        }

        spaces
    };

    log::info!("Adding {} space(s) after filtering", filtered_spaces.len());

    // Add spaces to manifest and write files
    let spaces_dir = project_dir.join("spaces");
    std::fs::create_dir_all(&spaces_dir)?;

    let mut added_count = 0;
    for space in &filtered_spaces {
        // Extract id and name
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        let space_name = space
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'name' field"))?;

        // Add to manifest (will skip if already exists)
        if manifest.add_space(space_id.to_string(), space_name.to_string()) {
            log::debug!("Added space to manifest: {} ({})", space_id, space_name);

            // Write space file using name for filename
            let space_file = spaces_dir.join(format!("{}.json", space_name));
            let json = serde_json::to_string_pretty(space)?;
            std::fs::write(&space_file, json)?;

            log::debug!("Wrote space file: {}", space_file.display());
            added_count += 1;
        } else {
            log::debug!("Space already in manifest, skipping: {}", space_id);
        }
    }

    // Create manifest directory if it doesn't exist
    let manifest_dir = project_dir.join("manifest");
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!("✓ Updated manifest now has {} space(s)", manifest.count());
    log::info!("✓ Added {} new space(s)", added_count);

    Ok(added_count)
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
        // Get space ID (required)
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        // Use name if available, otherwise use id for filename
        let filename = space
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(space_id);

        let space_file = spaces_dir.join(format!("{}.json", filename));
        let json = serde_json::to_string_pretty(space)?;
        std::fs::write(&space_file, json)?;

        log::debug!("Wrote space: {}", space_file.display());
        count += 1;
    }

    Ok(count)
}

/// Pull spaces from Kibana to local directory
///
/// Pipeline: SpacesExtractor → Write to spaces/<name or id>.json files
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
        // Get space ID (required)
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        // Use name if available, otherwise use id for filename
        let filename = space
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(space_id);

        let space_file = spaces_dir.join(format!("{}.json", filename));
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
/// Pipeline: Read from spaces/<name or id>.json → SpacesLoader
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
/// Pipeline: Read from spaces/<name or id>.json → SpacesLoader
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

/// Load workflows manifest from project directory
fn load_workflows_manifest(project_dir: impl AsRef<Path>) -> Result<WorkflowsManifest> {
    let manifest_path = project_dir.as_ref().join("manifest/workflows.yml");

    if !manifest_path.exists() {
        eyre::bail!("Workflows manifest not found: {}", manifest_path.display());
    }

    WorkflowsManifest::read(&manifest_path)
}

/// Pull workflows from Kibana to local directory (internal)
///
/// Pipeline: WorkflowsExtractor → Write to workflows/<name>.json files
async fn pull_workflows_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading workflows manifest from {}", project_dir.display());
    let manifest = load_workflows_manifest(project_dir)?;
    log::debug!("Manifest loaded: {} workflow(s)", manifest.count());

    // Build the extract pipeline
    log::debug!("Extracting workflows from Kibana...");
    let extractor = WorkflowsExtractor::new(client, Some(manifest));

    // Extract workflows
    let workflows = extractor.extract().await?;

    // Write each workflow to its own JSON file
    let workflows_dir = project_dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;

    let mut count = 0;
    for workflow in &workflows {
        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'name' field"))?;

        let workflow_file = workflows_dir.join(format!("{}.json", workflow_name));
        let json = serde_json::to_string_pretty(workflow)?;
        std::fs::write(&workflow_file, json)?;

        log::debug!("Wrote workflow: {}", workflow_file.display());
        count += 1;
    }

    log::debug!(
        "✓ Pulled {} workflow(s) to {}",
        count,
        workflows_dir.display()
    );

    Ok(count)
}

/// Pull workflows from Kibana to local directory
///
/// Pipeline: WorkflowsExtractor → Write to workflows/<name>.json files
pub async fn pull_workflows(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading workflows manifest from {}", project_dir.display());
    let manifest = load_workflows_manifest(project_dir)?;
    log::info!("Manifest loaded: {} workflow(s)", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    // Build the extract pipeline
    log::info!("Extracting workflows from Kibana...");
    let extractor = WorkflowsExtractor::new(client, Some(manifest));

    // Extract workflows
    let workflows = extractor.extract().await?;

    // Write each workflow to its own JSON file
    let workflows_dir = project_dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;

    let mut count = 0;
    for workflow in &workflows {
        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'name' field"))?;

        let workflow_file = workflows_dir.join(format!("{}.json", workflow_name));
        let json = serde_json::to_string_pretty(workflow)?;
        std::fs::write(&workflow_file, json)?;

        log::debug!("Wrote workflow: {}", workflow_file.display());
        count += 1;
    }

    log::info!(
        "✓ Pulled {} workflow(s) to {}",
        count,
        workflows_dir.display()
    );

    Ok(count)
}

/// Push workflows from local directory to Kibana (internal)
///
/// Pipeline: Read from workflows/<name>.json → WorkflowsLoader
async fn push_workflows_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading workflows from {}", project_dir.display());
    let workflows_dir = project_dir.join("workflows");

    if !workflows_dir.exists() {
        eyre::bail!("Workflows directory not found: {}", workflows_dir.display());
    }

    // Read all JSON files from workflows directory
    let mut workflows = Vec::new();
    for entry in std::fs::read_dir(&workflows_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let workflow: serde_json::Value = serde_json::from_str(&content)?;
            workflows.push(workflow);
        }
    }

    log::debug!("Read {} workflow(s) from disk", workflows.len());

    let loader = WorkflowsLoader::new(client);

    // Load workflows to Kibana
    let count = loader.load(workflows).await?;

    Ok(count)
}

/// Push workflows from local directory to Kibana
///
/// Pipeline: Read from workflows/<name>.json → WorkflowsLoader
pub async fn push_workflows(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading workflows from {}", project_dir.display());
    let workflows_dir = project_dir.join("workflows");

    if !workflows_dir.exists() {
        eyre::bail!("Workflows directory not found: {}", workflows_dir.display());
    }

    // Read all JSON files from workflows directory
    let mut workflows = Vec::new();
    for entry in std::fs::read_dir(&workflows_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let workflow: serde_json::Value = serde_json::from_str(&content)?;
            workflows.push(workflow);
        }
    }

    log::info!("Read {} workflow(s) from disk", workflows.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    let loader = WorkflowsLoader::new(client);

    // Load workflows to Kibana
    let count = loader.load(workflows).await?;

    log::info!("✓ Pushed {} workflow(s) to Kibana", count);

    Ok(count)
}

/// Bundle workflows to NDJSON file for distribution (internal)
///
/// Pipeline: Read from workflows/<name>.json → Write to workflows.ndjson
async fn bundle_workflows_to_ndjson_internal(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::debug!("Loading workflows from {}", project_dir.display());
    let workflows_dir = project_dir.join("workflows");

    if !workflows_dir.exists() {
        eyre::bail!("Workflows directory not found: {}", workflows_dir.display());
    }

    // Read all JSON files from workflows directory
    let mut workflows = Vec::new();
    for entry in std::fs::read_dir(&workflows_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let workflow: serde_json::Value = serde_json::from_str(&content)?;
            workflows.push(workflow);
        }
    }

    log::debug!("Read {} workflow(s) from disk", workflows.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for workflow in &workflows {
        let json_line = serde_json::to_string(workflow)?;
        writeln!(file, "{}", json_line)?;
    }

    Ok(workflows.len())
}

/// Bundle workflows to NDJSON file for distribution
///
/// Pipeline: Read from workflows/<name>.json → Write to workflows.ndjson
pub async fn bundle_workflows_to_ndjson(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::info!("Loading workflows from {}", project_dir.display());
    let workflows_dir = project_dir.join("workflows");

    if !workflows_dir.exists() {
        eyre::bail!("Workflows directory not found: {}", workflows_dir.display());
    }

    // Read all JSON files from workflows directory
    let mut workflows = Vec::new();
    for entry in std::fs::read_dir(&workflows_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let workflow: serde_json::Value = serde_json::from_str(&content)?;
            workflows.push(workflow);
        }
    }

    log::info!("Read {} workflow(s) from disk", workflows.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for workflow in &workflows {
        let json_line = serde_json::to_string(workflow)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} workflow(s) to {}",
        workflows.len(),
        output_file.display()
    );

    Ok(workflows.len())
}

// ==============================================================================
// Agents API Functions
// ==============================================================================

/// Load agents manifest from project directory
///
/// Expects manifest at `manifest/agents.yml`
fn load_agents_manifest(project_dir: impl AsRef<Path>) -> Result<AgentsManifest> {
    let project_dir = project_dir.as_ref();
    let manifest_path = project_dir.join("manifest/agents.yml");

    if !manifest_path.exists() {
        eyre::bail!(
            "Agents manifest not found: {}. Run 'kibob add agents' to create it.",
            manifest_path.display()
        );
    }

    AgentsManifest::read(&manifest_path)
}

/// Add agents to an existing manifest
///
/// Can add from Kibana search API or from a file (.json or .ndjson)
/// Optionally filter results by name using regex patterns (--include, --exclude)
pub async fn add_agents_to_manifest(
    project_dir: impl AsRef<Path>,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    use crate::kibana::agents::AgentEntry;

    let project_dir = project_dir.as_ref();

    // Load or create manifest
    log::info!("Loading agents manifest from {}", project_dir.display());
    let manifest_path = project_dir.join("manifest/agents.yml");
    let mut manifest = if manifest_path.exists() {
        AgentsManifest::read(&manifest_path)?
    } else {
        log::info!("No existing manifest found, will create new one");
        AgentsManifest::new()
    };
    log::info!("Current manifest has {} agent(s)", manifest.count());

    // Fetch agents from API or file
    let new_agents: Vec<serde_json::Value> = if let Some(file) = file_path {
        // Read from file
        log::info!("Reading agents from {}", file);
        let file_path = std::path::Path::new(&file);

        if !file_path.exists() {
            eyre::bail!("File not found: {}", file_path.display());
        }

        // Detect format by extension
        let extension = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match extension {
            "ndjson" => {
                // Parse NDJSON format (one agent per line)
                use std::io::{BufRead, BufReader};
                let file = std::fs::File::open(file_path)?;
                let reader = BufReader::new(file);

                let mut agents = Vec::new();
                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let agent: serde_json::Value = serde_json::from_str(&line)?;
                    agents.push(agent);
                }
                log::info!("Read {} agent(s) from NDJSON file", agents.len());
                agents
            }
            "json" => {
                // Parse JSON format (array or single object)
                let content = std::fs::read_to_string(file_path)?;
                let parsed: serde_json::Value = serde_json::from_str(&content)?;

                // Check if it's an array
                if let Some(arr) = parsed.as_array() {
                    log::info!("Read {} agent(s) from JSON array", arr.len());
                    arr.iter().cloned().collect()
                } else {
                    // Single agent object
                    log::info!("Read 1 agent from JSON file");
                    vec![parsed]
                }
            }
            _ => {
                eyre::bail!(
                    "Unsupported file format: {}. Expected .json or .ndjson",
                    extension
                );
            }
        }
    } else {
        // Search via API
        log::info!("Searching agents via API...");
        let client = load_kibana_client()?;
        let extractor = AgentsExtractor::new(client, None);

        extractor.search_agents(query.as_deref()).await?
    };

    log::info!("Found {} agent(s) before filtering", new_agents.len());

    // Apply regex filters: include first, then exclude
    let filtered_agents: Vec<serde_json::Value> = {
        let mut agents = new_agents;

        // Apply include filter (if specified)
        if let Some(include_pattern) = &include {
            let regex = regex::Regex::new(include_pattern)
                .with_context(|| format!("Invalid include regex pattern: {}", include_pattern))?;

            agents = agents
                .into_iter()
                .filter(|a| {
                    a.get("name")
                        .and_then(|v| v.as_str())
                        .map(|name| regex.is_match(name))
                        .unwrap_or(false)
                })
                .collect();

            log::info!(
                "After include filter '{}': {} agent(s)",
                include_pattern,
                agents.len()
            );
        }

        // Apply exclude filter (if specified)
        if let Some(exclude_pattern) = &exclude {
            let regex = regex::Regex::new(exclude_pattern)
                .with_context(|| format!("Invalid exclude regex pattern: {}", exclude_pattern))?;

            agents = agents
                .into_iter()
                .filter(|a| {
                    a.get("name")
                        .and_then(|v| v.as_str())
                        .map(|name| !regex.is_match(name))
                        .unwrap_or(true)
                })
                .collect();

            log::info!(
                "After exclude filter '{}': {} agent(s)",
                exclude_pattern,
                agents.len()
            );
        }

        agents
    };

    log::info!("Adding {} agent(s) after filtering", filtered_agents.len());

    // Add agents to manifest and write files
    let agents_dir = project_dir.join("agents");
    std::fs::create_dir_all(&agents_dir)?;

    let mut added_count = 0;
    for agent in &filtered_agents {
        // Extract id and name
        let agent_id = agent
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Agent missing 'id' field"))?;

        let agent_name = agent
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Agent missing 'name' field"))?;

        // Add to manifest (will skip if already exists)
        if manifest.add_agent(AgentEntry::new(agent_id, agent_name)) {
            log::debug!("Added agent to manifest: {} ({})", agent_name, agent_id);

            // Write agent file
            let agent_file = agents_dir.join(format!("{}.json", agent_name));
            let json = serde_json::to_string_pretty(agent)?;
            std::fs::write(&agent_file, json)?;

            log::debug!("Wrote agent file: {}", agent_file.display());
            added_count += 1;
        } else {
            log::debug!("Agent already in manifest, skipping: {}", agent_name);
        }
    }

    // Create manifest directory if it doesn't exist
    let manifest_dir = project_dir.join("manifest");
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!("✓ Updated manifest now has {} agent(s)", manifest.count());
    log::info!("✓ Added {} new agent(s)", added_count);

    Ok(added_count)
}

/// Pull agents from Kibana to local directory (internal)
///
/// Pipeline: AgentsExtractor → Write to agents/<name>.json files
async fn pull_agents_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading agents manifest from {}", project_dir.display());
    let manifest = load_agents_manifest(project_dir)?;
    log::debug!("Manifest loaded: {} agent(s)", manifest.count());

    // Build the extract pipeline
    log::debug!("Extracting agents from Kibana...");
    let extractor = AgentsExtractor::new(client, Some(manifest));

    // Extract agents
    let agents = extractor.extract().await?;

    // Write each agent to its own JSON file
    let agents_dir = project_dir.join("agents");
    std::fs::create_dir_all(&agents_dir)?;

    let mut count = 0;
    for agent in &agents {
        let agent_name = agent
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Agent missing 'name' field"))?;

        let agent_file = agents_dir.join(format!("{}.json", agent_name));
        let json = serde_json::to_string_pretty(agent)?;
        std::fs::write(&agent_file, json)?;

        log::debug!("Wrote agent: {}", agent_file.display());
        count += 1;
    }

    Ok(count)
}

/// Pull agents from Kibana to local directory
///
/// Pipeline: AgentsExtractor → Write to agents/<name>.json files
pub async fn pull_agents(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading agents manifest from {}", project_dir.display());
    let manifest = load_agents_manifest(project_dir)?;
    log::info!("Manifest loaded: {} agent(s)", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    // Build the extract pipeline
    log::info!("Extracting agents from Kibana...");
    let extractor = AgentsExtractor::new(client, Some(manifest));

    // Extract agents
    let agents = extractor.extract().await?;

    // Write each agent to its own JSON file
    let agents_dir = project_dir.join("agents");
    std::fs::create_dir_all(&agents_dir)?;

    let mut count = 0;
    for agent in &agents {
        let agent_name = agent
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Agent missing 'name' field"))?;

        let agent_file = agents_dir.join(format!("{}.json", agent_name));
        let json = serde_json::to_string_pretty(agent)?;
        std::fs::write(&agent_file, json)?;

        log::debug!("Wrote agent: {}", agent_file.display());
        count += 1;
    }

    log::info!("✓ Pulled {} agent(s) to {}", count, agents_dir.display());

    Ok(count)
}

/// Push agents from local directory to Kibana (internal)
///
/// Pipeline: Read from agents/<name>.json → AgentsLoader
async fn push_agents_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading agents from {}", project_dir.display());
    let agents_dir = project_dir.join("agents");

    if !agents_dir.exists() {
        eyre::bail!("Agents directory not found: {}", agents_dir.display());
    }

    // Read all JSON files from agents directory
    let mut agents = Vec::new();
    for entry in std::fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let agent: serde_json::Value = serde_json::from_str(&content)?;
            agents.push(agent);
        }
    }

    log::debug!("Read {} agent(s) from disk", agents.len());

    let loader = AgentsLoader::new(client);

    // Load agents to Kibana
    let count = loader.load(agents).await?;

    Ok(count)
}

/// Push agents from local directory to Kibana
///
/// Pipeline: Read from agents/<name>.json → AgentsLoader
pub async fn push_agents(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading agents from {}", project_dir.display());
    let agents_dir = project_dir.join("agents");

    if !agents_dir.exists() {
        eyre::bail!("Agents directory not found: {}", agents_dir.display());
    }

    // Read all JSON files from agents directory
    let mut agents = Vec::new();
    for entry in std::fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let agent: serde_json::Value = serde_json::from_str(&content)?;
            agents.push(agent);
        }
    }

    log::info!("Read {} agent(s) from disk", agents.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    let loader = AgentsLoader::new(client);

    // Load agents to Kibana
    let count = loader.load(agents).await?;

    log::info!("✓ Pushed {} agent(s) to Kibana", count);

    Ok(count)
}

/// Bundle agents to NDJSON file for distribution (internal)
///
/// Pipeline: Read from agents/<name>.json → Write to agents.ndjson
async fn bundle_agents_to_ndjson_internal(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::debug!("Loading agents from {}", project_dir.display());
    let agents_dir = project_dir.join("agents");

    if !agents_dir.exists() {
        eyre::bail!("Agents directory not found: {}", agents_dir.display());
    }

    // Read all JSON files from agents directory
    let mut agents = Vec::new();
    for entry in std::fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let agent: serde_json::Value = serde_json::from_str(&content)?;
            agents.push(agent);
        }
    }

    log::debug!("Read {} agent(s) from disk", agents.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for agent in &agents {
        let json_line = serde_json::to_string(agent)?;
        writeln!(file, "{}", json_line)?;
    }

    Ok(agents.len())
}

/// Bundle agents to NDJSON file for distribution
///
/// Pipeline: Read from agents/<name>.json → Write to agents.ndjson
pub async fn bundle_agents_to_ndjson(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::info!("Loading agents from {}", project_dir.display());
    let agents_dir = project_dir.join("agents");

    if !agents_dir.exists() {
        eyre::bail!("Agents directory not found: {}", agents_dir.display());
    }

    // Read all JSON files from agents directory
    let mut agents = Vec::new();
    for entry in std::fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let agent: serde_json::Value = serde_json::from_str(&content)?;
            agents.push(agent);
        }
    }

    log::info!("Read {} agent(s) from disk", agents.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for agent in &agents {
        let json_line = serde_json::to_string(agent)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} agent(s) to {}",
        agents.len(),
        output_file.display()
    );

    Ok(agents.len())
}

// ==============================================================================
// Tools API Functions
// ==============================================================================

/// Load tools manifest from project directory
///
/// Expects manifest at `manifest/tools.yml`
fn load_tools_manifest(project_dir: impl AsRef<Path>) -> Result<ToolsManifest> {
    let project_dir = project_dir.as_ref();
    let manifest_path = project_dir.join("manifest/tools.yml");

    if !manifest_path.exists() {
        eyre::bail!(
            "Tools manifest not found: {}. Run 'kibob add tools' to create it.",
            manifest_path.display()
        );
    }

    ToolsManifest::read(&manifest_path)
}

/// Add tools to an existing manifest
///
/// Can add from Kibana search API or from a file (.json or .ndjson)
/// Optionally filter results by name using regex patterns (--include, --exclude)
pub async fn add_tools_to_manifest(
    project_dir: impl AsRef<Path>,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    // Load or create manifest
    log::info!("Loading tools manifest from {}", project_dir.display());
    let manifest_path = project_dir.join("manifest/tools.yml");
    let mut manifest = if manifest_path.exists() {
        ToolsManifest::read(&manifest_path)?
    } else {
        log::info!("No existing manifest found, will create new one");
        ToolsManifest::new()
    };
    log::info!("Current manifest has {} tool(s)", manifest.count());

    // Fetch tools from API or file
    let new_tools: Vec<serde_json::Value> = if let Some(file) = file_path {
        // Read from file
        log::info!("Reading tools from {}", file);
        let file_path = std::path::Path::new(&file);

        if !file_path.exists() {
            eyre::bail!("File not found: {}", file_path.display());
        }

        // Detect format by extension
        let extension = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match extension {
            "ndjson" => {
                // Parse NDJSON format (one tool per line)
                use std::io::{BufRead, BufReader};
                let file = std::fs::File::open(file_path)?;
                let reader = BufReader::new(file);

                let mut tools = Vec::new();
                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let tool: serde_json::Value = serde_json::from_str(&line)?;
                    tools.push(tool);
                }
                log::info!("Read {} tool(s) from NDJSON file", tools.len());
                tools
            }
            "json" => {
                // Parse JSON format (array or single object)
                let content = std::fs::read_to_string(file_path)?;
                let parsed: serde_json::Value = serde_json::from_str(&content)?;

                // Check if it's an array
                if let Some(arr) = parsed.as_array() {
                    log::info!("Read {} tool(s) from JSON array", arr.len());
                    arr.iter().cloned().collect()
                } else {
                    // Single tool object
                    log::info!("Read 1 tool from JSON file");
                    vec![parsed]
                }
            }
            _ => {
                eyre::bail!(
                    "Unsupported file format: {}. Expected .json or .ndjson",
                    extension
                );
            }
        }
    } else {
        // Search via API
        log::info!("Searching tools via API...");
        let client = load_kibana_client()?;
        let extractor = ToolsExtractor::new(client, None);

        extractor.search_tools(query.as_deref()).await?
    };

    log::info!("Found {} tool(s) before filtering", new_tools.len());

    // Apply regex filters: include first, then exclude
    // Filter by id since tools don't have name field
    let filtered_tools: Vec<serde_json::Value> = {
        let mut tools = new_tools;

        // Apply include filter (if specified) - filter by id or name if available
        if let Some(include_pattern) = &include {
            let regex = regex::Regex::new(include_pattern)
                .with_context(|| format!("Invalid include regex pattern: {}", include_pattern))?;

            tools = tools
                .into_iter()
                .filter(|t| {
                    // Try name first, fallback to id
                    let filter_field = t
                        .get("name")
                        .or_else(|| t.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    regex.is_match(filter_field)
                })
                .collect();

            log::info!(
                "After include filter '{}': {} tool(s)",
                include_pattern,
                tools.len()
            );
        }

        // Apply exclude filter (if specified)
        if let Some(exclude_pattern) = &exclude {
            let regex = regex::Regex::new(exclude_pattern)
                .with_context(|| format!("Invalid exclude regex pattern: {}", exclude_pattern))?;

            tools = tools
                .into_iter()
                .filter(|t| {
                    // Try name first, fallback to id
                    let filter_field = t
                        .get("name")
                        .or_else(|| t.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    !regex.is_match(filter_field)
                })
                .collect();

            log::info!(
                "After exclude filter '{}': {} tool(s)",
                exclude_pattern,
                tools.len()
            );
        }

        tools
    };

    log::info!("Adding {} tool(s) after filtering", filtered_tools.len());

    // Add tools to manifest and write files
    let tools_dir = project_dir.join("tools");
    std::fs::create_dir_all(&tools_dir)?;

    let mut added_count = 0;
    for tool in &filtered_tools {
        // Extract id (required)
        let tool_id = tool
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Tool missing 'id' field"))?;

        // Use name for filename if available, otherwise use id
        let filename = tool.get("name").and_then(|v| v.as_str()).unwrap_or(tool_id);

        // Add to manifest (will skip if already exists)
        if manifest.add_tool(tool_id.to_string()) {
            log::debug!("Added tool to manifest: {}", tool_id);

            // Write tool file - use name if available, otherwise id
            let tool_file = tools_dir.join(format!("{}.json", filename));
            let json = serde_json::to_string_pretty(tool)?;
            std::fs::write(&tool_file, json)?;

            log::debug!("Wrote tool file: {}", tool_file.display());
            added_count += 1;
        } else {
            log::debug!("Tool already in manifest, skipping: {}", tool_id);
        }
    }

    // Create manifest directory if it doesn't exist
    let manifest_dir = project_dir.join("manifest");
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!("✓ Updated manifest now has {} tool(s)", manifest.count());
    log::info!("✓ Added {} new tool(s)", added_count);

    Ok(added_count)
}

/// Pull tools from Kibana to local directory (internal)
///
/// Pipeline: ToolsExtractor → Write to tools/<name or id>.json files
async fn pull_tools_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading tools manifest from {}", project_dir.display());
    let manifest = load_tools_manifest(project_dir)?;
    log::debug!("Manifest loaded: {} tool(s)", manifest.count());

    // Build the extract pipeline
    log::debug!("Extracting tools from Kibana...");
    let extractor = ToolsExtractor::new(client, Some(manifest));

    // Extract tools
    let tools = extractor.extract().await?;

    // Write each tool to its own JSON file
    let tools_dir = project_dir.join("tools");
    std::fs::create_dir_all(&tools_dir)?;

    let mut count = 0;
    for tool in &tools {
        // Get tool ID (required)
        let tool_id = tool
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Tool missing 'id' field"))?;

        // Use name if available, otherwise use id for filename
        let filename = tool.get("name").and_then(|v| v.as_str()).unwrap_or(tool_id);

        let tool_file = tools_dir.join(format!("{}.json", filename));
        let json = serde_json::to_string_pretty(tool)?;
        std::fs::write(&tool_file, json)?;

        log::debug!("Wrote tool: {}", tool_file.display());
        count += 1;
    }

    Ok(count)
}

/// Pull tools from Kibana to local directory
///
/// Pipeline: ToolsExtractor → Write to tools/<name or id>.json files
pub async fn pull_tools(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading tools manifest from {}", project_dir.display());
    let manifest = load_tools_manifest(project_dir)?;
    log::info!("Manifest loaded: {} tool(s)", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    // Build the extract pipeline
    log::info!("Extracting tools from Kibana...");
    let extractor = ToolsExtractor::new(client, Some(manifest));

    // Extract tools
    let tools = extractor.extract().await?;

    // Write each tool to its own JSON file
    let tools_dir = project_dir.join("tools");
    std::fs::create_dir_all(&tools_dir)?;

    let mut count = 0;
    for tool in &tools {
        // Get tool ID (required)
        let tool_id = tool
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Tool missing 'id' field"))?;

        // Use name if available, otherwise use id for filename
        let filename = tool.get("name").and_then(|v| v.as_str()).unwrap_or(tool_id);

        let tool_file = tools_dir.join(format!("{}.json", filename));
        let json = serde_json::to_string_pretty(tool)?;
        std::fs::write(&tool_file, json)?;

        log::debug!("Wrote tool: {}", tool_file.display());
        count += 1;
    }

    log::info!("✓ Pulled {} tool(s) to {}", count, tools_dir.display());

    Ok(count)
}

/// Push tools from local directory to Kibana (internal)
///
/// Pipeline: Read from tools/<name or id>.json → ToolsLoader
async fn push_tools_internal(project_dir: impl AsRef<Path>, client: Kibana) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading tools from {}", project_dir.display());
    let tools_dir = project_dir.join("tools");

    if !tools_dir.exists() {
        eyre::bail!("Tools directory not found: {}", tools_dir.display());
    }

    // Read all JSON files from tools directory
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let tool: serde_json::Value = serde_json::from_str(&content)?;
            tools.push(tool);
        }
    }

    log::debug!("Read {} tool(s) from disk", tools.len());

    let loader = ToolsLoader::new(client);

    // Load tools to Kibana
    let count = loader.load(tools).await?;

    Ok(count)
}

/// Push tools from local directory to Kibana
///
/// Pipeline: Read from tools/<name or id>.json → ToolsLoader
pub async fn push_tools(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading tools from {}", project_dir.display());
    let tools_dir = project_dir.join("tools");

    if !tools_dir.exists() {
        eyre::bail!("Tools directory not found: {}", tools_dir.display());
    }

    // Read all JSON files from tools directory
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let tool: serde_json::Value = serde_json::from_str(&content)?;
            tools.push(tool);
        }
    }

    log::info!("Read {} tool(s) from disk", tools.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client()?;

    let loader = ToolsLoader::new(client);

    // Load tools to Kibana
    let count = loader.load(tools).await?;

    log::info!("✓ Pushed {} tool(s) to Kibana", count);

    Ok(count)
}

/// Bundle tools to NDJSON file for distribution (internal)
///
/// Pipeline: Read from tools/<name or id>.json → Write to tools.ndjson
async fn bundle_tools_to_ndjson_internal(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::debug!("Loading tools from {}", project_dir.display());
    let tools_dir = project_dir.join("tools");

    if !tools_dir.exists() {
        eyre::bail!("Tools directory not found: {}", tools_dir.display());
    }

    // Read all JSON files from tools directory
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let tool: serde_json::Value = serde_json::from_str(&content)?;
            tools.push(tool);
        }
    }

    log::debug!("Read {} tool(s) from disk", tools.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for tool in &tools {
        let json_line = serde_json::to_string(tool)?;
        writeln!(file, "{}", json_line)?;
    }

    Ok(tools.len())
}

/// Bundle tools to NDJSON file for distribution
///
/// Pipeline: Read from tools/<name or id>.json → Write to tools.ndjson
pub async fn bundle_tools_to_ndjson(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::info!("Loading tools from {}", project_dir.display());
    let tools_dir = project_dir.join("tools");

    if !tools_dir.exists() {
        eyre::bail!("Tools directory not found: {}", tools_dir.display());
    }

    // Read all JSON files from tools directory
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            let tool: serde_json::Value = serde_json::from_str(&content)?;
            tools.push(tool);
        }
    }

    log::info!("Read {} tool(s) from disk", tools.len());

    // Write to NDJSON file
    use std::io::Write;
    let mut file = std::fs::File::create(output_file)?;
    for tool in &tools {
        let json_line = serde_json::to_string(tool)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} tool(s) to {}",
        tools.len(),
        output_file.display()
    );

    Ok(tools.len())
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
