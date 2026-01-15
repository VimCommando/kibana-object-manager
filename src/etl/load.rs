//! Loader trait for loading data to destinations

use async_trait::async_trait;
use eyre::Result;

/// Loader trait for loading data to a destination
///
/// Implementors define how to load items to destinations:
/// - Kibana APIs
/// - File systems
/// - Databases
///
/// # Example
/// ```no_run
/// use kibana_object_manager::etl::Loader;
/// use async_trait::async_trait;
/// use eyre::Result;
/// use std::path::PathBuf;
///
/// struct FileLoader {
///     output_dir: PathBuf,
/// }
///
/// #[async_trait]
/// impl Loader for FileLoader {
///     type Item = String;
///     
///     async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
///         // Write items to files
///         Ok(items.len())
///     }
/// }
/// ```
#[async_trait]
pub trait Loader: Send + Sync {
    /// The type of items to load
    type Item: Send;

    /// Load items to the destination
    ///
    /// Returns the number of items successfully loaded
    ///
    /// # Errors
    /// Returns an error if loading fails (network, I/O, validation, etc.)
    async fn load(&self, items: Vec<Self::Item>) -> Result<usize>;
}
