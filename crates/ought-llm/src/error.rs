//! Errors returned by [`Llm`](crate::Llm) implementations.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    /// Transport-level failure: TLS, DNS, timeout, etc.
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The provider returned a non-2xx HTTP response.
    #[error("provider returned status {status}: {message}")]
    Api { status: u16, message: String },

    /// Authentication failure (401/403, missing API key, malformed key).
    #[error("authentication error: {0}")]
    Auth(String),

    /// JSON serialization/deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Provider returned a 2xx response but its shape didn't match what
    /// we expect for this provider.
    #[error("invalid response from provider: {0}")]
    InvalidResponse(String),

    /// Catch-all for anything else (e.g. bad config).
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl LlmError {
    /// Whether the agent loop should retry after a backoff.
    ///
    /// True for transient transport failures, 429 rate-limit responses,
    /// and 5xx server errors. False for 4xx (other than 429), auth
    /// failures, serialization bugs, and invalid responses.
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::Http(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            LlmError::Api { status, .. } => *status == 429 || *status >= 500,
            LlmError::Auth(_) => false,
            LlmError::Serialization(_) => false,
            LlmError::InvalidResponse(_) => false,
            LlmError::Other(_) => false,
        }
    }
}
