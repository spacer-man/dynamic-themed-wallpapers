use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Transport-level failure: DNS, TLS, timeout, connection reset.
    #[error("network error: {0}")]
    Network(String),

    /// The server answered, but not with 2xx.
    #[error("HTTP {status} from {url}")]
    Status { status: u16, url: String },

    /// The server answered 2xx, but the body is not what we expected.
    #[error("unexpected response from {what}: {msg}")]
    Parse { what: &'static str, msg: String },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("image error: {0}")]
    Image(String),

    #[error("storage error: {0}")]
    Store(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub(crate) fn parse(what: &'static str, msg: impl fmt::Display) -> Self {
        Error::Parse {
            what,
            msg: msg.to_string(),
        }
    }

    /// Worth retrying later (e.g. on the next scheduled run) rather than reporting to the user.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Network(_) => true,
            Error::Status { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }
}
