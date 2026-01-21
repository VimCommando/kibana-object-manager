//! Extractor trait for data extraction from various sources

use eyre::Result;

/// Extractor trait for extracting data from a source
///
/// Implementors define how to extract items from sources like:
/// - Kibana APIs
/// - File systems
/// - Databases
///
/// # Example
/// ```no_run
/// use kibana_object_manager::etl::Extractor;
/// use eyre::Result;
/// use std::path::PathBuf;
///
/// struct FileExtractor {
///     path: PathBuf,
/// }
///
/// impl Extractor for FileExtractor {
///     type Item = String;
///     
///     async fn extract(&self) -> Result<Vec<Self::Item>> {
///         // Read files and return items
///         Ok(vec![])
///     }
/// }
/// ```
pub trait Extractor: Send + Sync {
    /// The type of items extracted
    type Item: Send;

    /// Extract items from the source
    ///
    /// # Errors
    /// Returns an error if extraction fails (network, I/O, parsing, etc.)
    fn extract(&self) -> impl std::future::Future<Output = Result<Vec<Self::Item>>> + Send;
}
