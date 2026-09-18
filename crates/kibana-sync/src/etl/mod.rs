//! Core ETL (Extract, Transform, Load) abstractions
//!
//! This module provides trait definitions for building data pipelines
//! that extract data from sources, transform it, and load it to destinations.

mod extract;
mod load;
mod pipeline;
mod transform;

pub use extract::Extractor;
pub use load::Loader;
pub use pipeline::Pipeline;
pub use transform::{IdentityTransformer, Transformer};
