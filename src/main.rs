use clap::{Parser, Subcommand, builder::styling};
use eyre::Result;
use kibana_object_manager::{
    cli::{
        add_objects_to_manifest, bundle_to_ndjson, init_from_export, load_kibana_client,
        pull_saved_objects, push_saved_objects,
    },
    migration::{MigrationResult, migrate_manifest},
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
    Pull {
        /// Project directory containing manifest (default: current directory)
        #[arg(default_value = ".")]
        output_dir: String,

        /// Kibana space to pull from (overrides KIBANA_SPACE env var)
        #[arg(long)]
        space: Option<String>,
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
    Push {
        /// Project directory containing objects to upload
        #[arg(default_value = ".")]
        input_dir: String,

        /// Make objects read-only in Kibana UI (managed: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        managed: bool,

        /// Kibana space to push to (overrides KIBANA_SPACE env var)
        #[arg(long)]
        space: Option<String>,
    },

    /// Add objects to an existing manifest
    ///
    /// Add objects by ID or merge from an export file into the manifest.
    ///
    /// Examples:
    ///   kibob add . --objects "dashboard=abc123,visualization=xyz789"
    ///   kibob add . --file new-export.ndjson
    Add {
        /// Project directory with existing manifest
        #[arg(default_value = ".")]
        output_dir: String,

        /// Comma-separated list of "type=id" objects to add
        #[arg(short, long, conflicts_with = "file")]
        objects: Option<Vec<String>>,

        /// Export NDJSON file to merge into manifest
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Bundle objects into distributable NDJSON files
    ///
    /// Creates a bundle/ directory with NDJSON files for each API:
    /// - bundle/saved_objects.ndjson - Saved objects
    /// - bundle/spaces.ndjson - Spaces (if manifest/spaces.yml exists)
    ///
    /// The bundle directory can be easily zipped for distribution.
    ///
    /// Example:
    ///   kibob togo ./my-dashboards
    ///   zip -r dashboards.zip my-dashboards/bundle/
    Togo {
        /// Project directory containing objects to bundle
        #[arg(default_value = ".")]
        input_dir: String,

        /// Set managed flag in bundled objects
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        managed: bool,
    },

    /// Migrate legacy manifest.json to new format
    ///
    /// Converts old manifest.json to new manifest/saved_objects.json structure.
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
    dotenvy::from_filename(&cli.env)?;

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

            match load_kibana_client() {
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
        Commands::Pull { output_dir, space } => {
            log::info!("Pulling objects to: {}", output_dir.bright_black());

            // Override KIBANA_SPACE if --space flag is provided
            if let Some(ref space_name) = space {
                unsafe {
                    std::env::set_var("KIBANA_SPACE", space_name);
                }
                log::info!("Using space from --space flag: {}", space_name.cyan());
            }

            match pull_saved_objects(&output_dir).await {
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

            // Override KIBANA_SPACE if --space flag is provided
            if let Some(ref space_name) = space {
                unsafe {
                    std::env::set_var("KIBANA_SPACE", space_name);
                }
                log::info!("Using space from --space flag: {}", space_name.cyan());
            }

            match push_saved_objects(&input_dir, managed).await {
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
            output_dir,
            objects,
            file,
        } => {
            log::info!("Adding objects to {}", output_dir.bright_black());

            match add_objects_to_manifest(&output_dir, objects, file).await {
                Ok(count) => {
                    log::info!("✓ Added {} object(s)", count);
                }
                Err(e) => {
                    log::error!("Add failed: {}", e);
                    return Err(e);
                }
            }
        }
        Commands::Togo { input_dir, managed } => {
            log::info!(
                "Creating to-go bundle from: {}, managed: {}",
                input_dir.bright_black(),
                managed.cyan()
            );

            // Create bundle directory
            let bundle_dir = std::path::Path::new(&input_dir).join("bundle");
            std::fs::create_dir_all(&bundle_dir)?;
            log::info!("Bundle directory: {}", bundle_dir.display());

            // Bundle saved objects
            let saved_objects_file = bundle_dir.join("saved_objects.ndjson");
            match bundle_to_ndjson(&input_dir, &saved_objects_file, managed).await {
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
            log::info!("Migrating manifest in: {}", project_dir.bright_black());

            match migrate_manifest(&project_dir, backup)? {
                MigrationResult::MigratedWithBackup(backup_path) => {
                    log::info!("✓ Migration completed successfully!");
                    log::info!(
                        "  New manifest: {}",
                        format!("{}/manifest/saved_objects.json", project_dir).green()
                    );
                    log::info!(
                        "  Backup saved: {}",
                        backup_path.display().to_string().cyan()
                    );
                }
                MigrationResult::MigratedWithoutBackup => {
                    log::info!("✓ Migration completed successfully!");
                    log::info!(
                        "  New manifest: {}",
                        format!("{}/manifest/saved_objects.json", project_dir).green()
                    );
                    log::info!("  Old manifest removed (no backup)");
                }
                MigrationResult::NoLegacyManifest => {
                    log::warn!("No legacy manifest.json found in {}", project_dir);
                    log::info!("Nothing to migrate.");
                }
                MigrationResult::AlreadyMigrated => {
                    log::info!("Already migrated!");
                    log::info!("  manifest/saved_objects.json already exists");
                }
            }
        }
    }

    Ok(())
}
