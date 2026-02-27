//! Kibana Object Manager
//!
//! A Git-flavored ETL tool for managing Kibana objects

pub mod cli;
pub mod client;
pub mod etl;
pub mod kibana;
pub mod migration;
pub mod storage;
pub mod transform;

// Re-exports for convenience
pub use client::{ApiCapability, Auth, AuthType, KibanaClient, KibanaVersion, KibanaVersionInfo};
pub use etl::{Extractor, IdentityTransformer, Loader, Pipeline, Transformer};
pub use storage::{
    DirectoryReader, DirectoryWriter, GitIgnoreManager, ManifestDirectory, NdjsonReader,
    NdjsonWriter,
};
pub use transform::{FieldDropper, FieldEscaper, FieldUnescaper, ManagedFlagAdder};
