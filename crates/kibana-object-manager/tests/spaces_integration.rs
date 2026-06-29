//! Integration tests for spaces functionality

use eyre::Result;
use kibana_object_manager::cli::{bundle_spaces_to_ndjson, pull_spaces, push_spaces};
use kibana_object_manager::kibana::spaces::{SpaceEntry, SpacesManifest};
use std::path::Path;
use tempfile::TempDir;

/// Create a test project with spaces manifest
fn create_test_project_with_spaces(dir: &Path) -> Result<()> {
    // Create spaces manifest at project root (spaces.yml)
    let manifest = SpacesManifest::with_spaces(vec![
        SpaceEntry::new("default".to_string(), "Default".to_string()),
        SpaceEntry::new("marketing".to_string(), "Marketing".to_string()),
    ]);
    manifest.write(dir.join("spaces.yml"))?;

    // Create space directories and write space.json files
    let default_space = serde_json::json!({
        "id": "default",
        "name": "Default",
        "description": "This is the default space",
        "disabledFeatures": []
    });
    let marketing_space = serde_json::json!({
        "id": "marketing",
        "name": "Marketing",
        "description": "Marketing team space",
        "disabledFeatures": []
    });

    // Write to {space_id}/space.json structure
    let default_dir = dir.join("default");
    std::fs::create_dir_all(&default_dir)?;
    std::fs::write(
        default_dir.join("space.json"),
        serde_json::to_string_pretty(&default_space)?,
    )?;

    let marketing_dir = dir.join("marketing");
    std::fs::create_dir_all(&marketing_dir)?;
    std::fs::write(
        marketing_dir.join("space.json"),
        serde_json::to_string_pretty(&marketing_space)?,
    )?;

    Ok(())
}

#[test]
fn test_spaces_manifest_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_spaces(temp_dir.path())?;

    // Verify manifest was created
    let manifest_path = temp_dir.path().join("spaces.yml");
    assert!(manifest_path.exists());

    // Verify manifest can be read
    let manifest = SpacesManifest::read(&manifest_path)?;
    assert_eq!(manifest.count(), 2);
    assert!(manifest.contains("default"));
    assert!(manifest.contains("marketing"));

    Ok(())
}

#[test]
fn test_spaces_directory_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_spaces(temp_dir.path())?;

    // Verify space directories exist
    let default_dir = temp_dir.path().join("default");
    assert!(default_dir.exists());
    let marketing_dir = temp_dir.path().join("marketing");
    assert!(marketing_dir.exists());

    // Verify space.json files exist
    assert!(default_dir.join("space.json").exists());
    assert!(marketing_dir.join("space.json").exists());

    // Verify space files are valid JSON
    let default_content = std::fs::read_to_string(default_dir.join("space.json"))?;
    let default_space: serde_json::Value = serde_json::from_str(&default_content)?;
    assert_eq!(default_space["id"], "default");

    Ok(())
}

#[tokio::test]
async fn test_bundle_spaces_to_ndjson() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_spaces(temp_dir.path())?;

    let output_file = temp_dir.path().join("spaces.ndjson");
    let count = bundle_spaces_to_ndjson(temp_dir.path(), &output_file).await?;

    assert_eq!(count, 2);
    assert!(output_file.exists());

    // Verify NDJSON format
    let content = std::fs::read_to_string(&output_file)?;
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);

    // Verify each line is valid JSON
    for line in lines {
        let _space: serde_json::Value = serde_json::from_str(line)?;
    }

    Ok(())
}

#[tokio::test]
#[ignore] // Requires live Kibana connection
async fn test_pull_spaces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_spaces(temp_dir.path())?;

    // This test requires a live Kibana connection
    let count = pull_spaces(temp_dir.path()).await?;
    assert!(count > 0);

    Ok(())
}

#[tokio::test]
#[ignore] // Requires live Kibana connection
async fn test_push_spaces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_spaces(temp_dir.path())?;

    // This test requires a live Kibana connection
    let count = push_spaces(temp_dir.path()).await?;
    assert_eq!(count, 2);

    Ok(())
}

#[test]
fn test_spaces_manifest_yaml_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manifest_path = temp_dir.path().join("spaces.yml");

    let manifest = SpacesManifest::with_spaces(vec![
        SpaceEntry::new("default".to_string(), "Default".to_string()),
        SpaceEntry::new("team-a".to_string(), "Team A".to_string()),
        SpaceEntry::new("team-b".to_string(), "Team B".to_string()),
    ]);

    manifest.write(&manifest_path)?;

    // Read the YAML file and verify format
    let content = std::fs::read_to_string(&manifest_path)?;
    assert!(content.contains("spaces:"));
    assert!(content.contains("id: default"));
    assert!(content.contains("name: Default"));
    assert!(content.contains("id: team-a"));
    assert!(content.contains("name: Team A"));
    assert!(content.contains("id: team-b"));
    assert!(content.contains("name: Team B"));

    // Verify it can be read back
    let loaded = SpacesManifest::read(&manifest_path)?;
    assert_eq!(loaded.count(), 3);

    Ok(())
}
