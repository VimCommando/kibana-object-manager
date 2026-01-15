//! Manifest migration utilities
//!
//! Provides utilities for migrating from the legacy single-file manifest structure
//! to the new directory-based manifest structure.
//!
//! Legacy structure:
//! ```text
//! project/
//!   ├── manifest.json        (saved objects manifest)
//!   └── objects/            (saved object files)
//! ```
//!
//! New structure:
//! ```text
//! project/
//!   ├── manifest/
//!   │   ├── saved_objects.json  (saved objects manifest)
//!   │   └── spaces.yml          (spaces manifest, optional)
//!   ├── objects/                (saved object files)
//!   └── spaces/                 (space definition files, optional)
//! ```

use eyre::{Context, Result};
use std::path::{Path, PathBuf};

use crate::kibana::saved_objects::SavedObjectsManifest;

/// Migrate a legacy manifest.json file to the new manifest/ directory structure
///
/// This function:
/// 1. Reads the legacy `manifest.json` file
/// 2. Creates a `manifest/` directory if it doesn't exist
/// 3. Writes the manifest to `manifest/saved_objects.json`
/// 4. Optionally backs up or removes the old `manifest.json` file
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
}
