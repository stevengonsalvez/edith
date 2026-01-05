//! Session error types

use thiserror::Error;

/// Session errors
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session not found: {key}")]
    NotFound { key: String },

    #[error("session expired: {key}")]
    Expired { key: String },

    #[error("failed to load session store: {0}")]
    LoadStore(String),

    #[error("failed to save session store: {0}")]
    SaveStore(String),

    #[error("failed to read transcript: {0}")]
    ReadTranscript(String),

    #[error("failed to write transcript: {0}")]
    WriteTranscript(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid session key: {0}")]
    InvalidKey(String),
}

pub type SessionResult<T> = Result<T, SessionError>;
