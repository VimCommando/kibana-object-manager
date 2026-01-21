//! Saved Objects manifest management
//!
//! The saved objects manifest is stored as `manifest/saved_objects.json` and
//! is used directly as the payload for the Kibana export API.
//!
//! Example format:
//! ```json
//! {
//!   "objects": [
//!     {"type": "dashboard", "id": "my-dashboard-id"},
//!     {"type": "visualization", "id": "my-viz-id"}
//!   ],
//!   "excludeExportDetails": true,
//!   "includeReferencesDeep": true
//! }
//! ```

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Saved objects manifest structure
///
/// This structure doubles as both:
/// 1. The manifest file format (manifest/saved_objects.json)
/// 2. The request payload for POST /api/saved_objects/_export
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedObjectsManifest {
    /// List of objects to export/import
    pub objects: Vec<SavedObject>,

    /// Exclude export details from response (default: true)
    #[serde(default = "default_exclude_export_details")]
    pub exclude_export_details: bool,

    /// Include referenced objects recursively (default: true)
    #[serde(default = "default_include_references_deep")]
    pub include_references_deep: bool,
}

fn default_exclude_export_details() -> bool {
    true
}

fn default_include_references_deep() -> bool {
    true
}

/// A saved object reference (type + id)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SavedObject {
    /// Object type (e.g., "dashboard", "visualization", "index-pattern")
    #[serde(rename = "type")]
    pub object_type: String,

    /// Object ID
    pub id: String,
}

impl SavedObjectsManifest {
    /// Create a new empty manifest with default settings
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            exclude_export_details: true,
            include_references_deep: true,
        }
    }

    /// Create a manifest with specified objects
    pub fn with_objects(objects: Vec<SavedObject>) -> Self {
        Self {
            objects,
            exclude_export_details: true,
            include_references_deep: true,
        }
    }

    /// Add an object to the manifest
    pub fn add_object(&mut self, object: SavedObject) {
        if !self.objects.contains(&object) {
            self.objects.push(object);
        }
    }

    /// Remove an object from the manifest
    pub fn remove_object(&mut self, object_type: &str, id: &str) -> bool {
        if let Some(pos) = self
            .objects
            .iter()
            .position(|o| o.object_type == object_type && o.id == id)
        {
            self.objects.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if an object is in the manifest
    pub fn contains(&self, object_type: &str, id: &str) -> bool {
        self.objects
            .iter()
            .any(|o| o.object_type == object_type && o.id == id)
    }

    /// Get the number of objects in the manifest
    pub fn count(&self) -> usize {
        self.objects.len()
    }

    /// Sort objects by type then id
    pub fn sort(&mut self) {
        self.objects
            .sort_by(|a, b| a.object_type.cmp(&b.object_type).then(a.id.cmp(&b.id)));
    }

    /// Read manifest from JSON file
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).with_context(|| {
            format!(
                "Failed to read saved objects manifest: {}",
                path.as_ref().display()
            )
        })?;

        let manifest: Self = serde_json::from_str(&content)
            .with_context(|| "Failed to parse saved objects manifest JSON")?;

        Ok(manifest)
    }

    /// Write manifest to JSON file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)
            .with_context(|| "Failed to serialize saved objects manifest to JSON")?;

        std::fs::write(path.as_ref(), json).with_context(|| {
            format!(
                "Failed to write saved objects manifest: {}",
                path.as_ref().display()
            )
        })?;

        Ok(())
    }
}

impl Default for SavedObjectsManifest {
    fn default() -> Self {
        Self::new()
    }
}

impl SavedObject {
    /// Create a new saved object reference
    pub fn new(object_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            object_type: object_type.into(),
            id: id.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_manifest() {
        let manifest = SavedObjectsManifest::new();
        assert_eq!(manifest.count(), 0);
        assert!(manifest.exclude_export_details);
        assert!(manifest.include_references_deep);
    }

    #[test]
    fn test_with_objects() {
        let objects = vec![
            SavedObject::new("dashboard", "dash-1"),
            SavedObject::new("visualization", "viz-1"),
        ];
        let manifest = SavedObjectsManifest::with_objects(objects);
        assert_eq!(manifest.count(), 2);
        assert!(manifest.contains("dashboard", "dash-1"));
        assert!(manifest.contains("visualization", "viz-1"));
    }

    #[test]
    fn test_add_object() {
        let mut manifest = SavedObjectsManifest::new();
        manifest.add_object(SavedObject::new("dashboard", "test"));
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains("dashboard", "test"));

        // Adding duplicate should not increase count
        manifest.add_object(SavedObject::new("dashboard", "test"));
        assert_eq!(manifest.count(), 1);
    }

    #[test]
    fn test_remove_object() {
        let mut manifest = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("dashboard", "dash-1"),
            SavedObject::new("visualization", "viz-1"),
        ]);

        assert!(manifest.remove_object("dashboard", "dash-1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains("dashboard", "dash-1"));

        // Removing non-existent object returns false
        assert!(!manifest.remove_object("dashboard", "nonexistent"));
    }

    #[test]
    fn test_sort() {
        let mut manifest = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("visualization", "viz-2"),
            SavedObject::new("dashboard", "dash-2"),
            SavedObject::new("dashboard", "dash-1"),
            SavedObject::new("visualization", "viz-1"),
        ]);

        manifest.sort();

        assert_eq!(manifest.objects[0].object_type, "dashboard");
        assert_eq!(manifest.objects[0].id, "dash-1");
        assert_eq!(manifest.objects[1].object_type, "dashboard");
        assert_eq!(manifest.objects[1].id, "dash-2");
        assert_eq!(manifest.objects[2].object_type, "visualization");
        assert_eq!(manifest.objects[2].id, "viz-1");
    }

    #[test]
    fn test_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest").join("saved_objects.json");

        let mut original = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("dashboard", "dash-1"),
            SavedObject::new("visualization", "viz-1"),
            SavedObject::new("index-pattern", "idx-1"),
        ]);
        original.exclude_export_details = true;
        original.include_references_deep = false;

        // Write
        original.write(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // Read
        let loaded = SavedObjectsManifest::read(&manifest_path).unwrap();
        assert_eq!(loaded, original);
        assert_eq!(loaded.count(), 3);
        assert!(!loaded.include_references_deep);
    }

    #[test]
    fn test_json_format() {
        let manifest = SavedObjectsManifest::with_objects(vec![
            SavedObject::new("dashboard", "dash-1"),
            SavedObject::new("visualization", "viz-1"),
        ]);
        let json = serde_json::to_string_pretty(&manifest).unwrap();

        assert!(json.contains("\"objects\""));
        assert!(json.contains("\"type\""));
        assert!(json.contains("\"dashboard\""));
        assert!(json.contains("\"excludeExportDetails\""));
        assert!(json.contains("\"includeReferencesDeep\""));
    }
}
