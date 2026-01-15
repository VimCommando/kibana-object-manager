//! Workflows manifest management
//!
//! The workflows manifest is stored as `manifest/workflows.yml` and contains
//! a list of workflows with their IDs and names.
//!
//! Example format:
//! ```yaml
//! workflows:
//!   - id: workflow-123
//!     name: my-workflow
//!   - id: workflow-456
//!     name: alert-workflow
//!   - id: workflow-789
//!     name: data-pipeline
//! ```

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Workflow entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEntry {
    /// Workflow ID (used for API calls)
    pub id: String,
    /// Workflow name (used for file storage)
    pub name: String,
}

impl WorkflowEntry {
    /// Create a new workflow entry
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Workflows manifest structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowsManifest {
    /// List of workflows to manage
    pub workflows: Vec<WorkflowEntry>,
}

impl WorkflowsManifest {
    /// Create a new empty workflows manifest
    pub fn new() -> Self {
        Self {
            workflows: Vec::new(),
        }
    }

    /// Create a manifest with specified workflows
    pub fn with_workflows(workflows: Vec<WorkflowEntry>) -> Self {
        Self { workflows }
    }

    /// Add a workflow to the manifest
    pub fn add_workflow(&mut self, workflow: WorkflowEntry) {
        // Check if workflow with same ID already exists
        if !self.workflows.iter().any(|w| w.id == workflow.id) {
            self.workflows.push(workflow);
        }
    }

    /// Remove a workflow by ID from the manifest
    pub fn remove_workflow_by_id(&mut self, workflow_id: &str) -> bool {
        if let Some(pos) = self.workflows.iter().position(|w| w.id == workflow_id) {
            self.workflows.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove a workflow by name from the manifest
    pub fn remove_workflow_by_name(&mut self, workflow_name: &str) -> bool {
        if let Some(pos) = self.workflows.iter().position(|w| w.name == workflow_name) {
            self.workflows.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if a workflow ID is in the manifest
    pub fn contains_id(&self, workflow_id: &str) -> bool {
        self.workflows.iter().any(|w| w.id == workflow_id)
    }

    /// Check if a workflow name is in the manifest
    pub fn contains_name(&self, workflow_name: &str) -> bool {
        self.workflows.iter().any(|w| w.name == workflow_name)
    }

    /// Get workflow entry by ID
    pub fn get_by_id(&self, workflow_id: &str) -> Option<&WorkflowEntry> {
        self.workflows.iter().find(|w| w.id == workflow_id)
    }

    /// Get workflow entry by name
    pub fn get_by_name(&self, workflow_name: &str) -> Option<&WorkflowEntry> {
        self.workflows.iter().find(|w| w.name == workflow_name)
    }

    /// Get the number of workflows in the manifest
    pub fn count(&self) -> usize {
        self.workflows.len()
    }

    /// Read manifest from YAML file
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).with_context(|| {
            format!(
                "Failed to read workflows manifest: {}",
                path.as_ref().display()
            )
        })?;

        let manifest: Self = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse workflows manifest YAML")?;

        Ok(manifest)
    }

    /// Write manifest to YAML file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize workflows manifest to YAML")?;

        std::fs::write(path.as_ref(), yaml).with_context(|| {
            format!(
                "Failed to write workflows manifest: {}",
                path.as_ref().display()
            )
        })?;

        Ok(())
    }
}

impl Default for WorkflowsManifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_manifest() {
        let manifest = WorkflowsManifest::new();
        assert_eq!(manifest.count(), 0);
    }

    #[test]
    fn test_with_workflows() {
        let manifest = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
        ]);
        assert_eq!(manifest.count(), 2);
        assert!(manifest.contains_id("wf1"));
        assert!(manifest.contains_name("workflow1"));
        assert!(manifest.contains_id("wf2"));
        assert!(manifest.contains_name("workflow2"));
    }

    #[test]
    fn test_add_workflow() {
        let mut manifest = WorkflowsManifest::new();
        manifest.add_workflow(WorkflowEntry::new("test-id", "test"));
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains_id("test-id"));
        assert!(manifest.contains_name("test"));

        // Adding duplicate should not increase count
        manifest.add_workflow(WorkflowEntry::new("test-id", "test"));
        assert_eq!(manifest.count(), 1);
    }

    #[test]
    fn test_remove_workflow_by_id() {
        let mut manifest = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
        ]);

        assert!(manifest.remove_workflow_by_id("wf1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains_id("wf1"));

        // Removing non-existent workflow returns false
        assert!(!manifest.remove_workflow_by_id("nonexistent"));
    }

    #[test]
    fn test_remove_workflow_by_name() {
        let mut manifest = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
        ]);

        assert!(manifest.remove_workflow_by_name("workflow1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains_name("workflow1"));

        // Removing non-existent workflow returns false
        assert!(!manifest.remove_workflow_by_name("nonexistent"));
    }

    #[test]
    fn test_get_by_id() {
        let manifest = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
        ]);

        let entry = manifest.get_by_id("wf1").unwrap();
        assert_eq!(entry.id, "wf1");
        assert_eq!(entry.name, "workflow1");

        assert!(manifest.get_by_id("nonexistent").is_none());
    }

    #[test]
    fn test_get_by_name() {
        let manifest = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
        ]);

        let entry = manifest.get_by_name("workflow1").unwrap();
        assert_eq!(entry.id, "wf1");
        assert_eq!(entry.name, "workflow1");

        assert!(manifest.get_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest").join("workflows.yml");

        let original = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
            WorkflowEntry::new("wf3", "workflow3"),
        ]);

        // Write
        original.write(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // Read
        let loaded = WorkflowsManifest::read(&manifest_path).unwrap();
        assert_eq!(loaded, original);
        assert_eq!(loaded.count(), 3);
    }

    #[test]
    fn test_yaml_format() {
        let manifest = WorkflowsManifest::with_workflows(vec![
            WorkflowEntry::new("wf1", "workflow1"),
            WorkflowEntry::new("wf2", "workflow2"),
        ]);
        let yaml = serde_yaml::to_string(&manifest).unwrap();

        assert!(yaml.contains("workflows:"));
        assert!(yaml.contains("id: wf1"));
        assert!(yaml.contains("name: workflow1"));
        assert!(yaml.contains("id: wf2"));
        assert!(yaml.contains("name: workflow2"));
    }
}
