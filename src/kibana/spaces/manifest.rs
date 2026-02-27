//! Spaces manifest management
//!
//! The spaces manifest is stored as `manifest/spaces.yml` and contains
//! a list of spaces with their IDs and names.
//!
//! Example format:
//! ```yaml
//! kibana:
//!   version: 9.3.2
//! spaces:
//!   - id: default
//!     name: Default
//!   - id: marketing
//!     name: Marketing
//!   - id: engineering
//!     name: Engineering
//! ```

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Space entry in manifest with ID and name
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceEntry {
    /// Space ID (used for API calls)
    pub id: String,
    /// Space name (used for filename)
    pub name: String,
}

impl SpaceEntry {
    /// Create a new space entry
    pub fn new(id: String, name: String) -> Self {
        Self { id, name }
    }
}

/// Kibana metadata captured for this project
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KibanaMetadata {
    /// Full Kibana version string from `/api/status` (e.g., "9.3.2")
    pub version: String,
}

impl KibanaMetadata {
    /// Create metadata with a specific version string
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

/// Spaces manifest structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpacesManifest {
    /// Optional Kibana metadata for version compatibility checks
    #[serde(default)]
    pub kibana: Option<KibanaMetadata>,
    /// List of spaces to manage (with ID and name)
    #[serde(default)]
    pub spaces: Vec<SpaceEntry>,
}

impl SpacesManifest {
    /// Create a new empty spaces manifest
    pub fn new() -> Self {
        Self {
            kibana: None,
            spaces: Vec::new(),
        }
    }

    /// Create a manifest with specified spaces
    pub fn with_spaces(spaces: Vec<SpaceEntry>) -> Self {
        Self {
            kibana: None,
            spaces,
        }
    }

    /// Set Kibana version metadata
    pub fn set_kibana_version(&mut self, version: impl Into<String>) {
        self.kibana = Some(KibanaMetadata::new(version.into()));
    }

    /// Get Kibana version metadata if present
    pub fn kibana_version(&self) -> Option<&str> {
        self.kibana.as_ref().map(|k| k.version.as_str())
    }

    /// Add a space to the manifest
    ///
    /// Returns true if space was added, false if it already exists
    pub fn add_space(&mut self, id: String, name: String) -> bool {
        if !self.spaces.iter().any(|s| s.id == id) {
            self.spaces.push(SpaceEntry::new(id, name));
            true
        } else {
            false
        }
    }

    /// Remove a space from the manifest by ID
    pub fn remove_space(&mut self, space_id: &str) -> bool {
        if let Some(pos) = self.spaces.iter().position(|s| s.id == space_id) {
            self.spaces.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if a space ID is in the manifest
    pub fn contains(&self, space_id: &str) -> bool {
        self.spaces.iter().any(|s| s.id == space_id)
    }

    /// Get all space IDs
    pub fn ids(&self) -> Vec<String> {
        self.spaces.iter().map(|s| s.id.clone()).collect()
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
        let manifest = SpacesManifest::with_spaces(vec![
            SpaceEntry::new("default".to_string(), "Default".to_string()),
            SpaceEntry::new("marketing".to_string(), "Marketing".to_string()),
        ]);
        assert_eq!(manifest.kibana_version(), None);
        assert_eq!(manifest.count(), 2);
        assert!(manifest.contains("default"));
        assert!(manifest.contains("marketing"));
    }

    #[test]
    fn test_add_space() {
        let mut manifest = SpacesManifest::new();
        manifest.add_space("test".to_string(), "Test".to_string());
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains("test"));

        // Adding duplicate should not increase count
        manifest.add_space("test".to_string(), "Test".to_string());
        assert_eq!(manifest.count(), 1);
    }

    #[test]
    fn test_remove_space() {
        let mut manifest = SpacesManifest::with_spaces(vec![
            SpaceEntry::new("space1".to_string(), "Space 1".to_string()),
            SpaceEntry::new("space2".to_string(), "Space 2".to_string()),
        ]);

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
            SpaceEntry::new("default".to_string(), "Default".to_string()),
            SpaceEntry::new("marketing".to_string(), "Marketing".to_string()),
            SpaceEntry::new("engineering".to_string(), "Engineering".to_string()),
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
        let mut manifest = SpacesManifest::with_spaces(vec![
            SpaceEntry::new("space1".to_string(), "Space 1".to_string()),
            SpaceEntry::new("space2".to_string(), "Space 2".to_string()),
        ]);
        manifest.set_kibana_version("9.3.2");
        let yaml = serde_yaml::to_string(&manifest).unwrap();

        assert!(yaml.contains("kibana:"));
        assert!(yaml.contains("version: 9.3.2"));
        assert!(yaml.contains("spaces:"));
        assert!(yaml.contains("id: space1"));
        assert!(yaml.contains("name: Space 1"));
        assert!(yaml.contains("id: space2"));
        assert!(yaml.contains("name: Space 2"));
    }

    #[test]
    fn test_backward_compatible_without_kibana_metadata() {
        let yaml = "spaces:\n  - id: default\n    name: Default\n";
        let manifest: SpacesManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.kibana_version(), None);
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains("default"));
    }
}
