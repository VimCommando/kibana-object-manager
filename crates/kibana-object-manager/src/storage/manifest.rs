//! Manifest directory management

use eyre::Result;
use std::path::{Path, PathBuf};

/// Manages a manifest directory containing multiple API manifests
pub struct ManifestDirectory {
    path: PathBuf,
}

impl ManifestDirectory {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// Get path to specific API manifest
    pub fn api_manifest_path(&self, api_name: &str, extension: &str) -> PathBuf {
        self.path.join(format!("{}.{}", api_name, extension))
    }

    /// Check if API manifest exists
    pub fn has_manifest(&self, api_name: &str, extension: &str) -> bool {
        self.api_manifest_path(api_name, extension).exists()
    }

    /// List all manifest files
    pub fn list_manifests(&self) -> Result<Vec<String>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut manifests = Vec::new();

        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(ext) = path.extension().and_then(|s| s.to_str())
                && (ext == "json" || ext == "yml" || ext == "yaml")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                manifests.push(name.to_string());
            }
        }

        Ok(manifests)
    }

    /// Get the directory path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_manifest_directory() {
        let temp = TempDir::new().unwrap();
        let manifest_dir = temp.path().join("manifest");

        let md = ManifestDirectory::new(&manifest_dir).unwrap();

        // Create some test manifest files
        std::fs::write(md.api_manifest_path("saved_objects", "json"), "{}").unwrap();
        std::fs::write(md.api_manifest_path("workflows", "yml"), "api: workflows").unwrap();

        assert!(md.has_manifest("saved_objects", "json"));
        assert!(md.has_manifest("workflows", "yml"));
        assert!(!md.has_manifest("agents", "yml"));

        let manifests = md.list_manifests().unwrap();
        assert_eq!(manifests.len(), 2);
        assert!(manifests.contains(&"saved_objects".to_string()));
        assert!(manifests.contains(&"workflows".to_string()));
    }
}
