use reqwest::StatusCode;
use std::fmt;

/// Result type used by `kibana-client` public APIs.
pub type Result<T> = std::result::Result<T, Error>;

pub trait ResultContext<T> {
    fn context(self, message: impl Into<String>) -> Result<T>;
    fn with_context<M>(self, message: impl FnOnce() -> M) -> Result<T>
    where
        M: Into<String>;
}

impl<T, E> ResultContext<T> for std::result::Result<T, E>
where
    E: fmt::Display,
{
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.map_err(|err| Error::message(format!("{}: {err}", message.into())))
    }

    fn with_context<M>(self, message: impl FnOnce() -> M) -> Result<T>
    where
        M: Into<String>,
    {
        self.map_err(|err| Error::message(format!("{}: {err}", message().into())))
    }
}

/// Error type used by `kibana-client` public APIs.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    InvalidConfiguration(String),
    InvalidSpace {
        id: String,
        available: Vec<String>,
    },
    UnsupportedCapability {
        capability: &'static str,
        detected: semver::Version,
        minimum: semver::Version,
    },
    MissingResourceId {
        resource: &'static str,
    },
    MissingField {
        field: &'static str,
    },
    ApiResponse {
        status: StatusCode,
        body: String,
    },
    Transport(reqwest::Error),
    Url(url::ParseError),
    HeaderName(reqwest::header::InvalidHeaderName),
    HeaderValue(reqwest::header::InvalidHeaderValue),
    HeaderToStr(reqwest::header::ToStrError),
    Json(serde_json::Error),
    Yaml(serde_yaml::Error),
    Version(semver::Error),
    Io(std::io::Error),
    SemaphoreClosed,
    Message(String),
}

impl Error {
    pub fn api_response(status: StatusCode, body: impl Into<String>) -> Self {
        Self::ApiResponse {
            status,
            body: body.into(),
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => write!(f, "invalid configuration: {message}"),
            Self::InvalidSpace { id, available } => write!(
                f,
                "space '{id}' not found in registry. Available spaces: {}",
                available.join(", ")
            ),
            Self::UnsupportedCapability {
                capability,
                detected,
                minimum,
            } => write!(
                f,
                "API '{capability}' requires Kibana {minimum}+ (detected {detected})"
            ),
            Self::MissingResourceId { resource } => {
                write!(f, "{resource} is missing required 'id' field")
            }
            Self::MissingField { field } => write!(f, "missing required field: {field}"),
            Self::ApiResponse { status, body } => {
                write!(f, "Kibana API returned {status}: {body}")
            }
            Self::Transport(err) => write!(f, "transport error: {err}"),
            Self::Url(err) => write!(f, "URL error: {err}"),
            Self::HeaderName(err) => write!(f, "invalid header name: {err}"),
            Self::HeaderValue(err) => write!(f, "invalid header value: {err}"),
            Self::HeaderToStr(err) => write!(f, "invalid header value: {err}"),
            Self::Json(err) => write!(f, "JSON error: {err}"),
            Self::Yaml(err) => write!(f, "YAML error: {err}"),
            Self::Version(err) => write!(f, "version parse error: {err}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::SemaphoreClosed => write!(f, "request semaphore closed"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(err) => Some(err),
            Self::Url(err) => Some(err),
            Self::HeaderName(err) => Some(err),
            Self::HeaderValue(err) => Some(err),
            Self::HeaderToStr(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::Yaml(err) => Some(err),
            Self::Version(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Transport(err)
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Self::Url(err)
    }
}

impl From<reqwest::header::InvalidHeaderName> for Error {
    fn from(err: reqwest::header::InvalidHeaderName) -> Self {
        Self::HeaderName(err)
    }
}

impl From<reqwest::header::InvalidHeaderValue> for Error {
    fn from(err: reqwest::header::InvalidHeaderValue) -> Self {
        Self::HeaderValue(err)
    }
}

impl From<reqwest::header::ToStrError> for Error {
    fn from(err: reqwest::header::ToStrError) -> Self {
        Self::HeaderToStr(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Yaml(err)
    }
}

impl From<semver::Error> for Error {
    fn from(err: semver::Error) -> Self {
        Self::Version(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
