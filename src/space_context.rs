//! Space context management for multi-space operations
//!
//! Handles loading spaces from manifest and determining which spaces to operate on

use crate::kibana::spaces::{SpaceEntry, SpacesManifest};
use eyre::{Context, Result};
use std::path::Path;

/// Context for multi-space operations
///
/// Manages which spaces are available and which should be operated on
#[derive(Debug, Clone)]
pub struct SpaceContext {
    /// All spaces defined in spaces.yml
    available_spaces: Vec<SpaceEntry>,
    /// Space IDs to operate on (filtered by --space flag)
    target_space_ids: Vec<String>,
    /// Whether spaces.yml exists
    has_spaces_manifest: bool,
}

impl SpaceContext {
    /// Load space context from project directory
    ///
    /// # Arguments
    /// * `project_dir` - Project root directory
    /// * `space_filter` - Optional comma-separated list of space IDs to filter
    ///
    /// # Behavior
    /// - If no spaces.yml exists → defaults to ["default"] space
    /// - If manifest exists → loads all spaces
    /// - If space_filter provided → filters to matching spaces only
    ///
    /// # Example
    /// ```no_run
    /// use kibana_object_manager::space_context::SpaceContext;
    /// use std::path::Path;
    ///
    /// # fn example() -> eyre::Result<()> {
    /// // Load all spaces from manifest
    /// let ctx = SpaceContext::load(Path::new("."), None)?;
    ///
    /// // Load only specific spaces
    /// let ctx = SpaceContext::load(Path::new("."), Some("marketing,engineering"))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load(project_dir: impl AsRef<Path>, space_filter: Option<&str>) -> Result<Self> {
        let project_dir = project_dir.as_ref();
        let spaces_manifest_path = project_dir.join("spaces.yml");

        let (available_spaces, has_spaces_manifest) = if spaces_manifest_path.exists() {
            log::debug!("Loading spaces from {}", spaces_manifest_path.display());
            let manifest = SpacesManifest::read(&spaces_manifest_path)
                .with_context(|| "Failed to load spaces manifest")?;
            (manifest.spaces, true)
        } else {
            log::debug!("No spaces manifest found, defaulting to 'default' space");
            (
                vec![SpaceEntry::new(
                    "default".to_string(),
                    "Default".to_string(),
                )],
                false,
            )
        };

        // Determine target spaces
        let target_space_ids = if let Some(filter) = space_filter {
            // Parse comma-separated list
            let requested: Vec<&str> = filter.split(',').map(|s| s.trim()).collect();

            // Filter available spaces to match requested
            let mut targets = Vec::new();
            for req in &requested {
                if available_spaces.iter().any(|s| s.id == *req) {
                    targets.push(req.to_string());
                } else {
                    log::warn!("Space '{}' not found in manifest, skipping", req);
                }
            }

            if targets.is_empty() && !requested.is_empty() {
                eyre::bail!(
                    "None of the requested spaces ({}) are defined in spaces.yml",
                    filter
                );
            }

            targets
        } else {
            // No filter - use all available spaces
            available_spaces.iter().map(|s| s.id.clone()).collect()
        };

        Ok(Self {
            available_spaces,
            target_space_ids,
            has_spaces_manifest,
        })
    }

    /// Get list of space IDs to operate on
    pub fn target_space_ids(&self) -> &[String] {
        &self.target_space_ids
    }

    /// Check if a space is managed (exists in manifest)
    pub fn is_space_managed(&self, space_id: &str) -> bool {
        self.available_spaces.iter().any(|s| s.id == space_id)
    }

    /// Get space name from ID
    pub fn get_space_name(&self, space_id: &str) -> Option<&str> {
        self.available_spaces
            .iter()
            .find(|s| s.id == space_id)
            .map(|s| s.name.as_str())
    }

    /// Get all available space entries
    pub fn available_spaces(&self) -> &[SpaceEntry] {
        &self.available_spaces
    }

    /// Check if spaces manifest exists
    pub fn has_spaces_manifest(&self) -> bool {
        self.has_spaces_manifest
    }

    /// Get default space context (single "default" space)
    pub fn default_only() -> Self {
        Self {
            available_spaces: vec![SpaceEntry::new(
                "default".to_string(),
                "Default".to_string(),
            )],
            target_space_ids: vec!["default".to_string()],
            has_spaces_manifest: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_no_manifest_defaults_to_default_space() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = SpaceContext::load(temp_dir.path(), None).unwrap();

        assert!(!ctx.has_spaces_manifest());
        assert_eq!(ctx.target_space_ids(), &["default"]);
        assert!(ctx.is_space_managed("default"));
        assert_eq!(ctx.get_space_name("default"), Some("Default"));
    }

    #[test]
    fn test_load_with_manifest() {
        let temp_dir = TempDir::new().unwrap();

        let manifest = SpacesManifest::with_spaces(vec![
            SpaceEntry::new("default".to_string(), "Default".to_string()),
            SpaceEntry::new("marketing".to_string(), "Marketing".to_string()),
            SpaceEntry::new("engineering".to_string(), "Engineering".to_string()),
        ]);
        manifest.write(&temp_dir.path().join("spaces.yml")).unwrap();

        let ctx = SpaceContext::load(temp_dir.path(), None).unwrap();

        assert!(ctx.has_spaces_manifest());
        assert_eq!(ctx.target_space_ids().len(), 3);
        assert!(ctx.is_space_managed("marketing"));
        assert_eq!(ctx.get_space_name("marketing"), Some("Marketing"));
    }

    #[test]
    fn test_space_filter() {
        let temp_dir = TempDir::new().unwrap();

        let manifest = SpacesManifest::with_spaces(vec![
            SpaceEntry::new("default".to_string(), "Default".to_string()),
            SpaceEntry::new("marketing".to_string(), "Marketing".to_string()),
            SpaceEntry::new("engineering".to_string(), "Engineering".to_string()),
        ]);
        manifest.write(&temp_dir.path().join("spaces.yml")).unwrap();

        let ctx = SpaceContext::load(temp_dir.path(), Some("marketing,engineering")).unwrap();

        assert_eq!(ctx.target_space_ids().len(), 2);
        assert_eq!(ctx.target_space_ids(), &["marketing", "engineering"]);
        assert!(!ctx.target_space_ids().contains(&"default".to_string()));
    }

    #[test]
    fn test_filter_nonexistent_space() {
        let temp_dir = TempDir::new().unwrap();

        let manifest = SpacesManifest::with_spaces(vec![SpaceEntry::new(
            "default".to_string(),
            "Default".to_string(),
        )]);
        manifest.write(&temp_dir.path().join("spaces.yml")).unwrap();

        // Requesting only nonexistent spaces should error
        let result = SpaceContext::load(temp_dir.path(), Some("nonexistent"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("None of the requested spaces") || err_msg.contains("not defined")
        );
    }

    #[test]
    fn test_default_only() {
        let ctx = SpaceContext::default_only();

        assert!(!ctx.has_spaces_manifest());
        assert_eq!(ctx.target_space_ids(), &["default"]);
        assert!(ctx.is_space_managed("default"));
    }
}
