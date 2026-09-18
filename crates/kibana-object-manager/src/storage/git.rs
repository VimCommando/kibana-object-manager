//! Git integration utilities

use eyre::Result;
use std::path::Path;

/// Manages .gitignore updates for kibob
pub struct GitIgnoreManager;

impl GitIgnoreManager {
    const MARKER_START: &'static str = "# Start --{kibob}-> (Kibana Object Manager)";
    const MARKER_END: &'static str = "# End --{kibob}->";

    /// Update .gitignore with kibob patterns
    pub fn update_gitignore(base_dir: &Path) -> Result<()> {
        let gitignore_path = base_dir.join(".gitignore");

        // Read existing or create new
        let mut content = if gitignore_path.exists() {
            std::fs::read_to_string(&gitignore_path)?
        } else {
            String::new()
        };

        // Check if already has kibob section
        if content.contains(Self::MARKER_START) {
            log::debug!(".gitignore already has kibob section");
            return Ok(());
        }

        // Add kibob section
        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }

        content.push_str(&format!(
            "{}\n.env*\nexport.ndjson\nimport.ndjson\n.import/\nresponse.json\nmanifest_patch.json\n{}\n",
            Self::MARKER_START,
            Self::MARKER_END
        ));

        std::fs::write(&gitignore_path, content)?;
        log::info!("Updated .gitignore");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_update_gitignore() {
        let temp = TempDir::new().unwrap();

        GitIgnoreManager::update_gitignore(temp.path()).unwrap();

        let gitignore = temp.path().join(".gitignore");
        assert!(gitignore.exists());

        let content = std::fs::read_to_string(&gitignore).unwrap();
        assert!(content.contains("--{kibob}->"));
        assert!(content.contains(".env*"));
        assert!(content.contains("export.ndjson"));

        // Test idempotence - running again shouldn't duplicate
        GitIgnoreManager::update_gitignore(temp.path()).unwrap();
        let content2 = std::fs::read_to_string(&gitignore).unwrap();
        assert_eq!(content, content2);
    }
}
