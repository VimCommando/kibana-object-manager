//! File system storage operations
//!
//! This module handles all file I/O operations including:
//! - NDJSON file reading/writing
//! - Directory-based object storage
//! - Manifest management
//! - Git integration

mod directory;
mod env;
mod git;
mod json_writer;
mod manifest;
mod ndjson;

pub use directory::{sanitize_filename, DirectoryReader, DirectoryWriter};
pub use env::transform_env_file;
pub use git::GitIgnoreManager;
pub use json_writer::{
    from_json5_str, read_json5_file, to_string_with_multiline, write_json_with_multiline,
};
pub use manifest::ManifestDirectory;
pub use ndjson::{NdjsonReader, NdjsonWriter};
