//! Workflows API extractor
//!
//! Extracts workflow definitions from Kibana via GET /api/workflows/<id>

use crate::client::Kibana;
use crate::etl::Extractor;
use async_trait::async_trait;
use eyre::{Context, Result};
use serde_json::Value;

/// Extractor for Kibana workflows
///
/// Fetches workflows by ID from the manifest. If no manifest is provided,
/// you should use the search API to discover workflows first.
///
/// # Example
/// ```no_run
/// use kibana_object_manager::kibana::workflows::{WorkflowsExtractor, WorkflowsManifest, WorkflowEntry};
/// use kibana_object_manager::client::{Auth, Kibana};
/// use kibana_object_manager::etl::Extractor;
/// use url::Url;
///
/// # async fn example() -> eyre::Result<()> {
/// let url = Url::parse("http://localhost:5601")?;
/// let client = Kibana::try_new(url, Auth::None, None)?;
/// let manifest = WorkflowsManifest::with_workflows(vec![
///     WorkflowEntry::new("workflow-123", "my-workflow"),
///     WorkflowEntry::new("workflow-456", "alert-workflow")
/// ]);
///
/// let extractor = WorkflowsExtractor::new(client, Some(manifest));
/// let workflows = extractor.extract().await?;
/// # Ok(())
/// # }
/// ```
pub struct WorkflowsExtractor {
    client: Kibana,
    manifest: Option<super::WorkflowsManifest>,
}

impl WorkflowsExtractor {
    /// Create a new workflows extractor
    ///
    /// # Arguments
    /// * `client` - Kibana HTTP client
    /// * `manifest` - Manifest containing workflow IDs to extract
    pub fn new(client: Kibana, manifest: Option<super::WorkflowsManifest>) -> Self {
        Self { client, manifest }
    }

    /// Fetch a single workflow by ID from Kibana
    async fn fetch_workflow(&self, workflow_id: &str) -> Result<Value> {
        let path = format!("/api/workflows/{}", workflow_id);

        log::debug!("Fetching workflow '{}' from {}", workflow_id, path);

        let response = self
            .client
            .get_internal(&path)
            .await
            .with_context(|| format!("Failed to fetch workflow '{}'", workflow_id))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eyre::bail!(
                "Failed to fetch workflow '{}' ({}): {}",
                workflow_id,
                status,
                body
            );
        }

        let workflow: Value = response
            .json()
            .await
            .with_context(|| format!("Failed to parse workflow '{}' response", workflow_id))?;

        log::debug!("Fetched workflow: {}", workflow_id);

        Ok(workflow)
    }

    /// Fetch specific workflows by ID from manifest
    async fn fetch_manifest_workflows(
        &self,
        manifest: &super::WorkflowsManifest,
    ) -> Result<Vec<Value>> {
        let mut workflows = Vec::new();

        for entry in &manifest.workflows {
            match self.fetch_workflow(&entry.id).await {
                Ok(workflow) => workflows.push(workflow),
                Err(e) => {
                    log::warn!(
                        "Failed to fetch workflow '{}' (id: {}): {}",
                        entry.name,
                        entry.id,
                        e
                    );
                    // Continue with other workflows instead of failing completely
                }
            }
        }

        log::info!("Fetched {} workflow(s) from manifest", workflows.len());

        Ok(workflows)
    }
}

#[async_trait]
impl Extractor for WorkflowsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        let workflows = if let Some(manifest) = &self.manifest {
            // Fetch only workflows from manifest by ID
            self.fetch_manifest_workflows(manifest).await?
        } else {
            // No manifest provided - return empty list
            // Use search API separately to discover workflows
            log::warn!("No manifest provided - use search API to discover workflows");
            Vec::new()
        };

        log::info!(
            "Extracted {} workflow(s){}",
            workflows.len(),
            if self.manifest.is_some() {
                " (from manifest)"
            } else {
                ""
            }
        );

        Ok(workflows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Auth;
    use url::Url;

    #[test]
    fn test_extractor_creation() {
        let url = Url::parse("http://localhost:5601").unwrap();
        let client = Kibana::try_new(url, Auth::None, None).unwrap();
        let _extractor = WorkflowsExtractor::new(client, None);
    }
}
