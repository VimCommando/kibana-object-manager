//! Manifest migration utilities
//!
//! Provides utilities for migrating from the legacy Bash-based structure
//! to the new Rust-based directory structure.
//!
//! Legacy structure (Bash version):
//! ```text
//! project/
//!   ├── manifest.json                          (saved objects manifest)
//!   └── objects/                               (flat saved object files)
//!       ├── allocation-overview.dashboard.json (format: name.type.json)
//!       ├── data-summary.dashboard.json
//!       └── test-viz.visualization.json
//! ```
//!
//! New structure (Rust v0.1.0+):
//! ```text
//! project/
//!   ├── manifest/
//!   │   └── saved_objects.json  (saved objects manifest)
//!   └── objects/                (hierarchical saved object files)
//!       ├── dashboard/
//!       │   ├── allocation-overview.json
//!       │   └── data-summary.json
//!       └── visualization/
//!           └── test-viz.json
//! ```

use eyre::{Context, Result};
use std::path::{Path, PathBuf};

use crate::client::{Auth, KibanaClient};
use crate::kibana::saved_objects::SavedObjectsManifest;
use crate::kibana::spaces::{SpacesExtractor, SpacesManifest};
use crate::storage::transform_env_file;
use url::Url;

/// Migrate a legacy manifest.json file to the new manifest/ directory structure
///
/// This function:
/// 1. Reads the legacy `manifest.json` file
/// 2. Creates a `manifest/` directory if it doesn't exist
/// 3. Writes the manifest to `manifest/saved_objects.json`
/// 4. Migrates flat object files (`objects/type-id.json`) to hierarchical (`objects/type/id.json`)
/// 5. Optionally backs up or removes the old `manifest.json` file
///
/// # Example
/// ```no_run
/// use kibana_object_manager::migration::migrate_manifest;
///
/// # fn example() -> eyre::Result<()> {
/// // Migrate manifest.json in the current directory
/// migrate_manifest(".", true)?;
/// # Ok(())
/// # }
/// ```
pub fn migrate_manifest(
    project_dir: impl AsRef<Path>,
    backup_old: bool,
) -> Result<MigrationResult> {
    let project_dir = project_dir.as_ref();
    let old_manifest_path = project_dir.join("manifest.json");
    let new_manifest_dir = project_dir.join("manifest");
    let new_manifest_path = new_manifest_dir.join("saved_objects.json");

    // Check if old manifest exists
    if !old_manifest_path.exists() {
        return Ok(MigrationResult::NoLegacyManifest);
    }

    // Check if new manifest already exists
    if new_manifest_path.exists() {
        return Ok(MigrationResult::AlreadyMigrated);
    }

    log::info!(
        "Migrating manifest from {} to {}",
        old_manifest_path.display(),
        new_manifest_path.display()
    );

    // Read the old manifest
    let manifest = SavedObjectsManifest::read(&old_manifest_path).with_context(|| {
        format!(
            "Failed to read legacy manifest: {}",
            old_manifest_path.display()
        )
    })?;

    log::info!("Read legacy manifest with {} objects", manifest.count());

    // Create the manifest directory
    std::fs::create_dir_all(&new_manifest_dir).with_context(|| {
        format!(
            "Failed to create manifest directory: {}",
            new_manifest_dir.display()
        )
    })?;

    // Write the new manifest
    manifest.write(&new_manifest_path).with_context(|| {
        format!(
            "Failed to write new manifest: {}",
            new_manifest_path.display()
        )
    })?;

    log::info!("Wrote new manifest to {}", new_manifest_path.display());

    // Migrate object files from flat to hierarchical structure
    migrate_object_files(project_dir)?;

    // Handle the old manifest file
    if backup_old {
        let backup_path = project_dir.join("manifest.json.backup");
        std::fs::rename(&old_manifest_path, &backup_path).with_context(|| {
            format!("Failed to backup old manifest to {}", backup_path.display())
        })?;
        log::info!("Backed up old manifest to {}", backup_path.display());
        Ok(MigrationResult::MigratedWithBackup(backup_path))
    } else {
        std::fs::remove_file(&old_manifest_path).with_context(|| {
            format!(
                "Failed to remove old manifest: {}",
                old_manifest_path.display()
            )
        })?;
        log::info!("Removed old manifest");
        Ok(MigrationResult::MigratedWithoutBackup)
    }
}

/// Migrate object files from flat structure to hierarchical structure
///
/// Converts:
///   objects/object_name.type.json → objects/type/object_name.json
///   objects/my-dashboard.dashboard.json → objects/dashboard/my-dashboard.json
fn migrate_object_files(project_dir: &Path) -> Result<()> {
    let objects_dir = project_dir.join("objects");

    // If objects directory doesn't exist, nothing to migrate
    if !objects_dir.exists() {
        log::debug!("No objects directory found, skipping object file migration");
        return Ok(());
    }

    let mut migrated_count = 0;
    let mut already_hierarchical = 0;

    // Read all entries in objects directory
    for entry in std::fs::read_dir(&objects_dir).with_context(|| {
        format!(
            "Failed to read objects directory: {}",
            objects_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();

        // Skip subdirectories (already hierarchical)
        if path.is_dir() {
            already_hierarchical += 1;
            continue;
        }

        // Only process .json files
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        // Extract filename without extension
        let filename = match path.file_stem().and_then(|s| s.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Parse flat filename: "object_name.type" format
        // Look for the LAST dot to split name and type
        if let Some(dot_pos) = filename.rfind('.') {
            let obj_name = &filename[..dot_pos];
            let obj_type = &filename[dot_pos + 1..];

            // Create hierarchical path: type/object_name.json
            let type_dir = objects_dir.join(obj_type);
            let new_path = type_dir.join(format!("{}.json", obj_name));

            // Create type directory if it doesn't exist
            std::fs::create_dir_all(&type_dir)
                .with_context(|| format!("Failed to create directory: {}", type_dir.display()))?;

            // Move file to new location
            std::fs::rename(&path, &new_path).with_context(|| {
                format!(
                    "Failed to move {} to {}",
                    path.display(),
                    new_path.display()
                )
            })?;

            log::debug!(
                "Migrated object file: {} → {}",
                path.display(),
                new_path.display()
            );
            migrated_count += 1;
        } else {
            log::warn!(
                "Skipping file with unexpected name format (expected 'object_name.type.json'): {}",
                path.display()
            );
        }
    }

    if migrated_count > 0 {
        log::info!(
            "Migrated {} object file(s) from flat to hierarchical structure",
            migrated_count
        );
    } else if already_hierarchical > 0 {
        log::info!(
            "Object files already in hierarchical structure ({} subdirectories found)",
            already_hierarchical
        );
    } else {
        log::info!("No object files to migrate");
    }

    Ok(())
}

/// Check if a project directory needs migration
pub fn needs_migration(project_dir: impl AsRef<Path>) -> bool {
    let project_dir = project_dir.as_ref();
    let old_manifest = project_dir.join("manifest.json");
    let new_manifest = project_dir.join("manifest/saved_objects.json");

    old_manifest.exists() && !new_manifest.exists()
}

/// Attempt to load a SavedObjectsManifest from either the new or legacy location
///
/// This function provides backward compatibility by checking:
/// 1. First tries `manifest/saved_objects.json` (new location)
/// 2. Falls back to `manifest.json` (legacy location)
///
/// # Example
/// ```no_run
/// use kibana_object_manager::migration::load_saved_objects_manifest;
///
/// # fn example() -> eyre::Result<()> {
/// let manifest = load_saved_objects_manifest(".")?;
/// println!("Loaded {} objects", manifest.count());
/// # Ok(())
/// # }
/// ```
pub fn load_saved_objects_manifest(project_dir: impl AsRef<Path>) -> Result<SavedObjectsManifest> {
    let project_dir = project_dir.as_ref();
    let new_path = project_dir.join("manifest/saved_objects.json");
    let old_path = project_dir.join("manifest.json");

    if new_path.exists() {
        log::debug!("Loading manifest from new location: {}", new_path.display());
        SavedObjectsManifest::read(&new_path)
    } else if old_path.exists() {
        log::warn!(
            "Loading manifest from legacy location: {}. Consider running 'kibob migrate' to update.",
            old_path.display()
        );
        SavedObjectsManifest::read(&old_path)
    } else {
        eyre::bail!(
            "No saved objects manifest found at {} or {}",
            new_path.display(),
            old_path.display()
        )
    }
}

/// Result of a migration operation
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationResult {
    /// Migration completed successfully with backup of old file
    MigratedWithBackup(PathBuf),
    /// Migration completed successfully without backup
    MigratedWithoutBackup,
    /// No legacy manifest.json file found (nothing to migrate)
    NoLegacyManifest,
    /// Already migrated (manifest/saved_objects.json already exists)
    AlreadyMigrated,
}

impl std::fmt::Display for MigrationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationResult::MigratedWithBackup(path) => {
                write!(
                    f,
                    "Migration completed. Old manifest backed up to: {}",
                    path.display()
                )
            }
            MigrationResult::MigratedWithoutBackup => {
                write!(f, "Migration completed. Old manifest removed.")
            }
            MigrationResult::NoLegacyManifest => {
                write!(f, "No legacy manifest.json found. Nothing to migrate.")
            }
            MigrationResult::AlreadyMigrated => {
                write!(f, "Already migrated. manifest/saved_objects.json exists.")
            }
        }
    }
}

//
// Unified migration function
//

/// Check if project needs any migration
///
/// Returns true if:
/// - Legacy structure exists: `manifest.json` in root
/// - Old single-space structure exists: `manifest/saved_objects.json` in root manifest/
/// - New multi-space structure doesn't exist: `{space}/manifest/` directory doesn't exist
pub fn needs_migration_unified(project_dir: impl AsRef<Path>) -> bool {
    let project_dir = project_dir.as_ref();
    let legacy_manifest = project_dir.join("manifest.json");
    let old_manifest = project_dir.join("manifest/saved_objects.json");

    // Detect target space from environment (prefer lowercase kibana_space)
    let target_space = std::env::var("kibana_space")
        .or_else(|_| std::env::var("KIBANA_SPACE"))
        .unwrap_or_else(|_| "default".to_string());
    let new_default_manifest_dir = project_dir.join(&target_space).join("manifest");

    // If new structure exists, already migrated
    if new_default_manifest_dir.exists() {
        return false;
    }

    // If legacy or old single-space structure exists, needs migration
    legacy_manifest.exists() || old_manifest.exists()
}

/// Unified migration: Migrate directly from legacy structure to multi-space structure
///
/// This performs a single-step migration from either:
/// - Legacy Bash structure (manifest.json) → Multi-space structure
/// - Old v0.1.0 single-space (manifest/saved_objects.json) → Multi-space structure
///
/// The migration uses the `kibana_space` environment variable (if set) to determine
/// the target space directory. If not set, it defaults to `KIBANA_SPACE` or "default".
/// It also updates the .env file if provided.
pub async fn migrate_to_multispace_unified(
    project_dir: impl AsRef<Path>,
    backup_old: bool,
    env_path: Option<impl AsRef<Path>>,
) -> Result<MigrationResult> {
    let project_dir = project_dir.as_ref();

    if !needs_migration_unified(project_dir) {
        return Ok(MigrationResult::AlreadyMigrated);
    }

    // Detect target space from environment (prefer lowercase kibana_space)
    let target_space = std::env::var("kibana_space")
        .or_else(|_| std::env::var("KIBANA_SPACE"))
        .unwrap_or_else(|_| "default".to_string());

    if target_space != "default" {
        log::info!("Using space '{}' for migration target", target_space);
    }

    log::info!("Migrating project to multi-space structure...");
    log::info!("Target space: '{}'", target_space);

    let legacy_manifest_path = project_dir.join("manifest.json");
    let old_manifest_path = project_dir.join("manifest/saved_objects.json");
    let target_space_dir = project_dir.join(&target_space);
    let new_manifest_dir = target_space_dir.join("manifest");
    let new_manifest_path = new_manifest_dir.join("saved_objects.json");

    // Create target directories
    std::fs::create_dir_all(&new_manifest_dir)?;
    std::fs::create_dir_all(&target_space_dir)?;

    // Update .env file if provided
    if let Some(env_path) = env_path {
        transform_env_file(env_path)?;
    }

    // Attempt to fetch space definition and update root spaces.yml
    if let Ok(kibana_url) = std::env::var("KIBANA_URL") {
        if let Ok(url) = Url::parse(&kibana_url) {
            let auth = if let Ok(api_key) = std::env::var("KIBANA_APIKEY") {
                Auth::Apikey(api_key)
            } else if let (Ok(u), Ok(p)) = (
                std::env::var("KIBANA_USERNAME"),
                std::env::var("KIBANA_PASSWORD"),
            ) {
                Auth::Basic(u, p)
            } else {
                Auth::None
            };

            if let Ok(client) = KibanaClient::try_new(url, auth, project_dir) {
                let extractor = SpacesExtractor::all(client);
                if let Ok(space_def) = extractor.fetch_space(&target_space).await {
                    let space_file = target_space_dir.join("space.json");
                    let json = serde_json::to_string_pretty(&space_def)?;
                    std::fs::write(&space_file, json)?;
                    log::info!(
                        "Fetched and wrote space definition to {}",
                        space_file.display()
                    );

                    // Update root spaces.yml
                    let spaces_manifest_path = project_dir.join("spaces.yml");
                    let mut spaces_manifest = if spaces_manifest_path.exists() {
                        SpacesManifest::read(&spaces_manifest_path)?
                    } else {
                        SpacesManifest::new()
                    };

                    let space_name = space_def
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&target_space);

                    if spaces_manifest.add_space(target_space.clone(), space_name.to_string()) {
                        spaces_manifest.write(&spaces_manifest_path)?;
                        log::info!("Added space '{}' to root spaces.yml", target_space);
                    }
                }
            }
        }
    }

    // Determine source manifest and migrate saved_objects.json
    let backup_path = if legacy_manifest_path.exists() {
        log::info!("Migrating from legacy manifest.json...");

        // Read legacy manifest
        let manifest = SavedObjectsManifest::read(&legacy_manifest_path)?;
        log::info!("Read legacy manifest with {} objects", manifest.count());

        // Write to new location
        manifest.write(&new_manifest_path)?;
        log::info!("Wrote new manifest to {}", new_manifest_path.display());

        // Migrate object files from flat to hierarchical (if needed)
        migrate_legacy_object_files(project_dir)?;

        // Move objects directory to {space}/objects
        let old_objects_dir = project_dir.join("objects");
        let new_objects_dir = target_space_dir.join("objects");
        if old_objects_dir.exists() && !new_objects_dir.exists() {
            std::fs::rename(&old_objects_dir, &new_objects_dir)?;
            log::info!("Moved objects/ → {}/objects/", target_space);
        }

        // Handle old manifest file
        if backup_old {
            let backup = project_dir.join("manifest.json.backup");
            std::fs::rename(&legacy_manifest_path, &backup)?;
            log::info!("Backed up old manifest to {}", backup.display());
            Some(backup)
        } else {
            std::fs::remove_file(&legacy_manifest_path)?;
            log::info!("Removed old manifest");
            None
        }
    } else if old_manifest_path.exists() {
        log::info!("Migrating from v0.1.0 single-space structure...");

        // Move manifest files to {space}/manifest/ subdirectory
        migrate_file_simple(&old_manifest_path, &new_manifest_path)?;

        // Move other manifest files
        let old_manifest_dir = project_dir.join("manifest");
        for file_name in &["workflows.yml", "agents.yml", "tools.yml"] {
            let old_file = old_manifest_dir.join(file_name);
            let new_file = new_manifest_dir.join(file_name);
            if old_file.exists() {
                migrate_file_simple(&old_file, &new_file)?;
            }
        }

        // Move content directories to {space}/
        for dir_name in &["objects", "workflows", "agents", "tools"] {
            let old_dir = project_dir.join(dir_name);
            let new_dir = target_space_dir.join(dir_name);
            if old_dir.exists() && !new_dir.exists() {
                std::fs::rename(&old_dir, &new_dir)?;
                log::info!("Moved {}/ → {}/{}/", dir_name, target_space, dir_name);
            }
        }

        // Clean up old manifest directory if it's empty
        let old_manifest_dir = project_dir.join("manifest");
        if old_manifest_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&old_manifest_dir) {
                if entries.count() == 0 {
                    std::fs::remove_dir(&old_manifest_dir)?;
                    log::info!("Removed empty manifest/ directory");
                }
            }
        }

        None
    } else {
        return Ok(MigrationResult::NoLegacyManifest);
    };

    log::info!("✓ Migration to multi-space structure complete!");
    log::info!("  New structure: {}/manifest/", target_space);

    if target_space != "default" {
        log::info!(
            "Note: KIBANA_SPACE='{}' is deprecated. You can now safely unset it.",
            target_space
        );
    }

    if let Some(backup) = backup_path {
        Ok(MigrationResult::MigratedWithBackup(backup))
    } else {
        Ok(MigrationResult::MigratedWithoutBackup)
    }
}

/// Helper to migrate legacy flat object files to hierarchical structure
fn migrate_legacy_object_files(project_dir: &Path) -> Result<()> {
    let objects_dir = project_dir.join("objects");

    if !objects_dir.exists() {
        return Ok(());
    }

    let mut migrated_count = 0;

    for entry in std::fs::read_dir(&objects_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip subdirectories (already hierarchical)
        if path.is_dir() {
            continue;
        }

        // Only process .json files
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        // Extract filename without extension
        let filename = match path.file_stem().and_then(|s| s.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Parse flat filename: "object_name.type" format
        if let Some(dot_pos) = filename.rfind('.') {
            let obj_name = &filename[..dot_pos];
            let obj_type = &filename[dot_pos + 1..];

            // Create hierarchical path: type/object_name.json
            let type_dir = objects_dir.join(obj_type);
            let new_path = type_dir.join(format!("{}.json", obj_name));

            std::fs::create_dir_all(&type_dir)?;
            std::fs::rename(&path, &new_path)?;

            log::debug!("Migrated: {} → {}", path.display(), new_path.display());
            migrated_count += 1;
        }
    }

    if migrated_count > 0 {
        log::info!(
            "Migrated {} object file(s) to hierarchical structure",
            migrated_count
        );
    }

    Ok(())
}

/// Simple file move helper
fn migrate_file_simple(old_path: &Path, new_path: &Path) -> Result<()> {
    if !old_path.exists() {
        return Ok(());
    }

    if new_path.exists() {
        log::warn!("Skipping {}: target already exists", old_path.display());
        return Ok(());
    }

    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(old_path, new_path)?;
    log::info!("Moved {} → {}", old_path.display(), new_path.display());

    Ok(())
}

//
// Multi-space migration functions (kept for backward compatibility)
//

/// Check if project needs migration from single-space to multi-space structure
///
/// Returns true if:
/// - Old single-space structure exists: `manifest/saved_objects.json` or `manifest/workflows.yml` (etc.) in root manifest/
/// - New multi-space structure doesn't exist: `manifest/default/` directory doesn't exist
pub fn needs_multispace_migration(project_dir: impl AsRef<Path>) -> bool {
    let project_dir = project_dir.as_ref();
    let old_manifest = project_dir.join("manifest/saved_objects.json");
    let old_workflows = project_dir.join("manifest/workflows.yml");
    let old_agents = project_dir.join("manifest/agents.yml");
    let old_tools = project_dir.join("manifest/tools.yml");

    let new_default_manifest_dir = project_dir.join("manifest/default");

    // If new structure exists, already migrated
    if new_default_manifest_dir.exists() {
        return false;
    }

    // If any old single-space manifest exists, needs migration
    old_manifest.exists() || old_workflows.exists() || old_agents.exists() || old_tools.exists()
}

/// Migrate from single-space structure to multi-space structure
///
/// This migrates the old v0.1.0 single-space structure to the new multi-space structure:
///
/// Old structure:
/// ```text
/// project/
///   ├── manifest/
///   │   ├── saved_objects.json
///   │   ├── workflows.yml
///   │   ├── agents.yml
///   │   ├── tools.yml
///   │   └── spaces.yml
///   ├── objects/           # flat saved objects
///   ├── workflows/         # flat workflows
///   ├── agents/            # flat agents
///   └── tools/             # flat tools
/// ```
///
/// New structure:
/// ```text
/// project/
///   ├── manifest/
///   │   ├── default/
///   │   │   ├── saved_objects.json
///   │   │   ├── workflows.yml
///   │   │   ├── agents.yml
///   │   │   └── tools.yml
///   │   └── spaces.yml
///   ├── default/
///   │   ├── objects/       # moved from root
///   │   ├── workflows/     # moved from root
///   │   ├── agents/        # moved from root
///   │   └── tools/         # moved from root
///   └── spaces/            # space definitions
///       └── default.json
/// ```
pub fn migrate_to_multispace(
    project_dir: impl AsRef<Path>,
    backup_old: bool,
) -> Result<MultispaceMigrationResult> {
    let project_dir = project_dir.as_ref();

    if !needs_multispace_migration(project_dir) {
        return Ok(MultispaceMigrationResult::AlreadyMigrated);
    }

    log::info!("Migrating project to multi-space structure...");

    let mut migrated_items = Vec::new();

    // Create target directories
    let default_manifest_dir = project_dir.join("manifest/default");
    let default_space_dir = project_dir.join("default");

    std::fs::create_dir_all(&default_manifest_dir)?;
    std::fs::create_dir_all(&default_space_dir)?;

    // Migrate saved_objects.json
    migrate_file_to_space(
        project_dir,
        "manifest/saved_objects.json",
        "manifest/default/saved_objects.json",
        backup_old,
        &mut migrated_items,
    )?;

    // Migrate workflows.yml
    migrate_file_to_space(
        project_dir,
        "manifest/workflows.yml",
        "manifest/default/workflows.yml",
        backup_old,
        &mut migrated_items,
    )?;

    // Migrate agents.yml
    migrate_file_to_space(
        project_dir,
        "manifest/agents.yml",
        "manifest/default/agents.yml",
        backup_old,
        &mut migrated_items,
    )?;

    // Migrate tools.yml
    migrate_file_to_space(
        project_dir,
        "manifest/tools.yml",
        "manifest/default/tools.yml",
        backup_old,
        &mut migrated_items,
    )?;

    // Migrate content directories
    migrate_directory_to_space(
        project_dir,
        "objects",
        "default/objects",
        backup_old,
        &mut migrated_items,
    )?;

    migrate_directory_to_space(
        project_dir,
        "workflows",
        "default/workflows",
        backup_old,
        &mut migrated_items,
    )?;

    migrate_directory_to_space(
        project_dir,
        "agents",
        "default/agents",
        backup_old,
        &mut migrated_items,
    )?;

    migrate_directory_to_space(
        project_dir,
        "tools",
        "default/tools",
        backup_old,
        &mut migrated_items,
    )?;

    log::info!(
        "✓ Multi-space migration complete: {} items migrated",
        migrated_items.len()
    );

    Ok(MultispaceMigrationResult::Migrated {
        items: migrated_items,
        backup_created: backup_old,
    })
}

/// Helper function to migrate a single file
fn migrate_file_to_space(
    project_dir: &Path,
    old_path: &str,
    new_path: &str,
    backup: bool,
    migrated_items: &mut Vec<String>,
) -> Result<()> {
    let old = project_dir.join(old_path);
    let new = project_dir.join(new_path);

    if !old.exists() {
        log::debug!("Skipping {}: file doesn't exist", old_path);
        return Ok(());
    }

    if new.exists() {
        log::warn!("Skipping {}: target already exists", new_path);
        return Ok(());
    }

    // Create parent directory
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if backup {
        // Copy then remove
        std::fs::copy(&old, &new)?;
        std::fs::remove_file(&old)?;
        log::info!("Migrated (with backup): {} → {}", old_path, new_path);
    } else {
        // Just move
        std::fs::rename(&old, &new)?;
        log::info!("Migrated: {} → {}", old_path, new_path);
    }

    migrated_items.push(old_path.to_string());
    Ok(())
}

/// Helper function to migrate a directory
fn migrate_directory_to_space(
    project_dir: &Path,
    old_path: &str,
    new_path: &str,
    backup: bool,
    migrated_items: &mut Vec<String>,
) -> Result<()> {
    let old = project_dir.join(old_path);
    let new = project_dir.join(new_path);

    if !old.exists() {
        log::debug!("Skipping {}: directory doesn't exist", old_path);
        return Ok(());
    }

    if new.exists() {
        log::warn!("Skipping {}: target already exists", new_path);
        return Ok(());
    }

    // Create parent directory
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if backup {
        // Copy recursively then remove
        copy_dir_all(&old, &new)?;
        std::fs::remove_dir_all(&old)?;
        log::info!(
            "Migrated directory (with backup): {} → {}",
            old_path,
            new_path
        );
    } else {
        // Just move
        std::fs::rename(&old, &new)?;
        log::info!("Migrated directory: {} → {}", old_path, new_path);
    }

    migrated_items.push(format!("{}/", old_path));
    Ok(())
}

/// Helper to recursively copy a directory
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Result of a multi-space migration operation
#[derive(Debug, Clone, PartialEq)]
pub enum MultispaceMigrationResult {
    /// Migration completed successfully
    Migrated {
        items: Vec<String>,
        backup_created: bool,
    },
    /// Already migrated (manifest/default/ already exists)
    AlreadyMigrated,
}

impl std::fmt::Display for MultispaceMigrationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultispaceMigrationResult::Migrated {
                items,
                backup_created,
            } => {
                write!(
                    f,
                    "Multi-space migration completed: {} items migrated{}",
                    items.len(),
                    if *backup_created {
                        " (with backups)"
                    } else {
                        ""
                    }
                )
            }
            MultispaceMigrationResult::AlreadyMigrated => {
                write!(f, "Already using multi-space structure")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kibana::saved_objects::SavedObject;
    use tempfile::TempDir;

    #[test]
    fn test_needs_migration() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // No manifest files - doesn't need migration
        assert!(!needs_migration(project_dir));

        // Create legacy manifest
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        // Now it needs migration
        assert!(needs_migration(project_dir));

        // Create new manifest directory
        std::fs::create_dir_all(project_dir.join("manifest")).unwrap();
        manifest
            .write(project_dir.join("manifest/saved_objects.json"))
            .unwrap();

        // Already migrated - doesn't need migration
        assert!(!needs_migration(project_dir));
    }

    #[test]
    fn test_migrate_with_backup() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create legacy manifest
        let manifest = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("dashboard", "test-1"),
            SavedObject::new("visualization", "test-2"),
        ]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        // Migrate with backup
        let result = migrate_manifest(project_dir, true).unwrap();
        match result {
            MigrationResult::MigratedWithBackup(backup_path) => {
                assert!(backup_path.exists());
                assert_eq!(backup_path, project_dir.join("manifest.json.backup"));
            }
            _ => panic!("Expected MigratedWithBackup result"),
        }

        // Verify new manifest exists and has correct content
        let new_manifest =
            SavedObjectsManifest::read(project_dir.join("manifest/saved_objects.json")).unwrap();
        assert_eq!(new_manifest.count(), 2);
        assert!(new_manifest.contains("dashboard", "test-1"));
        assert!(new_manifest.contains("visualization", "test-2"));

        // Verify old manifest doesn't exist
        assert!(!project_dir.join("manifest.json").exists());
    }

    #[test]
    fn test_migrate_without_backup() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create legacy manifest
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        // Migrate without backup
        let result = migrate_manifest(project_dir, false).unwrap();
        assert_eq!(result, MigrationResult::MigratedWithoutBackup);

        // Verify new manifest exists
        assert!(project_dir.join("manifest/saved_objects.json").exists());

        // Verify old manifest and backup don't exist
        assert!(!project_dir.join("manifest.json").exists());
        assert!(!project_dir.join("manifest.json.backup").exists());
    }

    #[test]
    fn test_migrate_no_legacy_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        let result = migrate_manifest(project_dir, true).unwrap();
        assert_eq!(result, MigrationResult::NoLegacyManifest);
    }

    #[test]
    fn test_migrate_already_migrated() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create both old and new manifests
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest.write(project_dir.join("manifest.json")).unwrap();
        std::fs::create_dir_all(project_dir.join("manifest")).unwrap();
        manifest
            .write(project_dir.join("manifest/saved_objects.json"))
            .unwrap();

        let result = migrate_manifest(project_dir, true).unwrap();
        assert_eq!(result, MigrationResult::AlreadyMigrated);
    }

    #[test]
    fn test_load_saved_objects_manifest_new_location() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create new manifest
        std::fs::create_dir_all(project_dir.join("manifest")).unwrap();
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "new-1")]);
        manifest
            .write(project_dir.join("manifest/saved_objects.json"))
            .unwrap();

        let loaded = load_saved_objects_manifest(project_dir).unwrap();
        assert_eq!(loaded.count(), 1);
        assert!(loaded.contains("dashboard", "new-1"));
    }

    #[test]
    fn test_load_saved_objects_manifest_legacy_location() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create legacy manifest only
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "legacy-1")]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        let loaded = load_saved_objects_manifest(project_dir).unwrap();
        assert_eq!(loaded.count(), 1);
        assert!(loaded.contains("dashboard", "legacy-1"));
    }

    #[test]
    fn test_load_saved_objects_manifest_prefers_new() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create both manifests with different content
        let legacy =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "legacy-1")]);
        legacy.write(project_dir.join("manifest.json")).unwrap();

        std::fs::create_dir_all(project_dir.join("manifest")).unwrap();
        let new = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("dashboard", "new-1"),
            SavedObject::new("visualization", "new-2"),
        ]);
        new.write(project_dir.join("manifest/saved_objects.json"))
            .unwrap();

        // Should load from new location
        let loaded = load_saved_objects_manifest(project_dir).unwrap();
        assert_eq!(loaded.count(), 2);
        assert!(loaded.contains("dashboard", "new-1"));
        assert!(loaded.contains("visualization", "new-2"));
    }

    #[test]
    fn test_load_saved_objects_manifest_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        let result = load_saved_objects_manifest(project_dir);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No saved objects manifest found")
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_needs_migration_unified() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // No manifest files - doesn't need migration
        assert!(!needs_migration_unified(project_dir));

        // Create legacy manifest
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        // Now it needs migration
        assert!(needs_migration_unified(project_dir));

        // Migrate to multi-space structure
        let result = migrate_to_multispace_unified(project_dir, false, None::<&Path>)
            .await
            .unwrap();
        match result {
            MigrationResult::MigratedWithoutBackup => {}
            _ => panic!("Expected MigratedWithoutBackup result"),
        }

        // After migration, shouldn't need migration anymore
        assert!(!needs_migration_unified(project_dir));

        // Verify new structure exists
        assert!(
            project_dir
                .join("default/manifest/saved_objects.json")
                .exists()
        );
        assert!(project_dir.join("default").exists());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_migrate_unified_from_legacy() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create legacy manifest
        let manifest = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("dashboard", "test-1"),
            SavedObject::new("visualization", "test-2"),
        ]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        // Create legacy objects directory with flat files
        let objects_dir = project_dir.join("objects");
        std::fs::create_dir_all(&objects_dir).unwrap();
        std::fs::write(objects_dir.join("test-1.dashboard.json"), "{}").unwrap();

        // Migrate with backup
        let result = migrate_to_multispace_unified(project_dir, true, None::<&Path>)
            .await
            .unwrap();
        match result {
            MigrationResult::MigratedWithBackup(backup_path) => {
                assert!(backup_path.exists());
                assert_eq!(backup_path, project_dir.join("manifest.json.backup"));
            }
            _ => panic!("Expected MigratedWithBackup result"),
        }

        // Verify new structure
        let new_manifest = project_dir.join("default/manifest/saved_objects.json");
        assert!(new_manifest.exists());

        let loaded = SavedObjectsManifest::read(&new_manifest).unwrap();
        assert_eq!(loaded.count(), 2);

        // Verify objects moved to hierarchical structure
        assert!(project_dir.join("default/objects").exists());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_migrate_unified_from_v0_1_0() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create v0.1.0 structure
        let manifest_dir = project_dir.join("manifest");
        std::fs::create_dir_all(&manifest_dir).unwrap();

        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest
            .write(manifest_dir.join("saved_objects.json"))
            .unwrap();

        // Create content directories
        let objects_dir = project_dir.join("objects");
        std::fs::create_dir_all(&objects_dir).unwrap();

        // Migrate (no backup for v0.1.0 → multi-space)
        let result = migrate_to_multispace_unified(project_dir, false, None::<&Path>)
            .await
            .unwrap();
        match result {
            MigrationResult::MigratedWithoutBackup => {}
            _ => panic!("Expected MigratedWithoutBackup result"),
        }

        // Verify new structure
        assert!(
            project_dir
                .join("default/manifest/saved_objects.json")
                .exists()
        );
        assert!(project_dir.join("default/objects").exists());

        // Old manifest should be gone
        assert!(!project_dir.join("manifest/saved_objects.json").exists());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_migrate_unified_already_migrated() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create new multi-space structure
        let new_manifest_dir = project_dir.join("default/manifest");
        std::fs::create_dir_all(&new_manifest_dir).unwrap();

        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest
            .write(new_manifest_dir.join("saved_objects.json"))
            .unwrap();

        // Try to migrate
        let result = migrate_to_multispace_unified(project_dir, false, None::<&Path>)
            .await
            .unwrap();
        match result {
            MigrationResult::AlreadyMigrated => {}
            _ => panic!("Expected AlreadyMigrated result"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_migrate_space_aware() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create legacy manifest
        let manifest =
            SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "test-1")]);
        manifest.write(project_dir.join("manifest.json")).unwrap();

        // Set lowercase env var
        unsafe {
            std::env::set_var("kibana_space", "marketing");
        }

        // Migrate
        let result = migrate_to_multispace_unified(project_dir, false, None::<&Path>)
            .await
            .unwrap();
        assert!(matches!(result, MigrationResult::MigratedWithoutBackup));

        // Verify it used "marketing" directory
        assert!(
            project_dir
                .join("marketing/manifest/saved_objects.json")
                .exists()
        );
        assert!(!project_dir.join("default").exists());

        // Clean up
        unsafe {
            std::env::remove_var("kibana_space");
        }
    }
}
