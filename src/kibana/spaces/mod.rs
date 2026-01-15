//! Kibana Spaces API
//!
//! Provides extract and load operations for Kibana spaces.
//! Manifest format: `manifest/spaces.yml` (YAML - human-readable list)

mod extractor;
mod loader;
mod manifest;

pub use extractor::SpacesExtractor;
pub use loader::SpacesLoader;
pub use manifest::SpacesManifest;
