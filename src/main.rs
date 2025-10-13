use clap::{Parser, Subcommand, builder::styling};
use eyre::Result;
use kibana_object_manager::KibanaObjectManagerBuilder;
use owo_colors::OwoColorize;
use std::path::PathBuf;

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

        /// Set "managed: false" to allow direct editing in Kibana
        #[arg(short, long, default_value_t = true)]
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

        /// Set "managed: false" to allow direct editing in Kibana
        #[arg(short, long, default_value_t = true)]
        managed: bool,
    },
}

fn main() -> Result<()> {
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

    let kibob = KibanaObjectManagerBuilder::new(std::env::var("KIBANA_URL")?)
        .username(std::env::var("KIBANA_USERNAME").ok())
        .apikey(std::env::var("KIBANA_APIKEY").ok())
        .password(std::env::var("KIBANA_PASSWORD").ok());

    log::debug!("{:?}", kibob);
    match cli.command {
        Commands::Init { export, manifest } => {
            log::info!(
                "Initializing {} and building manifest {}",
                export.bright_black(),
                manifest.bright_black()
            );
            kibob
                .export_path(PathBuf::from(export))
                .manifest(&PathBuf::from(manifest))
                .build_initializer()?
                .initialize()?;
            log::info!("Initialization complete");
        }
        Commands::Auth => {
            let kibob = kibob.build_authorizer()?;
            log::info!("Testing authorization to {}", kibob.url().bright_blue());
            match kibob.authorize() {
                Ok(msg) => log::info!("Authorization successful - {msg}"),
                Err(e) => log::error!("{}", e),
            }
        }
        Commands::Pull { output_dir } => {
            log::info!("Exporting objects to: {}", output_dir.bright_black());
            let output_path = PathBuf::from(output_dir);
            let kibob = kibob
                .manifest(&output_path)
                .export_path(output_path)
                .build_exporter()?;
            log::info!("Pulling objects from: {}", kibob.url().bright_blue());
            match kibob.pull() {
                Ok(msg) => log::info!("Pull successful - {msg}"),
                Err(e) => log::error!("{}", e),
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
            let kibob = kibob
                .import_path(PathBuf::from(input_dir))
                .managed(managed)
                .build_importer()?;
            log::info!("Pushing objects to: {}", kibob.url().bright_blue());
            match kibob.push() {
                Ok(msg) => log::info!("Push successful: {msg}"),
                Err(e) => log::error!("{}", e),
            }
        }
        Commands::Add {
            output_dir,
            objects,
            file,
        } => {
            let output_path = PathBuf::from(output_dir);
            match (objects, file) {
                (Some(objects), None) => {
                    log::info!(
                        "Adding {} to {}",
                        "objects".cyan(),
                        output_path.display().bright_black(),
                    );
                    kibob
                        .manifest(&output_path)
                        .export_path(output_path.join("merge.ndjson"))
                        .export_list(objects)?
                        .build_kibana_merger()?
                        .read()?
                        .merge()?;
                }
                (None, Some(file)) => {
                    log::info!(
                        "Adding {} contents to {}",
                        file.bright_black(),
                        output_path.display().bright_black(),
                    );
                    kibob
                        .manifest(&output_path)
                        .export_path(output_path)
                        .merge_path(PathBuf::from(file))
                        .build_file_merger()?
                        .read()?
                        .merge()?;
                }
                _ => log::error!(
                    "Invalid {} arguments, must specify either {} or {}",
                    "Add".magenta(),
                    "--objects".cyan(),
                    "--file".cyan()
                ),
            }
        }
        Commands::Togo { input_dir, managed } => {
            log::info!(
                "Creating to-go bundle from: {}, managed: {}",
                input_dir.bright_black(),
                managed.cyan()
            );
            let kibob = kibob
                .managed(managed)
                .import_path(PathBuf::from(input_dir))
                .build_bundler()?;
            log::info!(
                "Creating to-go bundle from: {}",
                kibob.to_string().bright_black()
            );
            match kibob.bundle() {
                Ok(msg) => log::info!("To-go ready: {msg}"),
                Err(e) => log::error!("{}", e),
            }
        }
    }
    Ok(())
}
