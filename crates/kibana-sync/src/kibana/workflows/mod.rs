//! Kibana Workflows API
//!
//! Provides extract and load operations for Kibana workflows.
//! Manifest format: `manifest/workflows.yml` (YAML - human-readable list)

mod extractor;
mod loader;
mod manifest;

use crate::client::KibanaVersion;

pub use extractor::WorkflowsExtractor;
pub use loader::WorkflowsLoader;
pub use manifest::{WorkflowEntry, WorkflowsManifest};

/// Workflow create path used by Kibana 9.4 and later.
pub const WORKFLOW_CREATE_PATH: &str = "api/workflows/workflow";
/// Workflow create path used by Kibana 9.3.
pub const LEGACY_WORKFLOW_CREATE_PATH: &str = "api/workflows";

/// Build the Workflow item path used by Kibana 9.4 and later.
pub fn workflow_resource_path(id: &str) -> String {
    format!("{WORKFLOW_CREATE_PATH}/{id}")
}

/// Return whether the detected Kibana version uses the 9.4+ Workflow routes.
pub fn uses_current_workflow_routes(version: &KibanaVersion) -> bool {
    version.major > 9 || (version.major == 9 && version.minor >= 4)
}

/// Select the Workflow create path for the detected Kibana version.
pub fn workflow_create_path_for_version(version: &KibanaVersion) -> &'static str {
    if uses_current_workflow_routes(version) {
        WORKFLOW_CREATE_PATH
    } else {
        LEGACY_WORKFLOW_CREATE_PATH
    }
}

/// Build the Workflow item path for the detected Kibana version.
pub fn workflow_resource_path_for_version(version: &KibanaVersion, id: &str) -> String {
    format!("{}/{id}", workflow_create_path_for_version(version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::parse_kibana_version;

    #[test]
    fn selects_workflow_routes_by_minor_version() {
        let v93 = parse_kibana_version("9.3.3").unwrap();
        assert_eq!(
            workflow_create_path_for_version(&v93),
            LEGACY_WORKFLOW_CREATE_PATH
        );
        assert_eq!(
            workflow_resource_path_for_version(&v93, "workflow-a"),
            "api/workflows/workflow-a"
        );

        for version in ["9.4.0-SNAPSHOT", "9.4.2", "10.0.0"] {
            let version = parse_kibana_version(version).unwrap();
            assert_eq!(
                workflow_create_path_for_version(&version),
                WORKFLOW_CREATE_PATH
            );
            assert_eq!(
                workflow_resource_path_for_version(&version, "workflow-a"),
                "api/workflows/workflow/workflow-a"
            );
        }
    }
}
