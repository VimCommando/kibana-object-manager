//! Kibana API implementations
//!
//! This module provides ETL extractors and loaders for various Kibana APIs.
//! Each API has its own submodule with manifest format and operations.

pub mod saved_objects;
pub mod spaces;
pub mod workflows;

pub use saved_objects::{SavedObjectsExtractor, SavedObjectsLoader};
pub use spaces::{SpacesExtractor, SpacesLoader};
pub use workflows::{WorkflowsExtractor, WorkflowsLoader};
