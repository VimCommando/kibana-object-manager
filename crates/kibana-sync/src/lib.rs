//! Reusable Kibana API client.
//!
//! This crate owns Kibana HTTP client configuration, authentication, version
//! and capability checks, space-aware request routing, endpoint modules,
//! dependency discovery, storage-neutral sync helpers, and explicit filesystem
//! bundle helpers. It does not discover `kibob` project roots or initialize
//! logging/tracing subscribers.

pub mod client;
pub mod error;
pub mod etl;
pub mod fs;
pub mod kibana;
pub mod sync;

#[cfg(test)]
pub(crate) mod test_support;

pub use client::{
    ApiCapability, Auth, KibanaClient, KibanaClientBuilder, KibanaVersion, KibanaVersionInfo,
    SpaceRegistry, parse_kibana_version,
};
pub use error::{Error, Result, ResultContext};
pub use etl::{Extractor, IdentityTransformer, Loader, Pipeline, Transformer};
pub use fs::KibanaFsBundle;
