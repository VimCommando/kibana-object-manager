use clap::{Parser, Subcommand, builder::styling};
use kibana_object_manager::KibanaObjectManagerBuilder;
use owo_colors::OwoColorize;
use std::{error::Error, path::PathBuf};

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
        manifest_file: String,
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

        /// Keep the temporary files and directories
        #[arg(short, long, default_value_t = true)]
        clean: bool,

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
        #[arg(short, long)]
        objects: Option<String>,

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

fn main() -> Result<(), Box<dyn Error>> {
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
        Commands::Init {
            export,
            manifest_file,
        } => {
            log::info!(
                "Initializing {} and building manifest {}",
                export.bright_black(),
                manifest_file.bright_black()
            );
            let kibob = kibob
                .export_path(PathBuf::from(export))
                .manifest_file(PathBuf::from(manifest_file))
                .build()?;
            kibob.initialize()?;
            log::info!("Initialization complete");
        }
        Commands::Auth => {
            let kibob = kibob.build()?;
            log::info!("Testing authorization to {}", kibob.url().bright_blue());
            match kibob.test_authorization() {
                Ok(msg) => log::info!("Authorization successful - {msg}"),
                Err(e) => log::error!("{}", e),
            }
        }
        Commands::Pull { output_dir } => {
            log::info!("Exporting objects to: {}", output_dir.bright_black());
            let kibob = kibob.export_path(PathBuf::from(output_dir)).build()?;
            log::info!("Pulling objects from: {}", kibob.url().bright_blue());
            match kibob.pull() {
                Ok(msg) => log::info!("Pull successful - {msg}"),
                Err(e) => log::error!("{}", e),
            }
        }
        Commands::Push {
            input_dir,
            clean,
            managed,
        } => {
            log::info!(
                "Pushing objects from: {}, clean: {}, managed: {}",
                input_dir,
                clean,
                managed
            );
        }
        Commands::Add {
            output_dir,
            objects,
            file,
        } => {
            log::info!(
                "Adding objects to: {}, objects: {:?}, file: {:?}",
                output_dir,
                objects,
                file
            );
        }
        Commands::Togo { input_dir, managed } => {
            log::info!(
                "Creating to-go bundle from: {}, managed: {}",
                input_dir,
                managed
            );
        }
    }
    Ok(())
}
