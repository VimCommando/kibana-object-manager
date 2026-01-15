//! Integration test for manifest migration

use eyre::Result;
use kibana_object_manager::migration::{
    MigrationResult, load_saved_objects_manifest, migrate_manifest, needs_migration,
};
use std::fs;
use tempfile::TempDir;

/// Helper function to create a test project with legacy manifest and flat object files
fn create_test_project() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let project_dir = temp_dir.path();

    // Create legacy manifest.json
    let manifest_content = r#"{
  "objects": [
    {
      "type": "dashboard",
      "id": "allocation-overview"
    },
    {
      "type": "dashboard",
      "id": "data-summary"
    },
    {
      "type": "visualization",
      "id": "test-viz"
    }
  ],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}"#;
    fs::write(project_dir.join("manifest.json"), manifest_content)?;

    // Create objects directory with FLAT structure (legacy format: object_name.type.json)
    fs::create_dir_all(project_dir.join("objects"))?;

    // Create flat files: object_name.type.json format
    fs::write(
        project_dir.join("objects/allocation-overview.dashboard.json"),
        r#"{"type":"dashboard","id":"allocation-overview","attributes":{"title":"Allocation Overview"}}"#,
    )?;
    fs::write(
        project_dir.join("objects/data-summary.dashboard.json"),
        r#"{"type":"dashboard","id":"data-summary","attributes":{"title":"Data Summary"}}"#,
    )?;
    fs::write(
        project_dir.join("objects/test-viz.visualization.json"),
        r#"{"type":"visualization","id":"test-viz","attributes":{"title":"Test Visualization"}}"#,
    )?;

    Ok(temp_dir)
}

#[test]
fn test_needs_migration_with_legacy_manifest() -> Result<()> {
    let temp_dir = create_test_project()?;
    let project_dir = temp_dir.path();

    // Should detect that migration is needed
    let needs = needs_migration(project_dir);
    assert!(needs, "Should detect migration is needed");

    // The test directory should have a legacy manifest.json
    assert!(
        project_dir.join("manifest.json").exists(),
        "Expected manifest.json to exist"
    );

    Ok(())
}

#[test]
fn test_load_legacy_manifest() -> Result<()> {
    let temp_dir = create_test_project()?;
    let project_dir = temp_dir.path();

    // Should be able to load from legacy location
    let manifest = load_saved_objects_manifest(project_dir)?;

    assert_eq!(manifest.count(), 3, "Should have loaded 3 objects");

    // Check expected objects
    assert!(manifest.contains("dashboard", "allocation-overview"));
    assert!(manifest.contains("dashboard", "data-summary"));
    assert!(manifest.contains("visualization", "test-viz"));

    Ok(())
}

#[test]
fn test_migrate_manifest() -> Result<()> {
    let temp_dir = create_test_project()?;
    let project_dir = temp_dir.path();

    // Verify legacy flat files exist before migration (object_name.type.json format)
    assert!(
        project_dir
            .join("objects/allocation-overview.dashboard.json")
            .exists(),
        "Legacy flat file should exist before migration"
    );
    assert!(
        project_dir
            .join("objects/data-summary.dashboard.json")
            .exists(),
        "Legacy flat file should exist before migration"
    );
    assert!(
        project_dir
            .join("objects/test-viz.visualization.json")
            .exists(),
        "Legacy flat file should exist before migration"
    );

    // Perform migration with backup
    let result = migrate_manifest(project_dir, true)?;

    match result {
        MigrationResult::MigratedWithBackup(backup_path) => {
            // Verify backup exists
            assert!(backup_path.exists(), "Backup file should exist");

            // Verify new manifest exists
            let new_manifest_path = project_dir.join("manifest/saved_objects.json");
            assert!(new_manifest_path.exists(), "New manifest should exist");

            // Load from new location
            let manifest = load_saved_objects_manifest(project_dir)?;
            assert_eq!(manifest.count(), 3, "Should have loaded 3 objects");

            // Verify old manifest doesn't exist
            assert!(
                !project_dir.join("manifest.json").exists(),
                "Old manifest.json should not exist"
            );

            // Verify object files were migrated to hierarchical structure
            assert!(
                project_dir
                    .join("objects/dashboard/allocation-overview.json")
                    .exists(),
                "Hierarchical dashboard file should exist"
            );
            assert!(
                project_dir
                    .join("objects/dashboard/data-summary.json")
                    .exists(),
                "Hierarchical dashboard file should exist"
            );
            assert!(
                project_dir
                    .join("objects/visualization/test-viz.json")
                    .exists(),
                "Hierarchical visualization file should exist"
            );

            // Verify flat files no longer exist
            assert!(
                !project_dir
                    .join("objects/allocation-overview.dashboard.json")
                    .exists(),
                "Flat file should be removed after migration"
            );
            assert!(
                !project_dir
                    .join("objects/data-summary.dashboard.json")
                    .exists(),
                "Flat file should be removed after migration"
            );
            assert!(
                !project_dir
                    .join("objects/test-viz.visualization.json")
                    .exists(),
                "Flat file should be removed after migration"
            );
        }
        _ => panic!("Expected MigratedWithBackup result"),
    }

    Ok(())
}

#[test]
fn test_needs_migration_already_migrated() -> Result<()> {
    let temp_dir = create_test_project()?;
    let project_dir = temp_dir.path();

    // First migrate
    migrate_manifest(project_dir, false)?;

    // Should detect no migration needed
    let needs = needs_migration(project_dir);
    assert!(!needs, "Should not need migration after migrating");

    Ok(())
}

#[test]
fn test_migrate_already_migrated() -> Result<()> {
    let temp_dir = create_test_project()?;
    let project_dir = temp_dir.path();

    // First migrate (removes legacy manifest.json)
    migrate_manifest(project_dir, false)?;

    // Try to migrate again - should detect no legacy manifest
    let result = migrate_manifest(project_dir, false)?;

    match result {
        MigrationResult::NoLegacyManifest => {
            // This is expected - legacy manifest was removed in first migration
        }
        other => panic!("Expected NoLegacyManifest result, got: {:?}", other),
    }

    Ok(())
}

#[test]
fn test_migrate_with_mixed_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_dir = temp_dir.path();

    // Create manifest
    let manifest_content = r#"{
  "objects": [
    {"type": "dashboard", "id": "flat-dash"},
    {"type": "dashboard", "id": "hierarchical-dash"},
    {"type": "visualization", "id": "flat-viz"}
  ]
}"#;
    fs::write(project_dir.join("manifest.json"), manifest_content)?;

    // Create objects directory with MIXED structure
    fs::create_dir_all(project_dir.join("objects"))?;
    fs::create_dir_all(project_dir.join("objects/dashboard"))?;

    // One flat file (legacy format: object_name.type.json)
    fs::write(
        project_dir.join("objects/flat-dash.dashboard.json"),
        r#"{"type":"dashboard","id":"flat-dash"}"#,
    )?;

    // One hierarchical file (already migrated)
    fs::write(
        project_dir.join("objects/dashboard/hierarchical-dash.json"),
        r#"{"type":"dashboard","id":"hierarchical-dash"}"#,
    )?;

    // Another flat file
    fs::write(
        project_dir.join("objects/flat-viz.visualization.json"),
        r#"{"type":"visualization","id":"flat-viz"}"#,
    )?;

    // Run migration
    migrate_manifest(project_dir, true)?;

    // Verify all files are now hierarchical
    assert!(
        project_dir
            .join("objects/dashboard/flat-dash.json")
            .exists(),
        "Flat file should be migrated to hierarchical"
    );
    assert!(
        project_dir
            .join("objects/dashboard/hierarchical-dash.json")
            .exists(),
        "Already hierarchical file should remain"
    );
    assert!(
        project_dir
            .join("objects/visualization/flat-viz.json")
            .exists(),
        "Flat visualization should be migrated"
    );

    // Verify flat files are gone
    assert!(
        !project_dir
            .join("objects/flat-dash.dashboard.json")
            .exists(),
        "Flat file should be removed"
    );
    assert!(
        !project_dir
            .join("objects/flat-viz.visualization.json")
            .exists(),
        "Flat file should be removed"
    );

    Ok(())
}
