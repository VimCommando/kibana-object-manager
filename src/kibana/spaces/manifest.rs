//! Spaces manifest management
//!
//! The spaces manifest is stored as `manifest/spaces.yml` and contains
//! a list of space IDs to manage.
//!
//! Example format:
//! ```yaml
//! spaces:
//!   - default
//!   - marketing
//!   - engineering
//! ```

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Spaces manifest structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpacesManifest {
    /// List of space IDs to manage
    pub spaces: Vec<String>,
}

impl SpacesManifest {
    /// Create a new empty spaces manifest
    pub fn new() -> Self {
        Self { spaces: Vec::new() }
    }

    /// Create a manifest with specified space IDs
    pub fn with_spaces(spaces: Vec<String>) -> Self {
        Self { spaces }
    }

    /// Add a space ID to the manifest
    pub fn add_space(&mut self, space_id: String) {
        if !self.spaces.contains(&space_id) {
            self.spaces.push(space_id);
        }
    }

    /// Remove a space ID from the manifest
    pub fn remove_space(&mut self, space_id: &str) -> bool {
        if let Some(pos) = self.spaces.iter().position(|s| s == space_id) {
            self.spaces.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if a space ID is in the manifest
    pub fn contains(&self, space_id: &str) -> bool {
        self.spaces.contains(&space_id.to_string())
    }

    /// Get the number of spaces in the manifest
    pub fn count(&self) -> usize {
        self.spaces.len()
    }

    /// Read manifest from YAML file
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).with_context(|| {
            format!(
                "Failed to read spaces manifest: {}",
                path.as_ref().display()
            )
        })?;

        let manifest: Self = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse spaces manifest YAML")?;

        Ok(manifest)
    }

    /// Write manifest to YAML file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize spaces manifest to YAML")?;

        std::fs::write(path.as_ref(), yaml).with_context(|| {
            format!(
                "Failed to write spaces manifest: {}",
                path.as_ref().display()
            )
        })?;

        Ok(())
    }
}

impl Default for SpacesManifest {
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
        let manifest = SpacesManifest::new();
        assert_eq!(manifest.count(), 0);
    }

    #[test]
    fn test_with_spaces() {
        let manifest =
            SpacesManifest::with_spaces(vec!["default".to_string(), "marketing".to_string()]);
        assert_eq!(manifest.count(), 2);
        assert!(manifest.contains("default"));
        assert!(manifest.contains("marketing"));
    }

    #[test]
    fn test_add_space() {
        let mut manifest = SpacesManifest::new();
        manifest.add_space("test".to_string());
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains("test"));

        // Adding duplicate should not increase count
        manifest.add_space("test".to_string());
        assert_eq!(manifest.count(), 1);
    }

    #[test]
    fn test_remove_space() {
        let mut manifest =
            SpacesManifest::with_spaces(vec!["space1".to_string(), "space2".to_string()]);

        assert!(manifest.remove_space("space1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains("space1"));

        // Removing non-existent space returns false
        assert!(!manifest.remove_space("nonexistent"));
    }

    #[test]
    fn test_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest").join("spaces.yml");

        let original = SpacesManifest::with_spaces(vec![
            "default".to_string(),
            "marketing".to_string(),
            "engineering".to_string(),
        ]);

        // Write
        original.write(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // Read
        let loaded = SpacesManifest::read(&manifest_path).unwrap();
        assert_eq!(loaded, original);
        assert_eq!(loaded.count(), 3);
    }

    #[test]
    fn test_yaml_format() {
        let manifest =
            SpacesManifest::with_spaces(vec!["space1".to_string(), "space2".to_string()]);
        let yaml = serde_yaml::to_string(&manifest).unwrap();

        assert!(yaml.contains("spaces:"));
        assert!(yaml.contains("space1"));
        assert!(yaml.contains("space2"));
    }
}
