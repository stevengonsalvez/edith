//! LLM error types

use thiserror::Error;

/// LLM errors
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("API key not configured")]
    MissingApiKey,

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("context length exceeded: {used} > {max}")]
    ContextLengthExceeded { used: u64, max: u64 },

    #[error("content filtered: {0}")]
    ContentFiltered(String),

    #[error("overloaded: {0}")]
    Overloaded(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("stream error: {0}")]
    StreamError(String),

    #[error("timeout after {0} seconds")]
    Timeout(u64),

    #[error("HTTP error: {status} - {message}")]
    Http { status: u16, message: String },

    #[error("connection error: {0}")]
    Connection(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl LlmError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LlmError::RateLimited { .. }
                | LlmError::Overloaded(_)
                | LlmError::Connection(_)
                | LlmError::Timeout(_)
        )
    }

    /// Get retry delay if applicable
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            LlmError::RateLimited { retry_after_secs } => Some(*retry_after_secs),
            LlmError::Overloaded(_) => Some(5),
            LlmError::Connection(_) | LlmError::Timeout(_) => Some(1),
            _ => None,
        }
    }
}

pub type LlmResult<T> = Result<T, LlmError>;
