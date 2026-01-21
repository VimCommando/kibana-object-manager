//! CLI helper functions

use crate::{
    client::{Auth, KibanaClient},
    etl::{Extractor, Loader, Transformer},
    kibana::agents::{AgentsExtractor, AgentsLoader, AgentsManifest},
    kibana::saved_objects::{SavedObjectsExtractor, SavedObjectsLoader},
    kibana::spaces::{SpacesExtractor, SpacesLoader, SpacesManifest},
    kibana::tools::{ToolsExtractor, ToolsLoader, ToolsManifest},
    kibana::workflows::{WorkflowsExtractor, WorkflowsLoader, WorkflowsManifest},
    migration::load_saved_objects_manifest,
    storage::{self, DirectoryReader, DirectoryWriter},
    transform::{
        FieldDropper, FieldEscaper, FieldUnescaper, ManagedFlagAdder, MultilineFieldFormatter,
        VegaSpecEscaper, VegaSpecUnescaper,
    },
};
use eyre::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;
use url::Url;

/// Load Kibana client from environment variables
///
/// Expected environment variables:
/// - KIBANA_URL: Kibana base URL (required)
/// - KIBANA_USERNAME: Username for basic auth (optional)
/// - KIBANA_PASSWORD: Password for basic auth (optional)
/// - KIBANA_APIKEY: API key for auth (optional, conflicts with username/password)
///
/// # Arguments
/// * `project_dir` - Project directory path containing spaces.yml
pub fn load_kibana_client(project_dir: impl AsRef<Path>) -> Result<KibanaClient> {
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

    KibanaClient::try_new(url, auth, project_dir).context("Failed to create Kibana client")
}

/// Check if an API should be processed based on the filter
fn should_process_api(api_name: &str, filter: Option<&[String]>) -> bool {
    match filter {
        None => true, // No filter means process everything
        Some(apis) => apis.iter().any(|api| {
            let api = api.to_lowercase();
            match api_name {
                "saved_objects" => {
                    api == "saved_objects"
                        || api == "objects"
                        || api == "object"
                        || api == "saved_object"
                }
                "workflows" => api == "workflows" || api == "workflow",
                "agents" => api == "agents" || api == "agent",
                "tools" => api == "tools" || api == "tool",
                "spaces" => api == "spaces" || api == "space",
                _ => false,
            }
        }),
    }
}

/// Pull saved objects from Kibana to local directory
///
/// Pipeline: SavedObjectsExtractor → FieldDropper → FieldUnescaper → DirectoryWriter
/// Also pulls spaces if spaces.yml exists
///
/// # Arguments
/// * `project_dir` - Project directory path
/// * `space_filter` - Optional list of space IDs to pull (e.g., ["default", "marketing"])
/// * `api_filter` - Optional list of APIs to pull (e.g., ["tools", "agents"])
pub async fn pull_saved_objects(
    project_dir: impl AsRef<Path>,
    space_filter: Option<&[String]>,
    api_filter: Option<&[String]>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;

    // Pull space definitions FIRST (before any other operations)
    // This ensures space definitions are up-to-date before pulling resources
    if should_process_api("spaces", api_filter) {
        let spaces_manifest_path = project_dir.join("spaces.yml");
        if spaces_manifest_path.exists() {
            log::info!("Pulling space definitions...");
            match pull_spaces_internal(project_dir, &client).await {
                Ok(space_count) => {
                    log::info!("✓ Pulled {} space definition(s)", space_count);
                }
                Err(e) => {
                    log::warn!("Failed to pull space definitions: {}", e);
                }
            }
        }
    } else {
        log::debug!("Skipping spaces pull (filtered out)");
    }

    // Determine which spaces to operate on
    let target_space_ids = get_target_space_ids(&client, space_filter);

    let mut total_count = 0;

    // Pull each managed space
    for space_id in &target_space_ids {
        log::info!("Processing space: {}", space_id.cyan());

        // Get space client for this space
        let space_client = client.space(space_id)?;

        // Pull saved objects for this space
        if should_process_api("saved_objects", api_filter) {
            if let Ok(count) = pull_space_saved_objects(project_dir, &space_client).await {
                total_count += count;
            }
        }

        // Pull workflows for this space
        if should_process_api("workflows", api_filter) {
            if let Ok(count) = pull_space_workflows(project_dir, &space_client).await {
                log::debug!("Pulled {} workflow(s) for space {}", count, space_id.cyan());
            }
        }

        // Pull agents for this space
        if should_process_api("agents", api_filter) {
            if let Ok(count) = pull_space_agents(project_dir, &space_client).await {
                log::debug!("Pulled {} agent(s) for space {}", count, space_id.cyan());
            }
        }

        // Pull tools for this space
        if should_process_api("tools", api_filter) {
            if let Ok(count) = pull_space_tools(project_dir, &space_client).await {
                log::debug!("Pulled {} tool(s) for space {}", count, space_id.cyan());
            }
        }
    }

    log::info!(
        "✓ Pull complete: {} total saved object(s) across all spaces",
        total_count
    );
    Ok(total_count)
}

/// Push saved objects from local directory to Kibana
///
/// Pipeline: DirectoryReader → VegaSpecEscaper → FieldEscaper → ManagedFlagAdder → SavedObjectsLoader
/// Also pushes spaces if spaces.yml exists
///
/// # Arguments
/// * `project_dir` - Project directory path
/// * `managed` - Whether to mark objects as managed (read-only in Kibana UI)
/// * `space_filter` - Optional list of space IDs to push (e.g., ["default", "marketing"])
/// * `api_filter` - Optional list of APIs to push (e.g., ["tools", "agents"])
pub async fn push_saved_objects(
    project_dir: impl AsRef<Path>,
    managed: bool,
    space_filter: Option<&[String]>,
    api_filter: Option<&[String]>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;

    // Push space definitions FIRST (before any other operations)
    // This ensures spaces exist in Kibana before pushing resources to them
    if should_process_api("spaces", api_filter) {
        let spaces_manifest_path = project_dir.join("spaces.yml");
        if spaces_manifest_path.exists() {
            log::info!("Pushing space definitions...");
            match push_spaces_internal(project_dir, &client).await {
                Ok(space_count) => {
                    log::info!("✓ Pushed {} space definition(s)", space_count);
                }
                Err(e) => {
                    log::warn!("Failed to push space definitions: {}", e);
                }
            }
        }
    } else {
        log::debug!("Skipping spaces push (filtered out)");
    }

    // Determine which spaces to operate on
    let target_space_ids = get_target_space_ids(&client, space_filter);

    let mut total_saved_objects = 0;
    let mut total_workflows = 0;
    let mut total_agents = 0;
    let mut total_tools = 0;

    // Push each managed space
    for space_id in &target_space_ids {
        log::info!("Processing space: {}", space_id.cyan());

        // Get space client for this space
        let space_client = client.space(space_id)?;

        // Push saved objects for this space
        if should_process_api("saved_objects", api_filter) {
            match push_space_saved_objects(project_dir, &space_client, managed).await {
                Ok(count) => {
                    total_saved_objects += count;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to push saved objects for space {}: {}",
                        space_id.cyan(),
                        e
                    );
                }
            }
        }

        // Push workflows for this space
        if should_process_api("workflows", api_filter) {
            match push_space_workflows(project_dir, &space_client).await {
                Ok(count) => {
                    total_workflows += count;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to push workflows for space {}: {}",
                        space_id.cyan(),
                        e
                    );
                }
            }
        }

        // Push agents for this space
        if should_process_api("agents", api_filter) {
            match push_space_agents(project_dir, &space_client).await {
                Ok(count) => {
                    total_agents += count;
                }
                Err(e) => {
                    log::warn!("Failed to push agents for space {}: {}", space_id.cyan(), e);
                }
            }
        }

        // Push tools for this space
        if should_process_api("tools", api_filter) {
            match push_space_tools(project_dir, &space_client).await {
                Ok(count) => {
                    total_tools += count;
                }
                Err(e) => {
                    log::warn!("Failed to push tools for space {}: {}", space_id.cyan(), e);
                }
            }
        }
    }

    log::info!(
        "✓ Push complete: {} saved object(s), {} workflow(s), {} agent(s), {} tool(s)",
        total_saved_objects,
        total_workflows,
        total_agents,
        total_tools
    );
    Ok(total_saved_objects + total_workflows + total_agents + total_tools)
}

/// Bundle saved objects to NDJSON file for distribution
///
/// Pipeline: DirectoryReader → VegaSpecEscaper → FieldEscaper → ManagedFlagAdder → Write to NDJSON
/// Creates per-space bundles in bundle/{space_id}/ directories
///
/// # Arguments
/// * `project_dir` - Project directory path
/// * `output_file` - Output file path (kept for backward compatibility, not used)
/// * `managed` - Whether to mark objects as managed
/// * `space_filter` - Optional list of space IDs to bundle (e.g., ["default", "marketing"])
/// * `api_filter` - Optional list of APIs to bundle (e.g., ["tools", "agents"])
pub async fn bundle_to_ndjson(
    project_dir: impl AsRef<Path>,
    _output_file: impl AsRef<Path>,
    managed: bool,
    space_filter: Option<&[String]>,
    api_filter: Option<&[String]>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    // Determine which spaces to operate on (reads from spaces.yml directly)
    let target_space_ids = get_target_space_ids_from_manifest(project_dir, space_filter);

    let mut total_count = 0;

    // Bundle each managed space
    for space_id in &target_space_ids {
        log::info!("Bundling space: {}", space_id.cyan());

        // Bundle saved objects for this space
        if should_process_api("saved_objects", api_filter) {
            if let Ok(count) = bundle_space_saved_objects(project_dir, space_id, managed).await {
                total_count += count;
            }
        }

        // Bundle workflows for this space
        if should_process_api("workflows", api_filter) {
            if let Ok(count) = bundle_space_workflows(project_dir, space_id).await {
                log::debug!(
                    "Bundled {} workflow(s) for space {}",
                    count,
                    space_id.cyan()
                );
            }
        }

        // Bundle agents for this space
        if should_process_api("agents", api_filter) {
            if let Ok(count) = bundle_space_agents(project_dir, space_id).await {
                log::debug!("Bundled {} agent(s) for space {}", count, space_id.cyan());
            }
        }

        // Bundle tools for this space
        if should_process_api("tools", api_filter) {
            if let Ok(count) = bundle_space_tools(project_dir, space_id).await {
                log::debug!("Bundled {} tool(s) for space {}", count, space_id.cyan());
            }
        }
    }

    // Bundle space definitions (global)
    if should_process_api("spaces", api_filter) {
        let spaces_manifest_path = project_dir.join("spaces.yml");
        if spaces_manifest_path.exists() {
            log::info!("Bundling space definitions...");
            let spaces_output = project_dir.join("bundle/spaces.ndjson");
            match bundle_spaces_to_ndjson_internal(project_dir, &spaces_output).await {
                Ok(space_count) => {
                    log::info!("✓ Bundled {} space definition(s)", space_count);
                }
                Err(e) => {
                    log::warn!("Failed to bundle space definitions: {}", e);
                }
            }
        }
    }

    log::info!(
        "✓ Bundle complete: {} total saved object(s) across all spaces",
        total_count
    );
    Ok(total_count)
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
    let vega_unescape = VegaSpecUnescaper::new();

    let dropped = drop_fields.transform_many(objects)?;
    let unescaped = unescape.transform_many(dropped)?;
    let vega_unescaped = vega_unescape.transform_many(unescaped)?;

    // Load to directory
    use crate::etl::Loader;
    let count = writer.load(vega_unescaped).await?;

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
        let client = load_kibana_client(project_dir)?;
        // TODO: This needs to be refactored for multi-space support
        // For now, hardcode "default" space until multi-space architecture is complete
        let space_client = client.space("default")?;

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
        let extractor = SavedObjectsExtractor::new(space_client, temp_manifest);
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
    let vega_unescape = VegaSpecUnescaper::new();

    let dropped = drop_fields.transform_many(new_objects)?;
    let unescaped = unescape.transform_many(dropped)?;
    let vega_unescaped = vega_unescape.transform_many(unescaped)?;
    let count = writer.load(vega_unescaped).await?;

    log::info!("✓ Wrote {} new object file(s)", count);

    Ok(count)
}

/// Add workflows to an existing manifest
///
/// Can add from Kibana search API or from a file (.json or .ndjson)
/// Optionally filter results by name using regex patterns (--include, --exclude)
pub async fn add_workflows_to_manifest(
    project_dir: impl AsRef<Path>,
    space_id: &str,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    use crate::kibana::workflows::WorkflowEntry;

    let project_dir = project_dir.as_ref();

    // Validate that space is managed (if spaces.yml exists)
    let spaces_manifest_path = project_dir.join("spaces.yml");
    if spaces_manifest_path.exists() {
        let manifest = SpacesManifest::read(&spaces_manifest_path)?;
        if !manifest.spaces.iter().any(|s| s.id == space_id) {
            eyre::bail!(
                "Space {} is not managed. Add it first with: kibob add spaces . --include '^{}$'",
                space_id.cyan(),
                space_id
            );
        }
    }

    // Load or create manifest for this space
    log::info!(
        "Loading workflows manifest for space {} from {}",
        space_id.cyan(),
        project_dir.display()
    );
    let manifest_path = get_space_workflows_manifest(project_dir, space_id);
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
        log::info!(
            "Searching workflows via API in space {}...",
            space_id.cyan()
        );
        let client = load_kibana_client(project_dir)?;
        let space_client = client.space(space_id)?;
        let extractor = WorkflowsExtractor::new(space_client, None);

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

    // Add workflows to space-specific directory
    let workflows_dir = get_space_workflows_dir(project_dir, space_id);
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
            let json = storage::to_string_with_multiline(workflow)?;
            std::fs::write(&workflow_file, json)?;

            log::debug!("Wrote workflow file: {}", workflow_file.display());
            added_count += 1;
        } else {
            log::debug!("Workflow already in manifest, skipping: {}", workflow_name);
        }
    }

    // Create manifest directory if it doesn't exist
    let manifest_dir = get_space_manifest_dir(project_dir, space_id);
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!(
        "✓ Updated manifest for space {} now has {} workflow(s)",
        space_id.cyan(),
        manifest.count()
    );
    log::info!("✓ Added {} new workflow(s)", added_count);

    Ok(added_count)
}

/// Add spaces to an existing manifest
///
/// Can add from Kibana search API or from a file (.json or .ndjson)
/// Optionally filter results by name using regex patterns (--include, --exclude)
/// or by space ID list (--space)
pub async fn add_spaces_to_manifest(
    project_dir: impl AsRef<Path>,
    space_filter: Option<&[String]>,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    // Load or create manifest in project root
    log::info!("Loading spaces manifest from {}", project_dir.display());
    let manifest_path = project_dir.join("spaces.yml");
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
        let client = load_kibana_client(project_dir)?;
        let extractor = SpacesExtractor::new(client, None);

        extractor.search_spaces(query.as_deref()).await?
    };

    log::info!("Found {} space(s) before filtering", new_spaces.len());

    // Apply regex and ID filters: include first, then exclude, then space ID list
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

        // Apply space ID list filter (if specified)
        if let Some(filter_ids) = space_filter {
            spaces = spaces
                .into_iter()
                .filter(|s| {
                    s.get("id")
                        .and_then(|v| v.as_str())
                        .map(|id| filter_ids.contains(&id.to_string()))
                        .unwrap_or(false)
                })
                .collect();

            log::info!("After space ID filter: {} space(s)", spaces.len());
        }

        spaces
    };

    log::info!("Adding {} space(s) after filtering", filtered_spaces.len());

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

            // Write space file to {space_id}/space.json
            let space_file = get_space_file(project_dir, space_id);

            // Ensure the space directory exists
            if let Some(parent) = space_file.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let json = serde_json::to_string_pretty(space)?;
            std::fs::write(&space_file, json)?;

            log::debug!("Wrote space file: {}", space_file.display());
            added_count += 1;
        } else {
            log::debug!("Space already in manifest, skipping: {}", space_id);
        }
    }

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!("✓ Updated manifest now has {} space(s)", manifest.count());
    log::info!("✓ Added {} new space(s)", added_count);

    Ok(added_count)
}

/// Load spaces manifest from project directory
///
/// Reads `spaces.yml` from the project directory
fn load_spaces_manifest(project_dir: impl AsRef<Path>) -> Result<SpacesManifest> {
    let manifest_path = project_dir.as_ref().join("spaces.yml");

    if !manifest_path.exists() {
        eyre::bail!("Spaces manifest not found: {}", manifest_path.display());
    }

    SpacesManifest::read(&manifest_path)
}

/// Get target space IDs to operate on from KibanaClient
///
/// If `space_filter` is provided, returns only those space IDs.
/// Otherwise, returns all space IDs from the client.
fn get_target_space_ids(client: &KibanaClient, space_filter: Option<&[String]>) -> Vec<String> {
    if let Some(filter) = space_filter {
        filter.to_vec()
    } else {
        // Return all space IDs from the client
        client.space_ids().into_iter().map(String::from).collect()
    }
}

/// Get target space IDs to operate on from the spaces manifest file directly
///
/// If `space_filter` is provided, returns only those space IDs.
/// Otherwise, returns all space IDs from the manifest file.
/// Falls back to ["default"] if no manifest exists.
fn get_target_space_ids_from_manifest(
    project_dir: &Path,
    space_filter: Option<&[String]>,
) -> Vec<String> {
    if let Some(filter) = space_filter {
        return filter.to_vec();
    }

    // Try to load from spaces.yml
    let manifest_path = project_dir.join("spaces.yml");
    if manifest_path.exists() {
        if let Ok(manifest) = SpacesManifest::read(&manifest_path) {
            return manifest.spaces.into_iter().map(|s| s.id).collect();
        }
    }

    // Default to ["default"]
    vec!["default".to_string()]
}

/// Pull spaces from Kibana to local directory (internal)
///
/// Pipeline: SpacesExtractor → Write to {space_id}/space.json files
async fn pull_spaces_internal(
    project_dir: impl AsRef<Path>,
    client: &KibanaClient,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading spaces manifest from {}", project_dir.display());
    let manifest = load_spaces_manifest(project_dir)?;
    log::debug!("Manifest loaded: {} space(s)", manifest.count());

    // Build the extract pipeline
    log::debug!("Extracting spaces from Kibana...");
    let extractor = SpacesExtractor::new(client.clone(), Some(manifest));

    // Extract spaces
    let spaces = extractor.extract().await?;

    // Write each space to its own space.json file in its directory
    let mut count = 0;
    for space in &spaces {
        // Get space ID (required)
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        // Write to {space_id}/space.json
        let space_file = get_space_file(project_dir, space_id);

        // Ensure the space directory exists
        if let Some(parent) = space_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(space)?;
        std::fs::write(&space_file, json)?;

        log::debug!("Wrote space: {}", space_file.display());
        count += 1;
    }

    Ok(count)
}

/// Pull spaces from Kibana to local directory
///
/// Pipeline: SpacesExtractor → Write to {space_id}/space.json files
pub async fn pull_spaces(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading spaces manifest from {}", project_dir.display());
    let manifest = load_spaces_manifest(project_dir)?;
    log::info!("Manifest loaded: {} space(s)", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;

    // Build the extract pipeline
    log::info!("Extracting spaces from Kibana...");
    let extractor = SpacesExtractor::new(client, Some(manifest));

    // Extract spaces
    let spaces = extractor.extract().await?;

    // Write each space to its own space.json file in its directory
    let mut count = 0;
    for space in &spaces {
        // Get space ID (required)
        let space_id = space
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Space missing 'id' field"))?;

        // Write to {space_id}/space.json
        let space_file = get_space_file(project_dir, space_id);

        // Ensure the space directory exists
        if let Some(parent) = space_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(space)?;
        std::fs::write(&space_file, json)?;

        log::debug!("Wrote space: {}", space_file.display());
        count += 1;
    }

    log::info!("✓ Pulled {} space(s)", count);

    Ok(count)
}

/// Push spaces from local directory to Kibana (internal)
///
/// Pipeline: Read from {space_id}/space.json → SpacesLoader
async fn push_spaces_internal(
    project_dir: impl AsRef<Path>,
    client: &KibanaClient,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::debug!("Loading spaces from {}", project_dir.display());

    // Load spaces manifest to know which spaces to push
    let manifest = load_spaces_manifest(project_dir)?;

    // Read space.json from each space directory
    let mut spaces = Vec::new();
    for entry in &manifest.spaces {
        let space_file = get_space_file(project_dir, &entry.id);

        if !space_file.exists() {
            log::warn!(
                "Space file not found for '{}': {}",
                entry.id,
                space_file.display()
            );
            continue;
        }

        let content = std::fs::read_to_string(&space_file)?;
        let space: serde_json::Value = serde_json::from_str(&content)?;
        spaces.push(space);
    }

    log::debug!("Read {} space(s) from disk", spaces.len());

    let loader = SpacesLoader::new(client.clone());

    // Load spaces to Kibana
    let count = loader.load(spaces).await?;

    Ok(count)
}

/// Push spaces from local directory to Kibana
///
/// Pipeline: Read from {space_id}/space.json → SpacesLoader
pub async fn push_spaces(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading spaces from {}", project_dir.display());

    // Load spaces manifest to know which spaces to push
    let manifest = load_spaces_manifest(project_dir)?;

    // Read space.json from each space directory
    let mut spaces = Vec::new();
    for entry in &manifest.spaces {
        let space_file = get_space_file(project_dir, &entry.id);

        if !space_file.exists() {
            log::warn!(
                "Space file not found for '{}': {}",
                entry.id,
                space_file.display()
            );
            continue;
        }

        let content = std::fs::read_to_string(&space_file)?;
        let space: serde_json::Value = serde_json::from_str(&content)?;
        spaces.push(space);
    }

    log::info!("Read {} space(s) from disk", spaces.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;

    let loader = SpacesLoader::new(client);

    // Load spaces to Kibana
    let count = loader.load(spaces).await?;

    log::info!("✓ Pushed {} space(s) to Kibana", count);

    Ok(count)
}

/// Bundle spaces to NDJSON file for distribution (internal)
///
/// Pipeline: Read from {space_id}/space.json → Write to spaces.ndjson
async fn bundle_spaces_to_ndjson_internal(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::debug!("Loading spaces from {}", project_dir.display());

    // Load spaces manifest to know which spaces to bundle
    let manifest = load_spaces_manifest(project_dir)?;

    // Read space.json from each space directory
    let mut spaces = Vec::new();
    for entry in &manifest.spaces {
        let space_file = get_space_file(project_dir, &entry.id);

        if !space_file.exists() {
            log::warn!(
                "Space file not found for '{}': {}",
                entry.id,
                space_file.display()
            );
            continue;
        }

        let content = std::fs::read_to_string(&space_file)?;
        let space: serde_json::Value = serde_json::from_str(&content)?;
        spaces.push(space);
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
/// Pipeline: Read from {space_id}/space.json → Write to spaces.ndjson
pub async fn bundle_spaces_to_ndjson(
    project_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();
    let output_file = output_file.as_ref();

    log::info!("Loading spaces from {}", project_dir.display());

    // Load spaces manifest to know which spaces to bundle
    let manifest = load_spaces_manifest(project_dir)?;

    // Read space.json from each space directory
    let mut spaces = Vec::new();
    for entry in &manifest.spaces {
        let space_file = get_space_file(project_dir, &entry.id);

        if !space_file.exists() {
            log::warn!(
                "Space file not found for '{}': {}",
                entry.id,
                space_file.display()
            );
            continue;
        }

        let content = std::fs::read_to_string(&space_file)?;
        let space: serde_json::Value = serde_json::from_str(&content)?;
        spaces.push(space);
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

/// Pull workflows from Kibana to local directory
///
/// Pipeline: WorkflowsExtractor → Write to workflows/<name>.json files
pub async fn pull_workflows(project_dir: impl AsRef<Path>) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    log::info!("Loading workflows manifest from {}", project_dir.display());
    let manifest = load_workflows_manifest(project_dir)?;
    log::info!("Manifest loaded: {} workflow(s)", manifest.count());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;
    // TODO: This needs to be refactored for multi-space support
    let space_client = client.space("default")?;

    // Build the extract pipeline
    log::info!("Extracting workflows from Kibana...");
    let extractor = WorkflowsExtractor::new(space_client, Some(manifest));

    // Extract workflows
    let workflows = extractor.extract().await?;

    // Apply YAML formatting transform
    use crate::etl::Transformer;
    use crate::transform::YamlFormatter;

    let formatter = YamlFormatter::for_workflows();
    let formatted_workflows: Result<Vec<_>> = workflows
        .into_iter()
        .map(|w| formatter.transform(w))
        .collect();
    let formatted_workflows = formatted_workflows?;

    // Write each workflow to its own JSON file
    let workflows_dir = project_dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;

    let mut count = 0;
    for workflow in &formatted_workflows {
        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'name' field"))?;

        let workflow_file = workflows_dir.join(format!("{}.json", workflow_name));
        let json = storage::to_string_with_multiline(workflow)?;
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
            let workflow = storage::read_json5_file(&path)?;
            workflows.push(workflow);
        }
    }

    log::info!("Read {} workflow(s) from disk", workflows.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;
    // TODO: This needs to be refactored for multi-space support
    let space_client = client.space("default")?;

    let loader = WorkflowsLoader::new(space_client);

    // Load workflows to Kibana
    let count = loader.load(workflows).await?;

    log::info!("✓ Pushed {} workflow(s) to Kibana", count);

    Ok(count)
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
    space_id: &str,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    use crate::kibana::agents::AgentEntry;

    let project_dir = project_dir.as_ref();

    // Validate that space is managed (if spaces.yml exists)
    let spaces_manifest_path = project_dir.join("spaces.yml");
    if spaces_manifest_path.exists() {
        let manifest = SpacesManifest::read(&spaces_manifest_path)?;
        if !manifest.spaces.iter().any(|s| s.id == space_id) {
            eyre::bail!(
                "Space {} is not managed. Add it first with: kibob add spaces . --include '^{}$'",
                space_id.cyan(),
                space_id
            );
        }
    }

    // Load or create manifest for this space
    log::info!(
        "Loading agents manifest for space {} from {}",
        space_id.cyan(),
        project_dir.display()
    );
    let manifest_path = get_space_agents_manifest(project_dir, space_id);
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
        log::info!("Searching agents via API in space {}...", space_id.cyan());
        let client = load_kibana_client(project_dir)?;
        let space_client = client.space(space_id)?;
        let extractor = AgentsExtractor::new(space_client, None);

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

    // Add agents to space-specific directory
    let agents_dir = get_space_agents_dir(project_dir, space_id);
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
    let manifest_dir = get_space_manifest_dir(project_dir, space_id);
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!(
        "✓ Updated manifest for space {} now has {} agent(s)",
        space_id.cyan(),
        manifest.count()
    );
    log::info!("✓ Added {} new agent(s)", added_count);

    Ok(added_count)
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
    let client = load_kibana_client(project_dir)?;
    // TODO: This needs to be refactored for multi-space support
    let space_client = client.space("default")?;

    // Build the extract pipeline
    log::info!("Extracting agents from Kibana...");
    let extractor = AgentsExtractor::new(space_client, Some(manifest));

    // Extract agents
    let agents = extractor.extract().await?;

    // Transform agents - format multiline instructions field
    let formatter = MultilineFieldFormatter::for_agents();
    let agents: Vec<_> = agents
        .into_iter()
        .map(|agent| formatter.transform(agent))
        .collect::<Result<_>>()?;

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
        let json = storage::to_string_with_multiline(agent)?;
        std::fs::write(&agent_file, json)?;

        log::debug!("Wrote agent: {}", agent_file.display());
        count += 1;
    }

    log::info!("✓ Pulled {} agent(s) to {}", count, agents_dir.display());

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
            let agent = storage::read_json5_file(&path)?;
            agents.push(agent);
        }
    }

    log::info!("Read {} agent(s) from disk", agents.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;
    let space_client = client.space("default")?;

    let loader = AgentsLoader::new(space_client);

    // Load agents to Kibana
    let count = loader.load(agents).await?;

    log::info!("✓ Pushed {} agent(s) to Kibana", count);

    Ok(count)
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
            let agent = storage::read_json5_file(&path)?;
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
    space_id: &str,
    query: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    file_path: Option<String>,
) -> Result<usize> {
    let project_dir = project_dir.as_ref();

    // Validate that space is managed by attempting to create a space client
    // This will fail if the space is not in spaces.yml
    let spaces_manifest_path = project_dir.join("spaces.yml");
    if spaces_manifest_path.exists() {
        let client = load_kibana_client(project_dir)?;
        if client.space(space_id).is_err() {
            eyre::bail!(
                "Space {} is not managed. Add it first with: kibob add spaces . --include '^{}$'",
                space_id.cyan(),
                space_id
            );
        }
    }

    // Load or create manifest for this space
    log::info!(
        "Loading tools manifest for space {} from {}",
        space_id.cyan(),
        project_dir.display()
    );
    let manifest_path = get_space_tools_manifest(project_dir, space_id);
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
        log::info!("Searching tools via API in space {}...", space_id.cyan());
        let client = load_kibana_client(project_dir)?;
        let space_client = client.space(space_id)?;
        let extractor = ToolsExtractor::new(space_client, None);

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

    // Add tools to space-specific directory
    let tools_dir = get_space_tools_dir(project_dir, space_id);
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
    let manifest_dir = get_space_manifest_dir(project_dir, space_id);
    std::fs::create_dir_all(&manifest_dir)?;

    // Save updated manifest
    manifest.write(&manifest_path)?;
    log::info!(
        "✓ Updated manifest for space {} now has {} tool(s)",
        space_id.cyan(),
        manifest.count()
    );
    log::info!("✓ Added {} new tool(s)", added_count);

    Ok(added_count)
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
    let client = load_kibana_client(project_dir)?;
    let space_client = client.space("default")?;

    // Build the extract pipeline
    log::info!("Extracting tools from Kibana...");
    let extractor = ToolsExtractor::new(space_client, Some(manifest));

    // Extract tools
    let tools = extractor.extract().await?;

    // Transform tools - format multiline query field
    let formatter = MultilineFieldFormatter::for_tools();
    let tools: Vec<_> = tools
        .into_iter()
        .map(|tool| formatter.transform(tool))
        .collect::<Result<_>>()?;

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
        let json = storage::to_string_with_multiline(tool)?;
        std::fs::write(&tool_file, json)?;

        log::debug!("Wrote tool: {}", tool_file.display());
        count += 1;
    }

    log::info!("✓ Pulled {} tool(s) to {}", count, tools_dir.display());

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
            let tool = storage::read_json5_file(&path)?;
            tools.push(tool);
        }
    }

    log::info!("Read {} tool(s) from disk", tools.len());

    log::info!("Connecting to Kibana...");
    let client = load_kibana_client(project_dir)?;
    let space_client = client.space("default")?;

    let loader = ToolsLoader::new(space_client);

    // Load tools to Kibana
    let count = loader.load(tools).await?;

    log::info!("✓ Pushed {} tool(s) to Kibana", count);

    Ok(count)
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
            let tool = storage::read_json5_file(&path)?;
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

//
// Helper functions for loading space-specific manifests
//

/// Pull saved objects for a specific space
async fn pull_space_saved_objects(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let manifest_path = get_space_saved_objects_manifest(project_dir, space_id);

    if !manifest_path.exists() {
        log::debug!(
            "No saved objects manifest for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Pulling saved objects for space {}", space_id.cyan());
    let manifest = crate::kibana::saved_objects::SavedObjectsManifest::read(&manifest_path)?;
    log::debug!("Loaded {} object(s) from manifest", manifest.count());

    let extractor = SavedObjectsExtractor::new(client.clone(), manifest);

    // Transform: Drop metadata fields and unescape JSON strings
    let drop_fields = FieldDropper::default_kibana_fields();
    let unescape = FieldUnescaper::default_kibana_fields();
    let vega_unescape = VegaSpecUnescaper::new();

    // Load to space-specific directory
    let objects_dir = get_space_objects_dir(project_dir, space_id);
    let writer = DirectoryWriter::new_with_options(&objects_dir, true)?;

    // Clear directory before writing
    writer.clear()?;

    // Extract → Drop → Unescape → VegaUnescape → Load
    let objects = extractor.extract().await?;
    let dropped = drop_fields.transform_many(objects)?;
    let unescaped = unescape.transform_many(dropped)?;
    let vega_unescaped = vega_unescape.transform_many(unescaped)?;
    let count = writer.load(vega_unescaped).await?;

    log::info!(
        "✓ Pulled {} saved object(s) for space {}",
        count,
        space_id.cyan()
    );
    Ok(count)
}

/// Pull workflows for a specific space
async fn pull_space_workflows(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let manifest_path = get_space_workflows_manifest(project_dir, space_id);

    if !manifest_path.exists() {
        log::debug!(
            "No workflows manifest for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Pulling workflows for space {}", space_id.cyan());
    let manifest = WorkflowsManifest::read(&manifest_path)?;
    log::debug!("Loaded {} workflow(s) from manifest", manifest.count());

    let extractor = WorkflowsExtractor::new(client.clone(), Some(manifest));
    let workflows = extractor.extract().await?;

    // Apply YAML formatting transform
    use crate::etl::Transformer;
    use crate::transform::YamlFormatter;

    let formatter = YamlFormatter::for_workflows();
    let formatted_workflows: Result<Vec<_>> = workflows
        .into_iter()
        .map(|w| formatter.transform(w))
        .collect();
    let formatted_workflows = formatted_workflows?;

    // Write each workflow to its own JSON file
    let workflows_dir = get_space_workflows_dir(project_dir, space_id);
    std::fs::create_dir_all(&workflows_dir)?;

    let mut count = 0;
    for workflow in &formatted_workflows {
        let workflow_name = workflow
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Workflow missing 'name' field"))?;

        let workflow_file = workflows_dir.join(format!("{}.json", workflow_name));
        let json = storage::to_string_with_multiline(workflow)?;
        std::fs::write(&workflow_file, json)?;
        count += 1;
    }

    log::info!(
        "✓ Pulled {} workflow(s) for space {}",
        count,
        space_id.cyan()
    );
    Ok(count)
}

/// Pull agents for a specific space
async fn pull_space_agents(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let manifest_path = get_space_agents_manifest(project_dir, space_id);

    if !manifest_path.exists() {
        log::debug!("No agents manifest for space {}, skipping", space_id.cyan());
        return Ok(0);
    }

    log::info!("Pulling agents for space {}", space_id.cyan());
    let manifest = AgentsManifest::read(&manifest_path)?;
    log::debug!("Loaded {} agent(s) from manifest", manifest.count());

    let extractor = AgentsExtractor::new(client.clone(), Some(manifest));
    let agents = extractor.extract().await?;

    // Transform agents - format multiline instructions field
    let formatter = MultilineFieldFormatter::for_agents();
    let agents: Vec<_> = agents
        .into_iter()
        .map(|agent| formatter.transform(agent))
        .collect::<Result<_>>()?;

    // Write each agent to its own JSON file
    let agents_dir = get_space_agents_dir(project_dir, space_id);
    std::fs::create_dir_all(&agents_dir)?;

    let mut count = 0;
    for agent in &agents {
        // Use name or id for filename
        let agent_name = agent
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| agent.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| eyre::eyre!("Agent missing both 'name' and 'id' fields"))?;

        let agent_file = agents_dir.join(format!("{}.json", agent_name));
        let json = storage::to_string_with_multiline(agent)?;
        std::fs::write(&agent_file, json)?;
        count += 1;
    }

    log::info!("✓ Pulled {} agent(s) for space {}", count, space_id.cyan());
    Ok(count)
}

/// Pull tools for a specific space
async fn pull_space_tools(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let manifest_path = get_space_tools_manifest(project_dir, space_id);

    if !manifest_path.exists() {
        log::debug!("No tools manifest for space {}, skipping", space_id.cyan());
        return Ok(0);
    }

    log::info!("Pulling tools for space {}", space_id.cyan());
    let manifest = ToolsManifest::read(&manifest_path)?;
    log::debug!("Loaded {} tool(s) from manifest", manifest.count());

    let extractor = ToolsExtractor::new(client.clone(), Some(manifest));
    let tools = extractor.extract().await?;

    // Transform tools - format multiline query field
    let formatter = MultilineFieldFormatter::for_tools();
    let tools: Vec<_> = tools
        .into_iter()
        .map(|tool| formatter.transform(tool))
        .collect::<Result<_>>()?;

    // Write each tool to its own JSON file
    let tools_dir = get_space_tools_dir(project_dir, space_id);
    std::fs::create_dir_all(&tools_dir)?;

    let mut count = 0;
    for tool in &tools {
        // Use name or id for filename
        let tool_name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| tool.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| eyre::eyre!("Tool missing both 'name' and 'id' fields"))?;

        let tool_file = tools_dir.join(format!("{}.json", tool_name));
        let json = storage::to_string_with_multiline(tool)?;
        std::fs::write(&tool_file, json)?;
        count += 1;
    }

    log::info!("✓ Pulled {} tool(s) for space {}", count, space_id.cyan());
    Ok(count)
}

//
// Push helper functions for multi-space support
//

/// Push saved objects for a specific space
async fn push_space_saved_objects(
    project_dir: &Path,
    client: &KibanaClient,
    managed: bool,
) -> Result<usize> {
    let space_id = client.space_id();
    let objects_dir = get_space_objects_dir(project_dir, space_id);

    if !objects_dir.exists() {
        log::debug!(
            "No objects directory for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Pushing saved objects for space {}", space_id.cyan());
    let reader = DirectoryReader::new(&objects_dir);

    // Transform: Escape Vega specs, escape JSON strings, and add managed flag
    let vega_escaper = VegaSpecEscaper::new();
    let escaper = FieldEscaper::default_kibana_fields();
    let managed_flag = ManagedFlagAdder::new(managed);

    let loader = SavedObjectsLoader::new(client.clone());

    // Read → Vega Escape → Field Escape → Add Managed Flag → Load
    let objects = reader.extract().await?;
    let vega_escaped = vega_escaper.transform_many(objects)?;
    let escaped = escaper.transform_many(vega_escaped)?;
    let flagged = managed_flag.transform_many(escaped)?;
    let count = loader.load(flagged).await?;

    log::info!(
        "✓ Pushed {} saved object(s) for space {}",
        count,
        space_id.cyan()
    );
    Ok(count)
}

/// Push workflows for a specific space
async fn push_space_workflows(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let workflows_dir = get_space_workflows_dir(project_dir, space_id);

    if !workflows_dir.exists() {
        log::debug!(
            "No workflows directory for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Pushing workflows for space {}", space_id.cyan());

    // Read all JSON files from workflows directory
    let mut workflows = Vec::new();
    for entry in std::fs::read_dir(&workflows_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let workflow = storage::read_json5_file(&path)?;
            workflows.push(workflow);
        }
    }

    let loader = WorkflowsLoader::new(client.clone());
    let count = loader.load(workflows).await?;

    log::info!(
        "✓ Pushed {} workflow(s) for space {}",
        count,
        space_id.cyan()
    );
    Ok(count)
}

/// Push agents for a specific space
async fn push_space_agents(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let agents_dir = get_space_agents_dir(project_dir, space_id);

    if !agents_dir.exists() {
        log::debug!(
            "No agents directory for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Pushing agents for space {}", space_id.cyan());

    // Read all JSON files from agents directory
    let mut agents = Vec::new();
    for entry in std::fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let agent = storage::read_json5_file(&path)?;
            agents.push(agent);
        }
    }

    let loader = AgentsLoader::new(client.clone());
    let count = loader.load(agents).await?;

    log::info!("✓ Pushed {} agent(s) for space {}", count, space_id.cyan());
    Ok(count)
}

/// Push tools for a specific space
async fn push_space_tools(project_dir: &Path, client: &KibanaClient) -> Result<usize> {
    let space_id = client.space_id();
    let tools_dir = get_space_tools_dir(project_dir, space_id);

    if !tools_dir.exists() {
        log::debug!("No tools directory for space {}, skipping", space_id.cyan());
        return Ok(0);
    }

    log::info!("Pushing tools for space {}", space_id.cyan());

    // Read all JSON files from tools directory
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let tool = storage::read_json5_file(&path)?;
            tools.push(tool);
        }
    }

    let loader = ToolsLoader::new(client.clone());
    let count = loader.load(tools).await?;

    log::info!("✓ Pushed {} tool(s) for space {}", count, space_id.cyan());
    Ok(count)
}

//
// Bundle helper functions for multi-space support
//

/// Bundle saved objects for a specific space
async fn bundle_space_saved_objects(
    project_dir: &Path,
    space_id: &str,
    managed: bool,
) -> Result<usize> {
    let objects_dir = get_space_objects_dir(project_dir, space_id);

    if !objects_dir.exists() {
        log::debug!(
            "No objects directory for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Bundling saved objects for space {}", space_id.cyan());

    let reader = DirectoryReader::new(&objects_dir);

    // Transform: Escape Vega specs, escape JSON strings, and add managed flag
    let vega_escaper = VegaSpecEscaper::new();
    let escaper = FieldEscaper::default_kibana_fields();
    let managed_flag = ManagedFlagAdder::new(managed);

    // Read → Vega Escape → Field Escape → Add Managed Flag
    let objects = reader.extract().await?;
    let vega_escaped = vega_escaper.transform_many(objects)?;
    let escaped = escaper.transform_many(vega_escaped)?;
    let flagged = managed_flag.transform_many(escaped)?;

    // Write to NDJSON file
    let bundle_dir = get_space_bundle_dir(project_dir, space_id);
    std::fs::create_dir_all(&bundle_dir)?;
    let output_file = bundle_dir.join("saved_objects.ndjson");

    use std::io::Write;
    let mut file = std::fs::File::create(&output_file)?;
    for obj in &flagged {
        let json_line = serde_json::to_string(obj)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} saved object(s) for space {} to {}",
        flagged.len(),
        space_id.cyan(),
        output_file.display()
    );
    Ok(flagged.len())
}

/// Bundle workflows for a specific space
async fn bundle_space_workflows(project_dir: &Path, space_id: &str) -> Result<usize> {
    let workflows_dir = get_space_workflows_dir(project_dir, space_id);

    if !workflows_dir.exists() {
        log::debug!(
            "No workflows directory for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Bundling workflows for space {}", space_id.cyan());

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

    // Write to NDJSON file
    let bundle_dir = get_space_bundle_dir(project_dir, space_id);
    std::fs::create_dir_all(&bundle_dir)?;
    let output_file = bundle_dir.join("workflows.ndjson");

    use std::io::Write;
    let mut file = std::fs::File::create(&output_file)?;
    for workflow in &workflows {
        let json_line = serde_json::to_string(workflow)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} workflow(s) for space {} to {}",
        workflows.len(),
        space_id.cyan(),
        output_file.display()
    );
    Ok(workflows.len())
}

/// Bundle agents for a specific space
async fn bundle_space_agents(project_dir: &Path, space_id: &str) -> Result<usize> {
    let agents_dir = get_space_agents_dir(project_dir, space_id);

    if !agents_dir.exists() {
        log::debug!(
            "No agents directory for space {}, skipping",
            space_id.cyan()
        );
        return Ok(0);
    }

    log::info!("Bundling agents for space {}", space_id.cyan());

    // Read all JSON files from agents directory
    let mut agents = Vec::new();
    for entry in std::fs::read_dir(&agents_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let agent = storage::read_json5_file(&path)?;
            agents.push(agent);
        }
    }

    // Write to NDJSON file
    let bundle_dir = get_space_bundle_dir(project_dir, space_id);
    std::fs::create_dir_all(&bundle_dir)?;
    let output_file = bundle_dir.join("agents.ndjson");

    use std::io::Write;
    let mut file = std::fs::File::create(&output_file)?;
    for agent in &agents {
        let json_line = serde_json::to_string(agent)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} agent(s) for space {} to {}",
        agents.len(),
        space_id.cyan(),
        output_file.display()
    );
    Ok(agents.len())
}

/// Bundle tools for a specific space
async fn bundle_space_tools(project_dir: &Path, space_id: &str) -> Result<usize> {
    let tools_dir = get_space_tools_dir(project_dir, space_id);

    if !tools_dir.exists() {
        log::debug!("No tools directory for space {}, skipping", space_id.cyan());
        return Ok(0);
    }

    log::info!("Bundling tools for space {}", space_id.cyan());

    // Read all JSON files from tools directory
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let tool = storage::read_json5_file(&path)?;
            tools.push(tool);
        }
    }

    // Write to NDJSON file
    let bundle_dir = get_space_bundle_dir(project_dir, space_id);
    std::fs::create_dir_all(&bundle_dir)?;
    let output_file = bundle_dir.join("tools.ndjson");

    use std::io::Write;
    let mut file = std::fs::File::create(&output_file)?;
    for tool in &tools {
        let json_line = serde_json::to_string(tool)?;
        writeln!(file, "{}", json_line)?;
    }

    log::info!(
        "✓ Bundled {} tool(s) for space {} to {}",
        tools.len(),
        space_id.cyan(),
        output_file.display()
    );
    Ok(tools.len())
}

//
// Path resolution helpers for multi-space support
//
// Path resolution helpers for multi-space support
//

/// Get space-specific directory
///
/// Returns the root directory for a specific space (e.g., `{project_dir}/default/`)
fn get_space_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    project_dir.join(space_id)
}

/// Get space-specific manifest directory
///
/// Returns the manifest directory for a specific space (e.g., `{project_dir}/default/manifest/`)
fn get_space_manifest_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_dir(project_dir, space_id).join("manifest")
}

/// Get objects directory for a space
///
/// Returns the objects directory for a specific space (e.g., `{project_dir}/default/objects/`)
fn get_space_objects_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_dir(project_dir, space_id).join("objects")
}

/// Get workflows directory for a space
///
/// Returns the workflows directory for a specific space (e.g., `{project_dir}/default/workflows/`)
fn get_space_workflows_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_dir(project_dir, space_id).join("workflows")
}

/// Get agents directory for a space
///
/// Returns the agents directory for a specific space (e.g., `{project_dir}/default/agents/`)
fn get_space_agents_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_dir(project_dir, space_id).join("agents")
}

/// Get tools directory for a space
///
/// Returns the tools directory for a specific space (e.g., `{project_dir}/default/tools/`)
fn get_space_tools_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_dir(project_dir, space_id).join("tools")
}

/// Get saved_objects manifest path for a space
///
/// Returns the saved_objects manifest path for a specific space (e.g., `{project_dir}/default/manifest/saved_objects.json`)
fn get_space_saved_objects_manifest(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_manifest_dir(project_dir, space_id).join("saved_objects.json")
}

/// Get workflows manifest path for a space
///
/// Returns the workflows manifest path for a specific space (e.g., `{project_dir}/default/manifest/workflows.yml`)
fn get_space_workflows_manifest(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_manifest_dir(project_dir, space_id).join("workflows.yml")
}

/// Get agents manifest path for a space
///
/// Returns the agents manifest path for a specific space (e.g., `{project_dir}/default/manifest/agents.yml`)
fn get_space_agents_manifest(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_manifest_dir(project_dir, space_id).join("agents.yml")
}

/// Get tools manifest path for a space
///
/// Returns the tools manifest path for a specific space (e.g., `{project_dir}/default/manifest/tools.yml`)
fn get_space_tools_manifest(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_manifest_dir(project_dir, space_id).join("tools.yml")
}

/// Get bundle directory for a space
///
/// Returns the bundle directory for a specific space (e.g., `{project_dir}/bundle/default/`)
fn get_space_bundle_dir(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    project_dir.join("bundle").join(space_id)
}

/// Get space definition file path
///
/// Returns the space.json file path for a specific space (e.g., `{project_dir}/default/space.json`)
fn get_space_file(project_dir: &Path, space_id: &str) -> std::path::PathBuf {
    get_space_dir(project_dir, space_id).join("space.json")
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

        let result = load_kibana_client(".");
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

        let result = load_kibana_client(".");
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

        let result = load_kibana_client(".");
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

    #[test]
    #[serial_test::serial]
    fn test_get_target_space_ids() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project_dir = temp_dir.path();
        let url = url::Url::parse("http://localhost:5601").unwrap();

        // Create a spaces.yml to populate client.space_ids()
        let manifest_path = project_dir.join("spaces.yml");
        let manifest = crate::kibana::spaces::SpacesManifest::with_spaces(vec![
            crate::kibana::spaces::SpaceEntry::new("default".into(), "Default".into()),
            crate::kibana::spaces::SpaceEntry::new("marketing".into(), "Marketing".into()),
        ]);
        manifest.write(&manifest_path).unwrap();

        let client = KibanaClient::try_new(url, Auth::None, project_dir).unwrap();

        // No filter
        let mut ids = get_target_space_ids(&client, None);
        ids.sort();
        let mut expected = vec!["default".to_string(), "marketing".to_string()];
        expected.sort();
        assert_eq!(ids, expected);

        // Single space filter
        let ids = get_target_space_ids(&client, Some(&["default".to_string()]));
        assert_eq!(ids, vec!["default"]);

        // Multiple space filter
        let mut ids = get_target_space_ids(
            &client,
            Some(&["default".to_string(), "marketing".to_string()]),
        );
        ids.sort();
        assert_eq!(ids, expected);
    }

    #[test]
    #[serial_test::serial]
    fn test_get_target_space_ids_from_manifest() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // No manifest
        let ids = get_target_space_ids_from_manifest(project_dir, None);
        assert_eq!(ids, vec!["default"]);

        // With manifest
        let manifest_path = project_dir.join("spaces.yml");
        let manifest = crate::kibana::spaces::SpacesManifest::with_spaces(vec![
            crate::kibana::spaces::SpaceEntry::new("default".into(), "Default".into()),
            crate::kibana::spaces::SpaceEntry::new("eng".into(), "Engineering".into()),
        ]);
        manifest.write(&manifest_path).unwrap();

        let mut ids = get_target_space_ids_from_manifest(project_dir, None);
        ids.sort();
        let mut expected = vec!["default".to_string(), "eng".to_string()];
        expected.sort();
        assert_eq!(ids, expected);

        // With filter
        let ids = get_target_space_ids_from_manifest(project_dir, Some(&["eng".to_string()]));
        assert_eq!(ids, vec!["eng"]);
    }
}
