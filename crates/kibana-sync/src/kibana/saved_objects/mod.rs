//! Kibana Saved Objects API
//!
//! Provides extract and load operations for Kibana saved objects.
//! Manifest format: `manifest/saved_objects.json` (JSON - doubles as API payload)

mod extractor;
mod loader;
mod manifest;

pub use extractor::SavedObjectsExtractor;
pub use loader::SavedObjectsLoader;
pub use manifest::{SavedObject, SavedObjectsManifest};
