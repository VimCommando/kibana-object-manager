use clap::{ArgGroup, Args, Parser, Subcommand, builder::styling};
use eyre::Result;
use kibana_object_manager::{
    cli::{
        StandaloneExportSelection, add_objects_to_manifest, bundle_to_ndjson,
        export_standalone_resources, import_standalone_resources, init_from_export,
        load_kibana_client, pull_saved_objects, push_saved_objects,
        standalone_export_failure_report, version_warning_message,
    },
    migration::{MigrationResult, migrate_to_multispace_unified},
    standalone::{ResourceBatchReport, ResourceFamily, ResourceOperation, ResourceOutcomeStatus},
};
use owo_colors::OwoColorize;
use std::fmt;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use tracing::field::{Field, Visit};
use tracing_subscriber::{
    field::RecordFields,
    fmt::{FormatFields, format::Writer},
};

fn init_logging(filter: &str) {
    let _ = tracing_log::LogTracer::init();
    let use_ansi = std::env::var_os("NO_COLOR").is_none()
        && (std::io::stderr().is_terminal()
            || std::env::var_os("FORCE_COLOR").is_some()
            || std::env::var_os("CLICOLOR_FORCE").is_some());
    let _ = tracing_subscriber::fmt()
        .fmt_fields(AnsiPassthroughFields)
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(false)
        .with_ansi(use_ansi)
        .try_init();
}

#[derive(Debug, Clone, Copy)]
struct AnsiPassthroughFields;

impl<'writer> FormatFields<'writer> for AnsiPassthroughFields {
    fn format_fields<R: RecordFields>(&self, writer: Writer<'writer>, fields: R) -> fmt::Result {
        let mut visitor = AnsiPassthroughVisitor {
            writer,
            is_empty: true,
            result: Ok(()),
        };
        fields.record(&mut visitor);
        visitor.result
    }
}

struct AnsiPassthroughVisitor<'writer> {
    writer: Writer<'writer>,
    is_empty: bool,
    result: fmt::Result,
}

impl AnsiPassthroughVisitor<'_> {
    fn record_value(&mut self, field: &Field, value: impl FnOnce(&mut Writer<'_>) -> fmt::Result) {
        if field.name().starts_with("log.") {
            return;
        }

        if self.result.is_err() {
            return;
        }

        self.result = (|| {
            if !self.is_empty {
                write!(self.writer, " ")?;
            }

            if field.name() != "message" {
                write!(self.writer, "{}=", field.name())?;
            }

            value(&mut self.writer)?;
            self.is_empty = false;
            Ok(())
        })();
    }
}

impl Visit for AnsiPassthroughVisitor<'_> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.record_value(field, |writer| write!(writer, "{value:?}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_value(field, |writer| write!(writer, "{value}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_value(field, |writer| write!(writer, "{value:?}"));
    }
}

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
#[derive(Debug, Parser)]
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

#[derive(Debug, Subcommand)]
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

    /// Import manifest-free artifacts into one Kibana API family
    ///
    /// Standalone imports create or update exactly the resources found at the
    /// source. They do not read manifests, expand dependencies, or prune Kibana.
    Import {
        #[command(subcommand)]
        resource: ImportResources,
    },

    /// Export explicitly selected resources without creating a project manifest
    Export {
        #[command(subcommand)]
        resource: ExportResources,
    },

    /// Pull (fetch) saved objects from Kibana to local files
    ///
    /// Downloads objects specified in the manifest from Kibana and saves them locally.
    /// Objects are organized by type in the objects/ directory.
    ///
    /// Examples:
    ///   kibob pull ./my-dashboards
    ///   kibob pull ./my-dashboards --space esdiag
    ///   kibob pull ./my-dashboards --api tools,agents
    ///   kibob pull ./my-dashboards --force            # Bypass version checks (warning)
    Pull {
        /// Project directory containing manifest (default: current directory)
        #[arg(default_value = ".")]
        output_dir: String,

        /// Kibana space(s) to pull from (comma-separated, overrides KIBANA_SPACE env var)
        #[arg(long, value_delimiter = ',')]
        space: Option<Vec<String>>,

        /// Comma-separated APIs to pull with min versions:
        /// saved_objects (8.0+), spaces (8.0+), agents (9.2+ tech preview), tools (9.2+ tech preview), workflows (9.3+ tech preview), skills (9.4+ experimental)
        #[arg(long, value_delimiter = ',')]
        api: Option<Vec<String>>,

        /// Bypass version compatibility checks and attempt API calls anyway (prints warning)
        #[arg(long)]
        force: bool,
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
    ///   kibob push . --force           # Bypass version checks (warning)
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

        /// Comma-separated APIs to push with min versions:
        /// saved_objects (8.0+), spaces (8.0+), agents (9.2+ tech preview), tools (9.2+ tech preview), workflows (9.3+ tech preview), skills (9.4+ experimental)
        #[arg(long, value_delimiter = ',')]
        api: Option<Vec<String>>,

        /// Bypass version compatibility checks and attempt API calls anyway (prints warning)
        #[arg(long)]
        force: bool,
    },

    /// Add items to an existing manifest
    ///
    /// Discovers and adds items via search API or reads from a file.
    /// Supports: objects, workflows, spaces, agents, tools, skills
    /// Skills are experimental as of Kibana 9.4 and are stored as
    /// skills/{skill-directory}/SKILL.md with YAML frontmatter fields:
    /// id, name, description, tool_ids, and experimental.
    /// Referenced content is projected from all files under the skill directory.
    /// Local experimental metadata is omitted from API create/update bodies.
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
    ///   kibob add skill threat-hunting                 # Fetch a skill by ID into the current directory
    ///   kibob add skills . --query "threat-hunting"    # Fetch a skill by ID
    ///   kibob add skills . --include "^triage"         # Include skills matching pattern
    ///   kibob add objects . --objects "dashboard=abc"  # Legacy: add specific objects by ID
    ///   kibob add workflows . --force                  # Bypass version checks (warning)
    Add {
        /// API to add to:
        /// objects (8.0+), spaces (8.0+), agents (9.2+ tech preview), tools (9.2+ tech preview), workflows (9.3+ tech preview), skills (9.4+ experimental)
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

        /// Exclude dependencies of added items (agents, tools, workflows, skills)
        #[arg(long)]
        exclude_dependencies: bool,

        /// Bypass version compatibility checks and attempt API calls anyway (prints warning)
        #[arg(long)]
        force: bool,
    },

    /// Bundle objects into distributable NDJSON files
    ///
    /// Creates a bundle/ directory with NDJSON files for each API:
    /// - bundle/{space_id}/saved_objects.ndjson - Saved objects per space
    /// - bundle/{space_id}/workflows.ndjson - Workflows per space
    /// - bundle/{space_id}/agents.ndjson - Agents per space
    /// - bundle/{space_id}/tools.ndjson - Tools per space
    /// - bundle/{space_id}/skills.ndjson - Skills per space
    /// - bundle/spaces.ndjson - Spaces (if manifest/spaces.yml exists)
    ///
    /// Skill JSON is generated from skills/{skill-directory}/SKILL.md and
    /// referenced markdown files only when bundling.
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

        /// Comma-separated list of APIs to bundle (e.g., "saved_objects,workflows,agents,tools,skills,spaces")
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

#[derive(Debug, Subcommand)]
enum ImportResources {
    /// Import one Skill directory or immediate-child Skill directories
    Skills(ImportArgs),
    /// Import one Tool JSON file or immediate JSON files in a directory
    Tools(ImportArgs),
    /// Import one Agent JSON file or immediate JSON files in a directory
    Agents(ImportArgs),
    /// Import one Workflow JSON file or immediate JSON files in a directory
    Workflows(ImportArgs),
}

impl ImportResources {
    fn into_parts(self) -> (ResourceFamily, ImportArgs) {
        match self {
            Self::Skills(args) => (ResourceFamily::Skills, args),
            Self::Tools(args) => (ResourceFamily::Tools, args),
            Self::Agents(args) => (ResourceFamily::Agents, args),
            Self::Workflows(args) => (ResourceFamily::Workflows, args),
        }
    }
}

#[derive(Args, Debug)]
struct ImportArgs {
    /// Skill directory, JSON file, or immediate-child collection root
    source: PathBuf,

    /// Kibana space ID (defaults to KIBANA_SPACE or default)
    #[arg(long)]
    space: Option<String>,

    /// Bypass the selected API family's Kibana version check
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Subcommand)]
enum ExportResources {
    /// Export Skills as immediate child directories containing SKILL.md
    Skills(ExportArgs),
    /// Export Tools as immediate JSON files
    Tools(ExportArgs),
    /// Export Agents as immediate JSON files
    Agents(ExportArgs),
    /// Export Workflows as immediate JSON files
    Workflows(ExportArgs),
}

impl ExportResources {
    fn into_parts(self) -> (ResourceFamily, ExportArgs) {
        match self {
            Self::Skills(args) => (ResourceFamily::Skills, args),
            Self::Tools(args) => (ResourceFamily::Tools, args),
            Self::Agents(args) => (ResourceFamily::Agents, args),
            Self::Workflows(args) => (ResourceFamily::Workflows, args),
        }
    }
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("selection")
        .required(true)
        .multiple(false)
        .args(["id", "all"])
))]
struct ExportArgs {
    /// Destination collection root
    destination: PathBuf,

    /// Resource ID to export; repeat for multiple resources
    #[arg(long)]
    id: Vec<String>,

    /// Export every user-created resource in the selected family
    #[arg(long)]
    all: bool,

    /// Kibana space ID (defaults to KIBANA_SPACE or default)
    #[arg(long)]
    space: Option<String>,

    /// Replace only selected outputs that already exist
    #[arg(long)]
    overwrite: bool,

    /// Bypass the selected API family's Kibana version check
    #[arg(long)]
    force: bool,
}

impl ExportArgs {
    fn selection(&self) -> StandaloneExportSelection {
        if self.all {
            StandaloneExportSelection::All
        } else {
            StandaloneExportSelection::Ids(self.id.clone())
        }
    }
}

fn log_resource_batch_report(report: &ResourceBatchReport, default_applied_label: &str) {
    for outcome in report.outcomes() {
        match outcome.status() {
            ResourceOutcomeStatus::Applied => {
                let operation = match outcome.operation() {
                    Some(ResourceOperation::Create) => "created",
                    Some(ResourceOperation::Update) => "updated",
                    None => default_applied_label,
                };
                log::info!("✓ {} {} {}", operation, outcome.family(), outcome.id());
            }
            ResourceOutcomeStatus::Skipped => log::warn!(
                "- skipped {} {}: {}",
                outcome.family(),
                outcome.id(),
                outcome.detail().unwrap_or("no detail")
            ),
            ResourceOutcomeStatus::Failed => log::error!(
                "✗ failed {} {}: {}",
                outcome.family(),
                outcome.id(),
                outcome.detail().unwrap_or("no detail")
            ),
        }
    }

    log::info!("{}", resource_batch_summary(report, default_applied_label));
}

fn resource_batch_summary(report: &ResourceBatchReport, applied_label: &str) -> String {
    let counts = report.counts();
    format!(
        "attempted: {}, {}: {}, skipped: {}, failed: {}",
        counts.attempted, applied_label, counts.applied, counts.skipped, counts.failed
    )
}

fn resolve_env_path(env: &str) -> PathBuf {
    let env_path = Path::new(env);

    if env == ".env" || env.contains(std::path::MAIN_SEPARATOR) || env.starts_with('.') {
        return env_path.to_path_buf();
    }

    if env_path.exists() {
        return env_path.to_path_buf();
    }

    PathBuf::from(format!(".env.{}", env))
}

fn is_skill_id_shortcut_arg(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let resolved_env = resolve_env_path(&cli.env);
    if let Err(e) = dotenvy::from_filename(&resolved_env) {
        log::warn!(
            "Failed to load environment variables from {}: {}",
            resolved_env.display(),
            e
        );
    }

    let log_level = match cli.debug {
        true => "debug",
        false => "info",
    };
    let filter = std::env::var("LOG_LEVEL").unwrap_or_else(|_| log_level.to_string());
    init_logging(&filter);

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
                        return Err(e.into());
                    }
                },
                Err(e) => {
                    log::error!("✗ Failed to create Kibana client: {}", e);
                    return Err(e);
                }
            }
        }
        Commands::Import { resource } => {
            let (family, args) = resource.into_parts();
            log::info!(
                "Importing standalone {} from {}",
                family,
                args.source.display().to_string().bright_black()
            );

            match import_standalone_resources(
                family,
                &args.source,
                args.space.as_deref(),
                args.force,
            )
            .await
            {
                Ok(report) => {
                    log_resource_batch_report(&report, "applied");
                    if report.has_failures() {
                        return Err(eyre::eyre!(
                            "standalone {} import failed for {} of {} resources",
                            family,
                            report.counts().failed,
                            report.counts().attempted
                        ));
                    }
                }
                Err(error) => {
                    if let Some(message) = version_warning_message(&error) {
                        log::warn!("{message}");
                        std::process::exit(2);
                    }
                    return Err(error);
                }
            }
        }
        Commands::Export { resource } => {
            let (family, args) = resource.into_parts();
            log::info!(
                "Exporting standalone {} to {}",
                family,
                args.destination.display().to_string().bright_black()
            );

            match export_standalone_resources(
                family,
                &args.destination,
                args.selection(),
                args.space.as_deref(),
                args.overwrite,
                args.force,
            )
            .await
            {
                Ok(report) => {
                    log_resource_batch_report(&report, "written");
                    if report.has_failures() {
                        return Err(eyre::eyre!(
                            "standalone {} export failed for {} of {} resources",
                            family,
                            report.counts().failed,
                            report.counts().attempted
                        ));
                    }
                }
                Err(error) => {
                    if let Some(message) = version_warning_message(&error) {
                        log::warn!("{message}");
                        std::process::exit(2);
                    }
                    if let Some(report) = standalone_export_failure_report(&error) {
                        log_resource_batch_report(report, "written");
                    }
                    return Err(error);
                }
            }
        }
        Commands::Pull {
            output_dir,
            space,
            api,
            force,
        } => {
            log::info!("Pulling objects to: {}", output_dir.bright_black());

            if let Some(spaces) = &space {
                log::info!("Filtering to space(s): {}", spaces.join(", ").cyan());
            }

            if let Some(apis) = &api {
                log::info!("Filtering to API(s): {}", apis.join(", ").cyan());
            }

            match pull_saved_objects(&output_dir, space.as_deref(), api.as_deref(), force).await {
                Ok(count) => {
                    log::info!("✓ Successfully pulled {} object(s)", count);
                }
                Err(e) => {
                    if let Some(message) = version_warning_message(&e) {
                        log::warn!("{}", message);
                        std::process::exit(2);
                    }
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
            force,
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

            match push_saved_objects(&input_dir, managed, space.as_deref(), api.as_deref(), force)
                .await
            {
                Ok(count) => {
                    log::info!("✓ Successfully pushed {} object(s)", count);
                }
                Err(e) => {
                    if let Some(message) = version_warning_message(&e) {
                        log::warn!("{}", message);
                        std::process::exit(2);
                    }
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
            force,
        } => {
            log::info!("Adding {} to {}", api.cyan(), output_dir.bright_black());

            // Route to appropriate handler based on API type
            let count_result: Result<usize> = match api.as_str() {
                "objects" => {
                    // Legacy objects support: --objects flag or --file
                    let target_space = space
                        .as_ref()
                        .and_then(|s| s.first())
                        .map(|s| s.as_str())
                        .unwrap_or("default");
                    log::info!("Using space: {}", target_space.cyan());
                    add_objects_to_manifest(&output_dir, target_space, objects, file, force).await
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
                        force,
                    )
                    .await
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
                        force,
                    )
                    .await
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
                        force,
                    )
                    .await
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
                        force,
                    )
                    .await
                }
                "skills" | "skill" => {
                    // Skills support: --query exact ID, --include, --exclude, or --file
                    let singular_id_shortcut = api == "skill"
                        && query.is_none()
                        && file.is_none()
                        && output_dir != "."
                        && is_skill_id_shortcut_arg(&output_dir)
                        && !Path::new(&output_dir).exists();
                    if api == "skill" && query.is_none() && file.is_none() && !singular_id_shortcut
                    {
                        return Err(eyre::eyre!(
                            "kibob add skill requires a skill id. For an existing project directory, use: kibob add skill <project_dir> --query <skill-id>. If the skill id matches a local path, use: kibob add skill . --query <skill-id>"
                        ));
                    }
                    let effective_output_dir;
                    let effective_query;
                    let (project_dir, query) = if singular_id_shortcut {
                        effective_output_dir = ".".to_string();
                        effective_query = Some(output_dir.clone());
                        (effective_output_dir.as_str(), effective_query)
                    } else {
                        (output_dir.as_str(), query)
                    };

                    let target_space = space
                        .as_ref()
                        .and_then(|s| s.first())
                        .map(|s| s.as_str())
                        .unwrap_or("default");
                    log::info!("Using space: {}", target_space.cyan());
                    use kibana_object_manager::cli::add_skills_to_manifest;
                    add_skills_to_manifest(
                        project_dir,
                        target_space,
                        query,
                        include,
                        exclude,
                        file,
                        exclude_dependencies,
                        force,
                    )
                    .await
                }
                _ => {
                    log::error!("Unknown API: {}", api);
                    return Err(eyre::eyre!(
                        "Unknown API '{}'. Supported: objects, workflows, spaces, agents, tools, skills",
                        api
                    ));
                }
            };

            match count_result {
                Ok(count) => log::info!("✓ Added {} item(s)", count),
                Err(e) => {
                    if let Some(message) = version_warning_message(&e) {
                        log::warn!("{}", message);
                        std::process::exit(2);
                    }
                    return Err(e);
                }
            }
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

            match migrate_to_multispace_unified(&project_dir, backup, Some(&resolved_env)).await? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn resource_batch_summary_uses_the_requested_success_label() {
        let report = ResourceBatchReport::new(Vec::new());

        assert_eq!(
            resource_batch_summary(&report, "written"),
            "attempted: 0, written: 0, skipped: 0, failed: 0"
        );
        assert_eq!(
            resource_batch_summary(&report, "applied"),
            "attempted: 0, applied: 0, skipped: 0, failed: 0"
        );
    }

    #[test]
    fn parses_every_standalone_import_family() {
        for (name, expected) in [
            ("skills", ResourceFamily::Skills),
            ("tools", ResourceFamily::Tools),
            ("agents", ResourceFamily::Agents),
            ("workflows", ResourceFamily::Workflows),
        ] {
            let cli = Cli::try_parse_from([
                "kibob",
                "import",
                name,
                "artifacts",
                "--space",
                "security",
                "--force",
            ])
            .unwrap();
            let Commands::Import { resource } = cli.command else {
                panic!("expected import command");
            };
            let (family, args) = resource.into_parts();

            assert_eq!(family, expected);
            assert_eq!(args.source, PathBuf::from("artifacts"));
            assert_eq!(args.space.as_deref(), Some("security"));
            assert!(args.force);
        }
    }

    #[test]
    fn import_help_lists_only_plural_supported_families() {
        let error = Cli::try_parse_from(["kibob", "import", "--help"]).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        let help = error.to_string();
        for family in ["skills", "tools", "agents", "workflows"] {
            assert!(help.contains(family));
        }
    }

    #[test]
    fn rejects_unsupported_and_singular_import_families() {
        for family in ["saved-objects", "skill", "tool", "agent", "workflow"] {
            let error = Cli::try_parse_from(["kibob", "import", family, "artifacts"]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
        }
    }

    #[test]
    fn existing_push_syntax_remains_project_oriented() {
        let cli = Cli::try_parse_from([
            "kibob",
            "push",
            "project",
            "--managed",
            "false",
            "--space",
            "security",
            "--api",
            "tools",
            "--force",
        ])
        .unwrap();

        let Commands::Push {
            input_dir,
            managed,
            space,
            api,
            force,
        } = cli.command
        else {
            panic!("expected push command");
        };
        assert_eq!(input_dir, "project");
        assert!(!managed);
        assert_eq!(space, Some(vec!["security".to_string()]));
        assert_eq!(api, Some(vec!["tools".to_string()]));
        assert!(force);
    }

    #[test]
    fn parses_every_standalone_export_family_and_repeated_ids() {
        for (name, expected) in [
            ("skills", ResourceFamily::Skills),
            ("tools", ResourceFamily::Tools),
            ("agents", ResourceFamily::Agents),
            ("workflows", ResourceFamily::Workflows),
        ] {
            let cli = Cli::try_parse_from([
                "kibob",
                "export",
                name,
                "artifacts",
                "--id",
                "first",
                "--id",
                "second",
                "--space",
                "security",
                "--overwrite",
                "--force",
            ])
            .unwrap();
            let Commands::Export { resource } = cli.command else {
                panic!("expected export command");
            };
            let (family, args) = resource.into_parts();

            assert_eq!(family, expected);
            assert_eq!(args.destination, PathBuf::from("artifacts"));
            assert_eq!(
                args.selection(),
                StandaloneExportSelection::Ids(vec!["first".into(), "second".into()])
            );
            assert_eq!(args.space.as_deref(), Some("security"));
            assert!(args.overwrite);
            assert!(args.force);
        }
    }

    #[test]
    fn export_requires_exactly_one_selector_form() {
        let missing = Cli::try_parse_from(["kibob", "export", "tools", "artifacts"]).unwrap_err();
        assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);

        let conflicting = Cli::try_parse_from([
            "kibob",
            "export",
            "tools",
            "artifacts",
            "--id",
            "tool-a",
            "--all",
        ])
        .unwrap_err();
        assert_eq!(conflicting.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn rejects_unsupported_and_singular_export_families() {
        for family in ["saved-objects", "skill", "tool", "agent", "workflow"] {
            let error =
                Cli::try_parse_from(["kibob", "export", family, "artifacts", "--all"]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
        }
    }

    #[test]
    fn existing_pull_syntax_remains_project_oriented() {
        let cli = Cli::try_parse_from([
            "kibob", "pull", "project", "--space", "security", "--api", "tools", "--force",
        ])
        .unwrap();

        let Commands::Pull {
            output_dir,
            space,
            api,
            force,
        } = cli.command
        else {
            panic!("expected pull command");
        };
        assert_eq!(output_dir, "project");
        assert_eq!(space, Some(vec!["security".to_string()]));
        assert_eq!(api, Some(vec!["tools".to_string()]));
        assert!(force);
    }
}
