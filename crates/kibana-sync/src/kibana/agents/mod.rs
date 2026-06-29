//! Kibana Agents API
//!
//! Support for managing Kibana agents via /api/agent_builder/agents endpoints

mod extractor;
mod loader;
mod manifest;

pub use extractor::AgentsExtractor;
pub use loader::AgentsLoader;
pub use manifest::{AgentEntry, AgentsManifest};
