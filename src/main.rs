use clap::{Parser, Subcommand, builder::styling};
use eyre::Result;
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
                "Initializing {} and building manifest {}",
                export.bright_black(),
                manifest.bright_black()
            );
            log::warn!("Init command - Phase 2 implementation");
        }
        Commands::Auth => {
            log::info!("Testing authorization");
            log::warn!("Auth command - Phase 2 implementation");
        }
        Commands::Pull { output_dir } => {
            log::info!("Pulling objects to: {}", output_dir.bright_black());
            log::warn!("Pull command - Phase 2 implementation");
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
            log::warn!("Push command - Phase 2 implementation");
        }
        Commands::Add {
            output_dir,
            objects: _,
            file: _,
        } => {
            log::info!("Adding objects to {}", output_dir.bright_black());
            log::warn!("Add command - Phase 2 implementation");
        }
        Commands::Togo { input_dir, managed } => {
            log::info!(
                "Creating to-go bundle from: {}, managed: {}",
                input_dir.bright_black(),
                managed.cyan()
            );
            log::warn!("Togo command - Phase 2 implementation");
        }
    }

    Ok(())
}
