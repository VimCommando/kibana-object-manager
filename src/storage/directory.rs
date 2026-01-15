//! Directory-based object storage

use crate::etl::{Extractor, Loader};
use async_trait::async_trait;
use eyre::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Read JSON objects from a directory
pub struct DirectoryReader {
    path: PathBuf,
}

impl DirectoryReader {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Read all JSON files from directory
    pub fn read_all(&self) -> Result<Vec<Value>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut objects = Vec::new();

        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read file: {}", path.display()))?;

                let value: Value = serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse JSON: {}", path.display()))?;

                objects.push(value);
            }
        }

        Ok(objects)
    }

    /// Count JSON files in directory
    pub fn count(&self) -> Result<usize> {
        if !self.path.exists() {
            return Ok(0);
        }

        let count = std::fs::read_dir(&self.path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .count();

        Ok(count)
    }
}

// Implement Extractor trait for reading from directories
#[async_trait]
impl Extractor for DirectoryReader {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        self.read_all()
    }
}

/// Write JSON objects to a directory
pub struct DirectoryWriter {
    path: PathBuf,
}

impl DirectoryWriter {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// Write objects to directory (uses jsrmx for file naming)
    pub fn write_all(&self, items: &[Value]) -> Result<usize> {
        // For now, simple implementation
        // Phase 2 will use jsrmx for better file naming
        let mut count = 0;

        for item in items.iter() {
            // Extract type and id for filename
            let filename = self.generate_filename(item);
            let path = self.path.join(filename);

            let json = serde_json::to_string_pretty(item)?;
            std::fs::write(path, json)?;
            count += 1;
        }

        Ok(count)
    }

    fn generate_filename(&self, item: &Value) -> String {
        // Try to extract type and id
        let obj_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object");

        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or_else(|| {
            item.get("originId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        });

        format!("{}-{}.json", obj_type, id)
    }

    /// Clear all JSON files from directory
    pub fn clear(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                std::fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

// Implement Loader trait for writing to directories
#[async_trait]
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
    fn test_write_read() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path()).unwrap();

        let data = vec![
            json!({"type": "dashboard", "id": "abc123", "name": "My Dashboard"}),
            json!({"type": "visualization", "id": "def456", "name": "My Viz"}),
        ];

        let count = writer.write_all(&data).unwrap();
        assert_eq!(count, 2);

        let reader = DirectoryReader::new(temp.path());
        let read_data = reader.read_all().unwrap();

        assert_eq!(read_data.len(), 2);
        assert_eq!(reader.count().unwrap(), 2);
    }

    #[test]
    fn test_clear() {
        let temp = TempDir::new().unwrap();
        let writer = DirectoryWriter::new(temp.path()).unwrap();

        let data = vec![json!({"type": "test", "id": "1"})];
        writer.write_all(&data).unwrap();

        let reader = DirectoryReader::new(temp.path());
        assert_eq!(reader.count().unwrap(), 1);

        writer.clear().unwrap();
        assert_eq!(reader.count().unwrap(), 0);
    }
}
