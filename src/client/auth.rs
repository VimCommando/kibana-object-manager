//! Authentication types for the Kibana client.

use clap::ValueEnum;
use std::str::FromStr;

/// Authentication credentials for Kibana.
pub enum Auth {
    /// Use an API key authentication via headers.
    Apikey(String),
    /// Use username and password authentication via Basic Auth headers.
    Basic(String, String),
    /// Don't use any authentication.
    None,
}

impl Auth {
    /// Create a new Auth instance based on the provided credentials and type.
    ///
    /// # Arguments
    /// * `type` - The desired authentication type
    /// * `username` - Optional username for Basic auth
    /// * `password` - Optional password for Basic auth
    /// * `apikey` - Optional API key for Apikey auth
    ///
    /// # Returns
    /// A new Auth instance. Defaults to Auth::None if required credentials for the type are missing.
    pub fn new(
        r#type: &AuthType,
        username: Option<String>,
        password: Option<String>,
        apikey: Option<String>,
    ) -> Self {
        match (r#type, username, password, apikey) {
            (AuthType::Apikey, _, _, Some(apikey)) => Self::Apikey(apikey),
            (AuthType::Basic, Some(username), Some(password), _) => Self::Basic(username, password),
            (AuthType::None, _, _, _) => Self::None,
            _ => Self::None,
        }
    }
}

impl std::fmt::Display for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Apikey(_) => write!(f, "Apikey"),
            Self::Basic(_, _) => write!(f, "Basic"),
            Self::None => write!(f, "None"),
        }
    }
}

/// Supported authentication types for the Kibana API.
#[derive(Clone, Debug, ValueEnum)]
pub enum AuthType {
    /// API Key authentication.
    Apikey,
    /// Basic authentication (Username/Password).
    Basic,
    /// No authentication.
    None,
}

impl FromStr for AuthType {
    type Err = ();

    /// Parse an authentication type from a string.
    ///
    /// Supports "apikey", "basic", and "none" (case-insensitive).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "apikey" => Ok(Self::Apikey),
            "basic" => Ok(Self::Basic),
            "none" => Ok(Self::None),
            _ => Err(()),
        }
    }
}
