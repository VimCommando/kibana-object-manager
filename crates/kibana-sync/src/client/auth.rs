//! Authentication types for the Kibana client.

/// Authentication credentials for Kibana.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Auth {
    /// Use an API key authentication via headers.
    Apikey(String),
    /// Use username and password authentication via Basic Auth headers.
    Basic(String, String),
    /// Don't use any authentication.
    None,
}

impl Auth {
    /// Create API key authentication.
    pub fn api_key(apikey: impl Into<String>) -> Self {
        Self::Apikey(apikey.into())
    }

    /// Create Basic authentication.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic(username.into(), password.into())
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
