//! Kibana API client and authentication.
//!
//! This module provides the [`KibanaClient`] for interacting with the Kibana API,
//! along with authentication type [`Auth`].

mod auth;
mod kibana;

pub use auth::Auth;
pub use kibana::{
    ApiCapability, KibanaClient, KibanaClientBuilder, KibanaVersion, KibanaVersionInfo,
    SpaceRegistry, parse_kibana_version,
};
