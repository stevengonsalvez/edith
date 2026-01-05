//! Media error types

use thiserror::Error;

/// Media processing errors
#[derive(Debug, Error)]
pub enum MediaError {
    #[error("transcription failed: {0}")]
    TranscriptionFailed(String),

    #[error("download failed: {0}")]
    DownloadFailed(String),

    #[error("invalid media type: {0}")]
    InvalidMediaType(String),

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("processing timeout")]
    Timeout,

    #[error("command failed: {0}")]
    CommandFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

pub type MediaResult<T> = Result<T, MediaError>;
