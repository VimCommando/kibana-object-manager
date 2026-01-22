use clap::{Parser, Subcommand, builder::styling};
use eyre::Result;
use kibana_object_manager::{
    cli::{
        add_objects_to_manifest, bundle_to_ndjson, init_from_export, load_kibana_client,
        pull_saved_objects, push_saved_objects,
    },
    migration::{MigrationResult, migrate_to_multispace_unified},
};
use owo_colors::OwoColorize;

// CLI Styling
const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::BrightWhite.on_default())
    .usage(styling::AnsiColor::BrightWhite.on_default())
    .literal(styling::AnsiColor::Green.on_default())
    .placeholder(styling::AnsiColor::Cyan.on_default());

/// Kibana Object Manager: --{kibob}-> Git-inspired CLI for managing Kibana saved objects in version control
///
/// Manage dashboards, visualizations, and saved objects with a familiar Git-like workflow.
/// Version control your Kibana artifacts, deploy across environments, and collaborate with Git.
///
/// Environment Variables:
///   KIBANA_URL       Kibana base URL (required)
///   KIBANA_USERNAME  Basic auth username (optional)
///   KIBANA_PASSWORD  Basic auth password (optional)
///   KIBANA_APIKEY    API key authentication (optional, conflicts with user/pass)
///   KIBANA_SPACE     Kibana space ID (default: 'default')
///
/// Examples:
///   kibob auth                              Test connection to Kibana
///   kibob init export.ndjson ./dashboards   Initialize project from export
///   kibob pull .                            Fetch objects from Kibana
///   kibob push . --managed true             Deploy to Kibana as managed objects
#[derive(Parser)]
#[command(name = "kibob", version, styles = STYLES, about, long_about)]
struct Cli {
    /// Dotenv file to load environment variables from
    #[arg(short, long, global = true, default_value = ".env")]
    env: String,

    /// Enable verbose logging (debug level)
    #[arg(long, global = true)]
    debug: bool,

    /// Command to execute
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new project from a Kibana export file
    ///
    /// Creates a manifest and extracts objects into organized directories.
    /// The export file is typically downloaded from Kibana UI (Stack Management → Saved Objects → Export).
    ///
    /// Example:
    ///   kibob init export.ndjson ./my-dashboards
    Init {
        /// NDJSON export file or directory containing export.ndjson
        #[arg(default_value = "export.ndjson")]
        export: String,

        /// Output directory for manifest and objects
        #[arg(default_value = "manifest.json")]
        manifest: String,
    },

    /// Test connection and authentication to Kibana
    ///
    /// Verifies that your credentials and connection are working.
    /// Requires KIBANA_URL and either KIBANA_USERNAME/KIBANA_PASSWORD or KIBANA_APIKEY.
    ///
    /// Example:
    ///   kibob auth
    Auth,

    /// Pull (fetch) saved objects from Kibana to local files
    ///
    /// Downloads objects specified in the manifest from Kibana and saves them locally.
    /// Objects are organized by type in the objects/ directory.
    ///
    /// Examples:
    ///   kibob pull ./my-dashboards
    ///   kibob pull ./my-dashboards --space esdiag
    ///   kibob pull ./my-dashboards --api tools,agents
    Pull {
        /// Project directory containing manifest (default: current directory)
        #[arg(default_value = ".")]
        output_dir: String,

        /// Kibana space(s) to pull from (comma-separated, overrides KIBANA_SPACE env var)
        #[arg(long, value_delimiter = ',')]
        space: Option<Vec<String>>,

        /// Comma-separated list of APIs to pull (e.g., "saved_objects,workflows,agents,tools,spaces")
        #[arg(long, value_delimiter = ',')]
        api: Option<Vec<String>>,
    },

    /// Push (upload) local saved objects to Kibana
    ///
    /// Uploads objects from local files to Kibana. Use --managed true (default) to make
    /// objects read-only in Kibana UI, or --managed false to allow editing.
    ///
    /// Examples:
    ///   kibob push . --managed true    # Read-only in Kibana (recommended for production)
    ///   kibob push . --managed false   # Editable in Kibana
    ///   kibob push . --space esdiag    # Push to specific space
    ///   kibob push . --api tools       # Push only tools
    Push {
        /// Project directory containing objects to upload
        #[arg(default_value = ".")]
        input_dir: String,

        /// Make objects read-only in Kibana UI (managed: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        managed: bool,

        /// Kibana space(s) to push to (comma-separated, overrides KIBANA_SPACE env var)
        #[arg(long, value_delimiter = ',')]
        space: Option<Vec<String>>,

        /// Comma-separated list of APIs to push (e.g., "saved_objects,workflows,agents,tools,spaces")
        #[arg(long, value_delimiter = ',')]
        api: Option<Vec<String>>,
    },

    /// Add items to an existing manifest
    ///
    /// Discovers and adds items via search API or reads from a file.
    /// Supports: objects, workflows, spaces, agents, tools
    ///
    /// Examples:
    ///   kibob add workflows .                          # Search all workflows in default space
    ///   kibob add workflows . --space marketing        # Search in specific space
    ///   kibob add workflows . --query "alert"          # Search for workflows matching "alert"
    ///   kibob add workflows . --include "^prod"        # Include names matching regex "^prod"
    ///   kibob add workflows . --exclude "test"         # Exclude names matching regex "test"
    ///   kibob add workflows . --file export.json       # Add from API response file
    ///   kibob add workflows . --file export.ndjson     # Add from bundle file
    ///   kibob add spaces .                             # Fetch all spaces
    ///   kibob add spaces . --include "prod|staging"    # Include spaces matching pattern
    ///   kibob add agents .                             # Fetch all agents
    ///   kibob add agents . --include "^support"        # Include agents matching pattern
    ///   kibob add tools .                              # Fetch all tools
    ///   kibob add tools . --include "^search"          # Include tools matching pattern
    ///   kibob add objects . --objects "dashboard=abc"  # Legacy: add specific objects by ID
    Add {
        /// API to add to (objects, workflows, spaces, agents, tools)
        api: String,

        /// Project directory with existing manifest
        #[arg(default_value = ".")]
        output_dir: String,

        /// Search query term for API
        #[arg(short, long, conflicts_with_all = &["file", "objects"])]
        query: Option<String>,

        /// Include items matching regex pattern (applied to name field)
        #[arg(short, long)]
        include: Option<String>,

        /// Exclude items matching regex pattern (applied to name field, after include)
        #[arg(short, long)]
        exclude: Option<String>,

        /// File to read from (.json or .ndjson)
        #[arg(long, conflicts_with_all = &["query", "objects"])]
        file: Option<String>,

        /// [objects only] Comma-separated "type=id" pairs to add
        #[arg(short = 'o', long, conflicts_with_all = &["query", "file"])]
        objects: Option<Vec<String>>,

        /// Kibana space(s) to add to/filter by (comma-separated, defaults to "default" for non-space APIs)
        #[arg(long, value_delimiter = ',')]
        space: Option<Vec<String>>,

        /// Exclude dependencies of added items (agents, tools, workflows)
        #[arg(long)]
        exclude_dependencies: bool,
    },

    /// Bundle objects into distributable NDJSON files
    ///
    /// Creates a bundle/ directory with NDJSON files for each API:
    /// - bundle/{space_id}/saved_objects.ndjson - Saved objects per space
    /// - bundle/{space_id}/workflows.ndjson - Workflows per space
    /// - bundle/{space_id}/agents.ndjson - Agents per space
    /// - bundle/{space_id}/tools.ndjson - Tools per space
    /// - bundle/spaces.ndjson - Spaces (if manifest/spaces.yml exists)
    ///
    /// The bundle directory can be easily zipped for distribution.
    ///
    /// Example:
    ///   kibob togo ./my-dashboards
    ///   kibob togo ./my-dashboards --space default
    ///   zip -r dashboards.zip my-dashboards/bundle/
    Togo {
        /// Project directory containing objects to bundle
        #[arg(default_value = ".")]
        input_dir: String,

        /// Set managed flag in bundled objects
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        managed: bool,

        /// Kibana space(s) to bundle (comma-separated, e.g., "default,marketing")
        #[arg(long, value_delimiter = ',')]
        space: Option<Vec<String>>,

        /// Comma-separated list of APIs to bundle (e.g., "saved_objects,workflows,agents,tools,spaces")
        #[arg(long, value_delimiter = ',')]
        api: Option<Vec<String>>,
    },

    /// Migrate legacy structure to multi-space format
    ///
    /// Converts either:
    /// - Legacy manifest.json → manifest/default/saved_objects.json
    /// - Old manifest/saved_objects.json → manifest/default/saved_objects.json
    ///
    /// This is a single-step migration that moves all content to the 'default' space.
    /// Creates a backup by default unless --no-backup is specified.
    ///
    /// Example:
    ///   kibob migrate ./old-project
    Migrate {
        /// Project directory containing legacy manifest.json
        #[arg(default_value = ".")]
        project_dir: String,

        /// Create backup of old manifest.json
        #[arg(short, long, default_value_t = true, action = clap::ArgAction::Set)]
        backup: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Err(e) = dotenvy::from_filename(&cli.env) {
        log::warn!("Failed to load environment variables: {}", e);
    }

    let log_level = match cli.debug {
        true => "debug",
        false => "info",
    };
    let env = env_logger::Env::default().filter_or("LOG_LEVEL", log_level);
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();

    match cli.command {
        Commands::Init { export, manifest } => {
            log::info!(
                "Initializing from {} and building manifest in {}",
                export.bright_black(),
                manifest.bright_black()
            );

            // Determine if export is a file or directory
            let export_path = std::path::Path::new(&export);
            let export_file = if export_path.is_dir() {
                export_path.join("export.ndjson")
            } else {
                export_path.to_path_buf()
            };

            if !export_file.exists() {
                log::error!("Export file not found: {}", export_file.display());
                return Err(eyre::eyre!(
                    "Export file not found: {}",
                    export_file.display()
                ));
            }

            match init_from_export(&export_file, &manifest).await {
                Ok(count) => {
                    log::info!("✓ Initialized {} object(s)", count);
                }
                Err(e) => {
                    log::error!("Init failed: {}", e);
                    return Err(e);
                }
            }
        }
        Commands::Auth => {
            log::info!("Testing authorization to Kibana");

            match load_kibana_client(".") {
                Ok(client) => match client.test_connection().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            log::info!("✓ Authorization successful");
                            log::info!(
                                "  Connected to: {}",
                                std::env::var("KIBANA_URL")
                                    .unwrap_or_else(|_| "unknown".to_string())
                                    .green()
                            );
                        } else {
                            log::error!("✗ Authorization failed: {}", response.status());
                            return Err(eyre::eyre!(
                                "Authorization failed with status: {}",
                                response.status()
                            ));
                        }
                    }
                    Err(e) => {
                        log::error!("✗ Connection test failed: {}", e);
                        return Err(e);
                    }
                },
                Err(e) => {
                    log::error!("✗ Failed to create Kibana client: {}", e);
                    return Err(e);
                }
            }
        }
        Commands::Pull {
            output_dir,
            space,
            api,
        } => {
            log::info!("Pulling objects to: {}", output_dir.bright_black());

            if let Some(spaces) = &space {
                log::info!("Filtering to space(s): {}", spaces.join(", ").cyan());
            }

            if let Some(apis) = &api {
                log::info!("Filtering to API(s): {}", apis.join(", ").cyan());
            }

            match pull_saved_objects(&output_dir, space.as_deref(), api.as_deref()).await {
                Ok(count) => {
                    log::info!("✓ Successfully pulled {} object(s)", count);
                }
                Err(e) => {
                    log::error!("Pull failed: {}", e);
                    return Err(e);
                }
            }
        }
        Commands::Push {
            input_dir,
            managed,
            space,
            api,
        } => {
            log::info!(
                "Pushing {} objects from: {}",
                match managed {
                    true => "managed",
                    false => "unmanaged",
                }
                .cyan(),
                input_dir.bright_black(),
            );

            if let Some(spaces) = &space {
                log::info!("Filtering to space(s): {}", spaces.join(", ").cyan());
            }

            if let Some(apis) = &api {
                log::info!("Filtering to API(s): {}", apis.join(", ").cyan());
            }

            match push_saved_objects(&input_dir, managed, space.as_deref(), api.as_deref()).await {
                Ok(count) => {
                    log::info!("✓ Successfully pushed {} object(s)", count);
                }
                Err(e) => {
                    log::error!("Push failed: {}", e);
                    return Err(e);
                }
            }
        }
        Commands::Add {
            api,
            output_dir,
            query,
            include,
            exclude,
            file,
            objects,
            space,
            exclude_dependencies,
        } => {
            log::info!("Adding {} to {}", api.cyan(), output_dir.bright_black());

            // Route to appropriate handler based on API type
            let count = match api.as_str() {
                "objects" => {
                    // Legacy objects support: --objects flag or --file
                    add_objects_to_manifest(&output_dir, objects, file).await?
                }
                "workflows" => {
                    // Workflows support: --query, --include, --exclude, or --file
                    let target_space = space
                        .as_ref()
                        .and_then(|s| s.first())
                        .map(|s| s.as_str())
                        .unwrap_or("default");
                    log::info!("Using space: {}", target_space.cyan());
                    use kibana_object_manager::cli::add_workflows_to_manifest;
                    add_workflows_to_manifest(
                        &output_dir,
                        target_space,
                        query,
                        include,
                        exclude,
                        file,
                        exclude_dependencies,
                    )
                    .await?
                }
                "spaces" => {
                    // Spaces support: --query (ignored), --include, --exclude, or --file
                    // and --space ID filtering
                    use kibana_object_manager::cli::add_spaces_to_manifest;
                    add_spaces_to_manifest(
                        &output_dir,
                        space.as_deref(),
                        query,
                        include,
                        exclude,
                        file,
                    )
                    .await?
                }
                "agents" => {
                    // Agents support: --query (ignored), --include, --exclude, or --file
                    let target_space = space
                        .as_ref()
                        .and_then(|s| s.first())
                        .map(|s| s.as_str())
                        .unwrap_or("default");
                    log::info!("Using space: {}", target_space.cyan());
                    use kibana_object_manager::cli::add_agents_to_manifest;
                    add_agents_to_manifest(
                        &output_dir,
                        target_space,
                        query,
                        include,
                        exclude,
                        file,
                        exclude_dependencies,
                    )
                    .await?
                }
                "tools" => {
                    // Tools support: --query (ignored), --include, --exclude, or --file
                    let target_space = space
                        .as_ref()
                        .and_then(|s| s.first())
                        .map(|s| s.as_str())
                        .unwrap_or("default");
                    log::info!("Using space: {}", target_space.cyan());
                    use kibana_object_manager::cli::add_tools_to_manifest;
                    add_tools_to_manifest(
                        &output_dir,
                        target_space,
                        query,
                        include,
                        exclude,
                        file,
                        exclude_dependencies,
                    )
                    .await?
                }
                _ => {
                    log::error!("Unknown API: {}", api);
                    return Err(eyre::eyre!(
                        "Unknown API '{}'. Supported: objects, workflows, spaces, agents, tools",
                        api
                    ));
                }
            };

            log::info!("✓ Added {} item(s)", count);
        }
        Commands::Togo {
            input_dir,
            managed,
            space,
            api,
        } => {
            log::info!(
                "Creating to-go bundle from: {}, managed: {}",
                input_dir.bright_black(),
                managed.cyan()
            );

            if let Some(spaces) = &space {
                log::info!("Filtering to space(s): {}", spaces.join(", ").cyan());
            }

            if let Some(apis) = &api {
                log::info!("Filtering to API(s): {}", apis.join(", ").cyan());
            }

            // Create bundle directory
            let bundle_dir = std::path::Path::new(&input_dir).join("bundle");
            std::fs::create_dir_all(&bundle_dir)?;
            log::info!("Bundle directory: {}", bundle_dir.display());

            // Bundle saved objects (now creates per-space bundles)
            let saved_objects_file = bundle_dir.join("saved_objects.ndjson");
            match bundle_to_ndjson(
                &input_dir,
                &saved_objects_file,
                managed,
                space.as_deref(),
                api.as_deref(),
            )
            .await
            {
                Ok(count) => {
                    log::info!("✓ Bundled {} saved object(s)", count);
                }
                Err(e) => {
                    log::error!("Bundle failed: {}", e);
                    return Err(e);
                }
            }

            log::info!("✓ Bundle created at {}", bundle_dir.display());
        }
        Commands::Migrate {
            project_dir,
            backup,
        } => {
            log::info!(
                "Migrating project to multi-space structure: {}",
                project_dir.bright_black()
            );

            match migrate_to_multispace_unified(&project_dir, backup, Some(&cli.env)).await? {
                MigrationResult::MigratedWithBackup(backup_path) => {
                    let target_space = std::env::var("kibana_space")
                        .or_else(|_| std::env::var("KIBANA_SPACE"))
                        .unwrap_or_else(|_| "default".to_string());
                    log::info!("✓ Migration completed successfully!");
                    log::info!(
                        "  New manifest: {}",
                        format!(
                            "{}/{}/manifest/saved_objects.json",
                            project_dir, target_space
                        )
                        .green()
                    );
                    log::info!(
                        "  Backup saved: {}",
                        backup_path.display().to_string().cyan()
                    );
                }
                MigrationResult::MigratedWithoutBackup => {
                    let target_space = std::env::var("kibana_space")
                        .or_else(|_| std::env::var("KIBANA_SPACE"))
                        .unwrap_or_else(|_| "default".to_string());
                    log::info!("✓ Migration completed successfully!");
                    log::info!(
                        "  New manifest: {}",
                        format!(
                            "{}/{}/manifest/saved_objects.json",
                            project_dir, target_space
                        )
                        .green()
                    );
                    log::info!("  Old files removed (no backup)");
                }
                MigrationResult::NoLegacyManifest => {
                    log::warn!("No legacy structure found in {}", project_dir);
                    log::info!("Nothing to migrate.");
                }
                MigrationResult::AlreadyMigrated => {
                    let target_space = std::env::var("kibana_space")
                        .or_else(|_| std::env::var("KIBANA_SPACE"))
                        .unwrap_or_else(|_| "default".to_string());
                    log::info!("✓ Project is already using multi-space structure!");
                    log::info!("  {}/manifest/ already exists", target_space);
                }
            }
        }
    }

    Ok(())
}
