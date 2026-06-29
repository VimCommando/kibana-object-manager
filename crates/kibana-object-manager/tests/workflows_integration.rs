//! Integration tests for workflows functionality

use eyre::Result;
use kibana_object_manager::cli::{bundle_workflows_to_ndjson, pull_workflows, push_workflows};
use kibana_object_manager::kibana::workflows::{WorkflowEntry, WorkflowsManifest};
use std::path::Path;
use tempfile::TempDir;

/// Create a test project with workflows manifest
fn create_test_project_with_workflows(dir: &Path) -> Result<()> {
    // Create manifest directory
    let manifest_dir = dir.join("manifest");
    std::fs::create_dir_all(&manifest_dir)?;

    // Create workflows manifest
    let manifest = WorkflowsManifest::with_workflows(vec![
        WorkflowEntry::new("workflow-123", "my-workflow"),
        WorkflowEntry::new("workflow-456", "alert-workflow"),
    ]);
    manifest.write(manifest_dir.join("workflows.yml"))?;

    // Create workflows directory with sample workflow files
    let workflows_dir = dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;

    // Write sample workflow files
    let my_workflow = serde_json::json!({
        "id": "workflow-123",
        "name": "my-workflow",
        "description": "My example workflow",
        "enabled": true
    });
    let alert_workflow = serde_json::json!({
        "id": "workflow-456",
        "name": "alert-workflow",
        "description": "Alert workflow",
        "enabled": true
    });

    std::fs::write(
        workflows_dir.join("my-workflow.json"),
        serde_json::to_string_pretty(&my_workflow)?,
    )?;
    std::fs::write(
        workflows_dir.join("alert-workflow.json"),
        serde_json::to_string_pretty(&alert_workflow)?,
    )?;

    Ok(())
}

#[test]
fn test_workflows_manifest_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_workflows(temp_dir.path())?;

    // Verify manifest was created
    let manifest_path = temp_dir.path().join("manifest/workflows.yml");
    assert!(manifest_path.exists());

    // Verify manifest can be read
    let manifest = WorkflowsManifest::read(&manifest_path)?;
    assert_eq!(manifest.count(), 2);
    assert!(manifest.contains_id("workflow-123"));
    assert!(manifest.contains_name("my-workflow"));
    assert!(manifest.contains_id("workflow-456"));
    assert!(manifest.contains_name("alert-workflow"));

    Ok(())
}

#[test]
fn test_workflows_directory_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_workflows(temp_dir.path())?;

    // Verify workflows directory exists
    let workflows_dir = temp_dir.path().join("workflows");
    assert!(workflows_dir.exists());

    // Verify workflow files exist
    assert!(workflows_dir.join("my-workflow.json").exists());
    assert!(workflows_dir.join("alert-workflow.json").exists());

    // Verify workflow files are valid JSON
    let my_workflow_content = std::fs::read_to_string(workflows_dir.join("my-workflow.json"))?;
    let my_workflow: serde_json::Value = serde_json::from_str(&my_workflow_content)?;
    assert_eq!(my_workflow["name"], "my-workflow");
    assert_eq!(my_workflow["id"], "workflow-123");

    Ok(())
}

#[tokio::test]
async fn test_bundle_workflows_to_ndjson() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_workflows(temp_dir.path())?;

    let output_file = temp_dir.path().join("workflows.ndjson");
    let count = bundle_workflows_to_ndjson(temp_dir.path(), &output_file).await?;

    assert_eq!(count, 2);
    assert!(output_file.exists());

    // Verify NDJSON format
    let content = std::fs::read_to_string(&output_file)?;
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);

    // Verify each line is valid JSON
    for line in lines {
        let _workflow: serde_json::Value = serde_json::from_str(line)?;
    }

    Ok(())
}

#[tokio::test]
#[ignore] // Requires live Kibana connection
async fn test_pull_workflows() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_workflows(temp_dir.path())?;

    // This test requires a live Kibana connection
    let count = pull_workflows(temp_dir.path()).await?;
    assert!(count > 0);

    Ok(())
}

#[tokio::test]
#[ignore] // Requires live Kibana connection
async fn test_push_workflows() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_project_with_workflows(temp_dir.path())?;

    // This test requires a live Kibana connection
    let count = push_workflows(temp_dir.path()).await?;
    assert_eq!(count, 2);

    Ok(())
}

#[test]
fn test_workflows_manifest_yaml_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manifest_path = temp_dir.path().join("workflows.yml");

    let manifest = WorkflowsManifest::with_workflows(vec![
        WorkflowEntry::new("wf1", "workflow1"),
        WorkflowEntry::new("wf2", "workflow2"),
        WorkflowEntry::new("wf3", "workflow3"),
    ]);

    manifest.write(&manifest_path)?;

    // Read the YAML file and verify format
    let content = std::fs::read_to_string(&manifest_path)?;
    assert!(content.contains("workflows:"));
    assert!(content.contains("id: wf1"));
    assert!(content.contains("name: workflow1"));
    assert!(content.contains("id: wf2"));
    assert!(content.contains("name: workflow2"));
    assert!(content.contains("id: wf3"));
    assert!(content.contains("name: workflow3"));

    // Verify it can be read back
    let loaded = WorkflowsManifest::read(&manifest_path)?;
    assert_eq!(loaded.count(), 3);

    Ok(())
}
