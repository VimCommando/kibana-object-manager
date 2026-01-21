//! NDJSON (Newline Delimited JSON) file operations

use crate::etl::{Extractor, Loader};

use eyre::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Read NDJSON from a file
pub struct NdjsonReader {
    path: std::path::PathBuf,
}

impl NdjsonReader {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Read all lines as JSON values
    pub fn read(&self) -> Result<Vec<Value>> {
        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read NDJSON file: {}", self.path.display()))?;

        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line)
                    .with_context(|| format!("Failed to parse JSON line: {}", line))
            })
            .collect()
    }

    /// Read specific number of lines
    pub fn read_lines(&self, count: usize) -> Result<Vec<Value>> {
        let content = std::fs::read_to_string(&self.path)?;

        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(count)
            .map(|line| serde_json::from_str(line).map_err(Into::into))
            .collect()
    }
}

// Implement Extractor trait for reading NDJSON files

impl Extractor for NdjsonReader {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        self.read()
    }
}

/// Write NDJSON to a file
pub struct NdjsonWriter {
    path: std::path::PathBuf,
}

impl NdjsonWriter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Write JSON values as NDJSON
    pub fn write(&self, items: &[Value]) -> Result<()> {
        let ndjson = items
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");

        // Add trailing newline
        let content = if ndjson.is_empty() {
            String::new()
        } else {
            format!("{}\n", ndjson)
        };

        std::fs::write(&self.path, content)
            .with_context(|| format!("Failed to write NDJSON file: {}", self.path.display()))?;

        Ok(())
    }

    /// Append items to existing NDJSON file
    pub fn append(&self, items: &[Value]) -> Result<()> {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        for item in items {
            writeln!(file, "{}", serde_json::to_string(item)?)?;
        }

        Ok(())
    }
}

// Implement Loader trait for writing NDJSON files

impl Loader for NdjsonWriter {
    type Item = Value;

    async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
        self.write(&items)?;
        Ok(items.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_write() {
        let temp = NamedTempFile::new().unwrap();
        let writer = NdjsonWriter::new(temp.path());

        let data = vec![json!({"a": 1}), json!({"b": 2})];
        writer.write(&data).unwrap();

        let reader = NdjsonReader::new(temp.path());
        let read_data = reader.read().unwrap();

        assert_eq!(data, read_data);
    }

    #[test]
    fn test_append() {
        let temp = NamedTempFile::new().unwrap();
        let writer = NdjsonWriter::new(temp.path());

        writer.write(&[json!({"a": 1})]).unwrap();
        writer.append(&[json!({"b": 2})]).unwrap();

        let reader = NdjsonReader::new(temp.path());
        let data = reader.read().unwrap();

        assert_eq!(data.len(), 2);
    }
}
