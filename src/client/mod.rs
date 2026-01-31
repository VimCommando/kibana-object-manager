//! Kibana API client and authentication.
//!
//! This module provides the [`KibanaClient`] for interacting with the Kibana API,
//! along with authentication types ([`Auth`], [`AuthType`]).

mod auth;
mod kibana;

pub use auth::{Auth, AuthType};
pub use kibana::KibanaClient;
