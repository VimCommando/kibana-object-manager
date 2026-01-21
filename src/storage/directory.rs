//! Directory-based object storage

use crate::etl::{Extractor, Loader};
use crate::storage::{from_json5_str, to_string_with_multiline};

use eyre::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Read JSON objects from a directory
///
/// Reads JSON files from a directory structure, supporting both:
/// - Flat structure: `dashboard-abc123.json`
/// - Hierarchical structure: `dashboard/abc123.json`
pub struct DirectoryReader {
    path: PathBuf,
}

impl DirectoryReader {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Read all JSON files from directory (recursively)
    pub fn read_all(&self) -> Result<Vec<Value>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut objects = Vec::new();
        self.read_recursive(&self.path, &mut objects)?;
        Ok(objects)
    }

    fn read_recursive(&self, dir: &Path, objects: &mut Vec<Value>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively read subdirectories
                self.read_recursive(&path, objects)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read file: {}", path.display()))?;

                // Use from_json5_str to support triple-quoted strings
                let value: Value = from_json5_str(&content)
                    .with_context(|| format!("Failed to parse JSON: {}", path.display()))?;

                objects.push(value);
            }
        }
        Ok(())
    }

    /// Count JSON files in directory (recursively)
    pub fn count(&self) -> Result<usize> {
        if !self.path.exists() {
            return Ok(0);
        }

        Ok(self.count_recursive(&self.path)?)
    }

    fn count_recursive(&self, dir: &Path) -> Result<usize> {
        let mut count = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                count += self.count_recursive(&path)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                count += 1;
            }
        }
        Ok(count)
    }
}

// Implement Extractor trait for reading from directories

impl Extractor for DirectoryReader {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        self.read_all()
    }
}

/// Write JSON objects to a directory
///
/// Supports two organization modes:
/// - Hierarchical (default): `dashboard/abc123.json`
/// - Flat: `dashboard-abc123.json`
pub struct DirectoryWriter {
    path: PathBuf,
    hierarchical: bool,
    filename_fields: Vec<String>,
}

impl DirectoryWriter {
    /// Create a new directory writer with hierarchical organization
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        Self::new_with_options(path, true)
    }

    /// Create a new directory writer with custom options
    ///
    /// # Arguments
    /// * `path` - Base directory path
    /// * `hierarchical` - If true, organize as `type/id.json`, otherwise `type-id.json`
    pub fn new_with_options(path: impl AsRef<Path>, hierarchical: bool) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            path,
            hierarchical,
            filename_fields: vec![
                "attributes.title".to_string(),
                "attributes.name".to_string(),
            ],
        })
    }

    /// Set the fields to use for filename generation (in order of preference)
    pub fn with_filename_fields(mut self, fields: Vec<&str>) -> Self {
        self.filename_fields = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Write objects to directory
    pub fn write_all(&self, items: &[Value]) -> Result<usize> {
        let mut count = 0;

        for item in items.iter() {
            let path = if self.hierarchical {
                self.generate_hierarchical_path(item)?
            } else {
                self.generate_flat_path(item)
            };

            // Create parent directory if needed
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let json = to_string_with_multiline(item)?;
            std::fs::write(path, json)?;
            count += 1;
        }

        Ok(count)
    }

    fn generate_hierarchical_path(&self, item: &Value) -> Result<PathBuf> {
        let obj_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object");

        let filename = self.generate_filename_from_fields(item);

        Ok(self.path.join(obj_type).join(format!("{}.json", filename)))
    }

    fn generate_flat_path(&self, item: &Value) -> PathBuf {
        let obj_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object");

        let filename = self.generate_filename_from_fields(item);

        self.path.join(format!("{}-{}.json", obj_type, filename))
    }

    fn generate_filename_from_fields(&self, item: &Value) -> String {
        // Try to get filename from configured fields
        for field_path in &self.filename_fields {
            if let Some(name) = self.get_nested_string(item, field_path) {
                return self.sanitize_filename(&name);
            }
        }

        // Fallback to id or originId
        if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
            return self.sanitize_filename(id);
        }

        if let Some(origin_id) = item.get("originId").and_then(|v| v.as_str()) {
            return self.sanitize_filename(origin_id);
        }

        // Last resort
        "unknown".to_string()
    }

    fn get_nested_string(&self, obj: &Value, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in parts {
            current = current.get(part)?;
        }

        current.as_str().map(|s| s.to_string())
    }

    fn sanitize_filename(&self, name: &str) -> String {
        // Replace characters that are problematic in filenames
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c => c,
            })
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Clear all JSON files from directory (recursively)
    pub fn clear(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        self.clear_recursive(&self.path)
    }

    fn clear_recursive(&self, dir: &Path) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.clear_recursive(&path)?;
                // Try to remove empty directory
                let _ = std::fs::remove_dir(&path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                std::fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

// Implement Loader trait for writing to directories

impl Loader for DirectoryWriter {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        self.write_all(&items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_write_read_hierarchical() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path()).unwrap();

        let data = vec![
            json!({"type": "dashboard", "id": "abc123", "attributes": {"title": "My Dashboard"}}),
            json!({"type": "visualization", "id": "def456", "attributes": {"name": "My Viz"}}),
        ];

        let count = writer.write_all(&data).unwrap();
        assert_eq!(count, 2);

        // Verify hierarchical structure
        assert!(
            temp.path()
                .join("dashboard")
                .join("My Dashboard.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("visualization")
                .join("My Viz.json")
                .exists()
        );

        let reader = DirectoryReader::new(temp.path());
        let read_data = reader.read_all().unwrap();

        assert_eq!(read_data.len(), 2);
        assert_eq!(reader.count().unwrap(), 2);
    }

    #[test]
    fn test_write_read_flat() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new_with_options(temp.path(), false).unwrap();

        let data = vec![
            json!({"type": "dashboard", "id": "abc123", "attributes": {"title": "My Dashboard"}}),
            json!({"type": "visualization", "id": "def456", "attributes": {"name": "My Viz"}}),
        ];

        let count = writer.write_all(&data).unwrap();
        assert_eq!(count, 2);

        // Verify flat structure
        assert!(temp.path().join("dashboard-My Dashboard.json").exists());
        assert!(temp.path().join("visualization-My Viz.json").exists());

        let reader = DirectoryReader::new(temp.path());
        let read_data = reader.read_all().unwrap();

        assert_eq!(read_data.len(), 2);
        assert_eq!(reader.count().unwrap(), 2);
    }

    #[test]
    fn test_custom_filename_fields() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path())
            .unwrap()
            .with_filename_fields(vec!["attributes.custom_name", "id"]);

        let data = vec![json!({
            "type": "dashboard",
            "id": "fallback-id",
            "attributes": {"custom_name": "Custom Name"}
        })];

        writer.write_all(&data).unwrap();
        assert!(
            temp.path()
                .join("dashboard")
                .join("Custom Name.json")
                .exists()
        );
    }

    #[test]
    fn test_sanitize_filename() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path()).unwrap();

        let data = vec![json!({
            "type": "dashboard",
            "id": "test",
            "attributes": {"title": "Bad/Name:With*Special?Chars"}
        })];

        writer.write_all(&data).unwrap();
        assert!(
            temp.path()
                .join("dashboard")
                .join("Bad_Name_With_Special_Chars.json")
                .exists()
        );
    }

    #[test]
    fn test_clear_hierarchical() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path()).unwrap();

        let data = vec![
            json!({"type": "dashboard", "id": "1", "attributes": {"title": "Test 1"}}),
            json!({"type": "visualization", "id": "2", "attributes": {"title": "Test 2"}}),
        ];
        writer.write_all(&data).unwrap();

        let reader = DirectoryReader::new(temp.path());
        assert_eq!(reader.count().unwrap(), 2);

        writer.clear().unwrap();
        assert_eq!(reader.count().unwrap(), 0);
    }

    #[test]
    fn test_fallback_to_id() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path()).unwrap();

        // Object without title or name, should use id
        let data = vec![json!({"type": "test", "id": "my-id-123"})];

        writer.write_all(&data).unwrap();
        assert!(temp.path().join("test").join("my-id-123.json").exists());
    }
}
