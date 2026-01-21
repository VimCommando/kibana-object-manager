//! Agents manifest management
//!
//! The agents manifest is stored as `manifest/agents.yml` and contains
//! a list of agents with their IDs and names.
//!
//! Example format:
//! ```yaml
//! agents:
//!   - id: agent-123
//!     name: my-agent
//!   - id: agent-456
//!     name: customer-support-agent
//!   - id: agent-789
//!     name: data-analysis-agent
//! ```

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Agent entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentEntry {
    /// Agent ID (used for API calls)
    pub id: String,
    /// Agent name (used for file storage)
    pub name: String,
}

impl AgentEntry {
    /// Create a new agent entry
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Agents manifest structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentsManifest {
    /// List of agents to manage
    pub agents: Vec<AgentEntry>,
}

impl AgentsManifest {
    /// Create a new empty agents manifest
    pub fn new() -> Self {
        Self { agents: Vec::new() }
    }

    /// Create a manifest with specified agents
    pub fn with_agents(agents: Vec<AgentEntry>) -> Self {
        Self { agents }
    }

    /// Add an agent to the manifest
    ///
    /// Returns true if agent was added, false if it already exists
    pub fn add_agent(&mut self, agent: AgentEntry) -> bool {
        // Check if agent with same ID already exists
        if !self.agents.iter().any(|a| a.id == agent.id) {
            self.agents.push(agent);
            true
        } else {
            false
        }
    }

    /// Remove an agent by ID from the manifest
    pub fn remove_agent_by_id(&mut self, agent_id: &str) -> bool {
        if let Some(pos) = self.agents.iter().position(|a| a.id == agent_id) {
            self.agents.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove an agent by name from the manifest
    pub fn remove_agent_by_name(&mut self, agent_name: &str) -> bool {
        if let Some(pos) = self.agents.iter().position(|a| a.name == agent_name) {
            self.agents.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if an agent ID is in the manifest
    pub fn contains_id(&self, agent_id: &str) -> bool {
        self.agents.iter().any(|a| a.id == agent_id)
    }

    /// Check if an agent name is in the manifest
    pub fn contains_name(&self, agent_name: &str) -> bool {
        self.agents.iter().any(|a| a.name == agent_name)
    }

    /// Get agent entry by ID
    pub fn get_by_id(&self, agent_id: &str) -> Option<&AgentEntry> {
        self.agents.iter().find(|a| a.id == agent_id)
    }

    /// Get agent entry by name
    pub fn get_by_name(&self, agent_name: &str) -> Option<&AgentEntry> {
        self.agents.iter().find(|a| a.name == agent_name)
    }

    /// Get the number of agents in the manifest
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Read manifest from YAML file
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).with_context(|| {
            format!(
                "Failed to read agents manifest: {}",
                path.as_ref().display()
            )
        })?;

        let manifest: Self = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse agents manifest YAML")?;

        Ok(manifest)
    }

    /// Write manifest to YAML file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize agents manifest to YAML")?;

        std::fs::write(path.as_ref(), yaml).with_context(|| {
            format!(
                "Failed to write agents manifest: {}",
                path.as_ref().display()
            )
        })?;

        Ok(())
    }
}

impl Default for AgentsManifest {
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
        let manifest = AgentsManifest::new();
        assert_eq!(manifest.count(), 0);
    }

    #[test]
    fn test_with_agents() {
        let manifest = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
        ]);
        assert_eq!(manifest.count(), 2);
        assert!(manifest.contains_id("agent1"));
        assert!(manifest.contains_name("agent1"));
        assert!(manifest.contains_id("agent2"));
        assert!(manifest.contains_name("agent2"));
    }

    #[test]
    fn test_add_agent() {
        let mut manifest = AgentsManifest::new();
        assert!(manifest.add_agent(AgentEntry::new("test-id", "test")));
        assert_eq!(manifest.count(), 1);
        assert!(manifest.contains_id("test-id"));
        assert!(manifest.contains_name("test"));

        // Adding duplicate should not increase count
        assert!(!manifest.add_agent(AgentEntry::new("test-id", "test")));
        assert_eq!(manifest.count(), 1);
    }

    #[test]
    fn test_remove_agent_by_id() {
        let mut manifest = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
        ]);

        assert!(manifest.remove_agent_by_id("agent1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains_id("agent1"));

        // Removing non-existent agent returns false
        assert!(!manifest.remove_agent_by_id("nonexistent"));
    }

    #[test]
    fn test_remove_agent_by_name() {
        let mut manifest = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
        ]);

        assert!(manifest.remove_agent_by_name("agent1"));
        assert_eq!(manifest.count(), 1);
        assert!(!manifest.contains_name("agent1"));

        // Removing non-existent agent returns false
        assert!(!manifest.remove_agent_by_name("nonexistent"));
    }

    #[test]
    fn test_get_by_id() {
        let manifest = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
        ]);

        let entry = manifest.get_by_id("agent1").unwrap();
        assert_eq!(entry.id, "agent1");
        assert_eq!(entry.name, "agent1");

        assert!(manifest.get_by_id("nonexistent").is_none());
    }

    #[test]
    fn test_get_by_name() {
        let manifest = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
        ]);

        let entry = manifest.get_by_name("agent1").unwrap();
        assert_eq!(entry.id, "agent1");
        assert_eq!(entry.name, "agent1");

        assert!(manifest.get_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest").join("agents.yml");

        let original = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
            AgentEntry::new("agent3", "agent3"),
        ]);

        // Write
        original.write(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // Read
        let loaded = AgentsManifest::read(&manifest_path).unwrap();
        assert_eq!(loaded, original);
        assert_eq!(loaded.count(), 3);
    }

    #[test]
    fn test_yaml_format() {
        let manifest = AgentsManifest::with_agents(vec![
            AgentEntry::new("agent1", "agent1"),
            AgentEntry::new("agent2", "agent2"),
        ]);
        let yaml = serde_yaml::to_string(&manifest).unwrap();

        assert!(yaml.contains("agents:"));
        assert!(yaml.contains("id: agent1"));
        assert!(yaml.contains("name: agent1"));
        assert!(yaml.contains("id: agent2"));
        assert!(yaml.contains("name: agent2"));
    }
}
