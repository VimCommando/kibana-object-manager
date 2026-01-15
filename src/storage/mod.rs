//! File system storage operations
//!
//! This module handles all file I/O operations including:
//! - NDJSON file reading/writing
//! - Directory-based object storage
//! - Manifest management
//! - Git integration

mod directory;
mod git;
mod manifest;
mod ndjson;

pub use directory::{DirectoryReader, DirectoryWriter};
pub use git::GitIgnoreManager;
pub use manifest::ManifestDirectory;
pub use ndjson::{NdjsonReader, NdjsonWriter};
