//! Kibana Object Manager
//!
//! A Git-flavored ETL tool for managing Kibana objects

pub mod client;
pub mod etl;
pub mod storage;

// Phase 2 - will be removed
// Temporarily keep old modules commented out for reference
// mod processor;
// mod exporter;
// mod receiver;
// mod kibana_object_manager;

// Re-exports for convenience
pub use client::{Auth, AuthType, Kibana};
pub use etl::{Extractor, IdentityTransformer, Loader, Pipeline, Transformer};
pub use storage::{
    DirectoryReader, DirectoryWriter, GitIgnoreManager, ManifestDirectory, NdjsonReader,
    NdjsonWriter,
};
