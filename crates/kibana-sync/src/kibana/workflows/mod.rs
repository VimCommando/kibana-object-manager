//! Kibana Workflows API
//!
//! Provides extract and load operations for Kibana workflows.
//! Manifest format: `manifest/workflows.yml` (YAML - human-readable list)

mod extractor;
mod loader;
mod manifest;

pub use extractor::WorkflowsExtractor;
pub use loader::WorkflowsLoader;
pub use manifest::{WorkflowEntry, WorkflowsManifest};

pub const WORKFLOW_CREATE_PATH: &str = "api/workflows/workflow";

pub fn workflow_resource_path(id: &str) -> String {
    format!("{WORKFLOW_CREATE_PATH}/{id}")
}
