//! Skills manifest management.
//!
//! The skills manifest is stored as `manifest/skills.yml` and contains
//! the user-created Skills tracked for a space. Skill content remains in
//! `skills/<skill-directory>/SKILL.md`.

use crate::{Result, ResultContext};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
}

impl SkillEntry {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillsManifest {
    pub skills: Vec<SkillEntry>,
}

impl SkillsManifest {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    pub fn with_skills(skills: Vec<SkillEntry>) -> Self {
        Self { skills }
    }

    pub fn add_skill(&mut self, skill: SkillEntry) -> bool {
        if !self.skills.iter().any(|entry| entry.id == skill.id) {
            self.skills.push(skill);
            true
        } else {
            false
        }
    }

    pub fn contains_id(&self, skill_id: &str) -> bool {
        self.skills.iter().any(|entry| entry.id == skill_id)
    }

    pub fn count(&self) -> usize {
        self.skills.len()
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).with_context(|| {
            format!(
                "Failed to read skills manifest: {}",
                path.as_ref().display()
            )
        })?;

        yaml_serde::from_str(&content).with_context(|| "Failed to parse skills manifest YAML")
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let yaml = yaml_serde::to_string(self)
            .with_context(|| "Failed to serialize skills manifest to YAML")?;

        std::fs::write(path.as_ref(), yaml).with_context(|| {
            format!(
                "Failed to write skills manifest: {}",
                path.as_ref().display()
            )
        })
    }
}

impl Default for SkillsManifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn add_and_check_skills() {
        let mut manifest = SkillsManifest::new();

        assert!(manifest.add_skill(SkillEntry::new("skill-a", "Skill A")));
        assert!(!manifest.add_skill(SkillEntry::new("skill-a", "Skill A")));
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains_id("skill-a"));
    }

    #[test]
    fn read_write_manifest() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("manifest/skills.yml");
        let original = SkillsManifest::with_skills(vec![
            SkillEntry::new("skill-a", "Skill A"),
            SkillEntry::new("skill-b", "Skill B"),
        ]);

        original.write(&path).unwrap();
        let loaded = SkillsManifest::read(&path).unwrap();

        assert_eq!(loaded, original);
    }

    #[test]
    fn yaml_format() {
        let manifest = SkillsManifest::with_skills(vec![SkillEntry::new("skill-a", "Skill A")]);
        let yaml = yaml_serde::to_string(&manifest).unwrap();

        assert!(yaml.contains("skills:"));
        assert!(yaml.contains("id: skill-a"));
        assert!(yaml.contains("name: Skill A"));
    }
}
