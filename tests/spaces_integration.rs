//! Integration tests for spaces functionality

use eyre::Result;
use kibana_object_manager::cli::{bundle_spaces_to_ndjson, pull_spaces, push_spaces};
use kibana_object_manager::kibana::spaces::SpacesManifest;
use std::path::Path;
use tempfile::TempDir;

/// Create a test project with spaces manifest
fn create_test_project_with_spaces(dir: &Path) -> Result<()> {
    // Create manifest directory
    let manifest_dir = dir.join("manifest");
    std::fs::create_dir_all(&manifest_dir)?;

    // Create spaces manifest
    let manifest =
        SpacesManifest::with_spaces(vec!["default".to_string(), "marketing".to_string()]);
    manifest.write(manifest_dir.join("spaces.yml"))?;

    // Create spaces directory with sample space files
    let spaces_dir = dir.join("spaces");
    std::fs::create_dir_all(&spaces_dir)?;

    // Write sample space files
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

    std::fs::write(
        spaces_dir.join("default.json"),
        serde_json::to_string_pretty(&default_space)?,
    )?;
    std::fs::write(
        spaces_dir.join("marketing.json"),
        serde_json::to_string_pretty(&marketing_space)?,
    )?;

    Ok(())
}

#[test]
fn test_spaces_manifest_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_spaces(temp_dir.path())?;

    // Verify manifest was created
    let manifest_path = temp_dir.path().join("manifest/spaces.yml");
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

    // Verify spaces directory exists
    let spaces_dir = temp_dir.path().join("spaces");
    assert!(spaces_dir.exists());

    // Verify space files exist
    assert!(spaces_dir.join("default.json").exists());
    assert!(spaces_dir.join("marketing.json").exists());

    // Verify space files are valid JSON
    let default_content = std::fs::read_to_string(spaces_dir.join("default.json"))?;
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
        "default".to_string(),
        "team-a".to_string(),
        "team-b".to_string(),
    ]);

    manifest.write(&manifest_path)?;

    // Read the YAML file and verify format
    let content = std::fs::read_to_string(&manifest_path)?;
    assert!(content.contains("spaces:"));
    assert!(content.contains("- default"));
    assert!(content.contains("- team-a"));
    assert!(content.contains("- team-b"));

    // Verify it can be read back
    let loaded = SpacesManifest::read(&manifest_path)?;
    assert_eq!(loaded.count(), 3);

    Ok(())
}
