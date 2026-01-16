//! Tools manifest management
//!
//! The tools manifest is stored as `manifest/tools.yml` and contains
//! a list of tool IDs to manage.
//!
//! Example format:
//! ```yaml
//! tools:
//!   - platform.core.search
//!   - platform.core.get_document_by_id
//!   - platform.core.generate_esql
//! ```

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Tools manifest structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolsManifest {
    /// List of tool IDs to manage
    pub tools: Vec<String>,
}

impl ToolsManifest {
    /// Create a new empty tools manifest
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Create a manifest with specified tool IDs
    pub fn with_tools(tools: Vec<String>) -> Self {
        Self { tools }
    }

    /// Add a tool ID to the manifest
    ///
    /// Returns true if tool was added, false if it already exists
    pub fn add_tool(&mut self, tool_id: String) -> bool {
        if !self.tools.contains(&tool_id) {
            self.tools.push(tool_id);
            true
        } else {
            false
        }
    }

    /// Remove a tool ID from the manifest
    pub fn remove_tool(&mut self, tool_id: &str) -> bool {
        if let Some(pos) = self.tools.iter().position(|t| t == tool_id) {
            self.tools.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if a tool ID is in the manifest
    pub fn contains(&self, tool_id: &str) -> bool {
        self.tools.contains(&tool_id.to_string())
    }

    /// Get the number of tools in the manifest
    pub fn count(&self) -> usize {
        self.tools.len()
    }

    /// Read manifest from YAML file
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).with_context(|| {
            format!("Failed to read tools manifest: {}", path.as_ref().display())
        })?;

        let manifest: Self = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse tools manifest YAML")?;

        Ok(manifest)
    }

    /// Write manifest to YAML file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize tools manifest to YAML")?;

        std::fs::write(path.as_ref(), yaml).with_context(|| {
            format!(
                "Failed to write tools manifest: {}",
                path.as_ref().display()
            )
        })?;

        Ok(())
    }
}

impl Default for ToolsManifest {
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
        let manifest = ToolsManifest::new();
        assert_eq!(manifest.count(), 0);
    }

    #[test]
    fn test_with_tools() {
        let manifest = ToolsManifest::with_tools(vec!["tool1".to_string(), "tool2".to_string()]);
        assert_eq!(manifest.count(), 2);
        assert!(manifest.contains("tool1"));
        assert!(manifest.contains("tool2"));
    }

    #[test]
    fn test_add_tool() {
        let mut manifest = ToolsManifest::new();
        assert!(manifest.add_tool("test-id".to_string()));
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains("test-id"));

        // Adding duplicate should not increase count
        assert!(!manifest.add_tool("test-id".to_string()));
        assert_eq!(manifest.count(), 1);
    }

    #[test]
    fn test_remove_tool() {
        let mut manifest =
            ToolsManifest::with_tools(vec!["tool1".to_string(), "tool2".to_string()]);

        assert!(manifest.remove_tool("tool1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains("tool1"));

        // Removing non-existent tool returns false
        assert!(!manifest.remove_tool("nonexistent"));
    }

    #[test]
    fn test_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest").join("tools.yml");

        let original = ToolsManifest::with_tools(vec![
            "tool1".to_string(),
            "tool2".to_string(),
            "tool3".to_string(),
        ]);

        // Write
        original.write(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // Read
        let loaded = ToolsManifest::read(&manifest_path).unwrap();
        assert_eq!(loaded, original);
        assert_eq!(loaded.count(), 3);
    }

    #[test]
    fn test_yaml_format() {
        let manifest = ToolsManifest::with_tools(vec!["tool1".to_string(), "tool2".to_string()]);
        let yaml = serde_yaml::to_string(&manifest).unwrap();

        assert!(yaml.contains("tools:"));
        assert!(yaml.contains("tool1"));
        assert!(yaml.contains("tool2"));
    }
}
