//! .env file transformation utilities
//!
//! Provides functions to update .env files during migration, such as
//! uppercasing keys and commenting out deprecated variables.

use eyre::{Context, Result};
use std::fs;
use std::path::Path;

/// Transform a .env file for the new multi-space structure.
///
/// 1. UPPERCASES all variable names (keys).
/// 2. Comments out the `KIBANA_SPACE` line.
/// 3. Inserts a migration note above the `KIBANA_SPACE` line.
pub fn transform_env_file(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(());
    }

    log::info!("Transforming .env file: {}", path.display());

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read .env file: {}", path.display()))?;

    let mut new_lines = Vec::new();
    let migration_note =
        "# Commented out by Kibana Migrate, now use spaces.yml and space directories";

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines or comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            new_lines.push(line.to_string());
            continue;
        }

        // Parse key=value
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            let value = &trimmed[eq_pos + 1..];

            // Handle optional 'export ' prefix
            let (prefix, actual_key) = if key.to_lowercase().starts_with("export ") {
                ("export ", key[7..].trim())
            } else {
                ("", key)
            };

            let upper_actual_key = actual_key.to_uppercase();
            let upper_key = format!("{}{}", prefix, upper_actual_key);

            if upper_actual_key == "KIBANA_SPACE" {
                new_lines.push(migration_note.to_string());
                new_lines.push(format!("# {}={}", upper_key, value));
            } else {
                new_lines.push(format!("{}={}", upper_key, value));
            }
        } else {
            // Not a key=value pair, keep as is
            new_lines.push(line.to_string());
        }
    }

    let new_content = if content.ends_with('\n') {
        format!("{}\n", new_lines.join("\n"))
    } else {
        new_lines.join("\n")
    };

    fs::write(path, new_content)
        .with_context(|| format!("Failed to write transformed .env file: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_transform_env_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "kibana_url=http://localhost:5601\nkibana_user=admin\nkibana_space=marketing\n# Already a comment\n\nKEEP_ME=true";
        writeln!(temp_file, "{}", content).unwrap();

        transform_env_file(temp_file.path()).unwrap();

        let transformed = fs::read_to_string(temp_file.path()).unwrap();
        assert!(transformed.contains("KIBANA_URL=http://localhost:5601"));
        assert!(transformed.contains("KIBANA_USER=admin"));
        assert!(transformed.contains("# Commented out by Kibana Migrate"));
        assert!(transformed.contains("# KIBANA_SPACE=marketing"));
        assert!(transformed.contains("# Already a comment"));
        assert!(transformed.contains("KEEP_ME=true"));
    }

    #[test]
    fn test_transform_env_file_with_export() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "export kibana_url=http://localhost:5601\nexport kibana_space=marketing";
        writeln!(temp_file, "{}", content).unwrap();

        transform_env_file(temp_file.path()).unwrap();

        let transformed = fs::read_to_string(temp_file.path()).unwrap();
        assert!(transformed.contains("export KIBANA_URL=http://localhost:5601"));
        assert!(transformed.contains("# Commented out by Kibana Migrate"));
        assert!(transformed.contains("# export KIBANA_SPACE=marketing"));
    }
}
