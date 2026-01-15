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

/// Kibana Object Manager: --{kibob}-> is the Git-flavored side dish to prepare Kibana saved objects for version control
#[derive(Parser)]
#[command(name = "kibob", version, styles = STYLES)]
struct Cli {
    /// The dotenv file to source credentials from
    #[arg(short, long, global = true, default_value = ".env")]
    env: String,

    /// More verbose logging and retention of temporary files
    #[arg(long, global = true)]
    debug: bool,

    /// Command to execute
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Slice up an export.ndjson into objects files and create a manifest.json
    Init {
        /// An NDJSON file or directory with an export.ndjson to build a manifest file from
        #[arg(default_value = "export.ndjson")]
        export: String,

        /// The manifest file to generate
        #[arg(default_value = "manifest.json")]
        manifest: String,
    },

    /// Test authorization to a Kibana remote
    Auth,

    /// Fetch saved objects from a Kibana remote
    Pull {
        /// Directory to save exported objects to. Must contain a manifest.json file.
        #[arg(default_value = ".")]
        output_dir: String,
    },

    /// Update saved objects in a Kibana remote
    Push {
        /// A directory containing the manifest.json file to import
        #[arg(default_value = ".")]
        input_dir: String,

        /// Set "managed: false" to allow direct editing in Kibana.
        /// Use --no-managed to disable management.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        managed: bool,
    },

    /// Add saved objects to the manifest
    Add {
        /// Directory to save the exported objects to. Must contain a manifest.json file.
        #[arg(default_value = ".")]
        output_dir: String,

        /// Comma-separated list of "type=uuid" objects to export from Kibana
        #[arg(short, long, conflicts_with = "file")]
        objects: Option<Vec<String>>,

        /// Filename of an export.ndjson to merge into existing manifest
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Order your Kibana objects to go! (bundle an NDJSON for distribution)
    Togo {
        /// Directory containing the objects to bundle
        #[arg(default_value = ".")]
        input_dir: String,

        /// Set "managed: false" to allow direct editing in Kibana.
        /// Use --no-managed to disable management.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        managed: bool,
    },

    /// Migrate legacy manifest.json to new manifest/ directory structure
    Migrate {
        /// Project directory containing manifest.json
        #[arg(default_value = ".")]
        project_dir: String,

        /// Keep a backup of the old manifest.json file
        #[arg(short, long, default_value_t = true)]
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

    log::info!("Kibana Object Manager - Phase 1 (ETL Framework)");

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
        Commands::Pull { output_dir } => {
            log::info!("Pulling objects to: {}", output_dir.bright_black());

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
        Commands::Push { input_dir, managed } => {
            log::info!(
                "Pushing {} objects from: {}",
                match managed {
                    true => "managed",
                    false => "unmanaged",
                }
                .cyan(),
                input_dir.bright_black(),
            );

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

            let output_file = std::path::Path::new(&input_dir).join("export.ndjson");

            match bundle_to_ndjson(&input_dir, &output_file, managed).await {
                Ok(count) => {
                    log::info!("✓ Bundled {} object(s) to {}", count, output_file.display());
                }
                Err(e) => {
                    log::error!("Bundle failed: {}", e);
                    return Err(e);
                }
            }
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
