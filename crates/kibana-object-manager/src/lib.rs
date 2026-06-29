//! Kibana Object Manager
//!
//! A Git-flavored ETL tool for managing Kibana objects

pub mod cli;
pub mod migration;
pub mod storage;
pub mod transform;

pub use kibana_sync::{client, etl, kibana};

// Re-exports for convenience
pub use client::{
    ApiCapability, Auth, KibanaClient, KibanaVersion, KibanaVersionInfo, parse_kibana_version,
};
pub use etl::{Extractor, IdentityTransformer, Loader, Pipeline, Transformer};
pub use storage::{
    DirectoryReader, DirectoryWriter, GitIgnoreManager, ManifestDirectory, NdjsonReader,
    NdjsonWriter,
};
pub use transform::{FieldDropper, FieldEscaper, FieldUnescaper, ManagedFlagAdder};
