//! Provider error types

use thiserror::Error;

/// Provider errors
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider not configured: {0}")]
    NotConfigured(String),

    #[error("provider not available: {0}")]
    NotAvailable(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("message send failed: {0}")]
    SendFailed(String),

    #[error("media upload failed: {0}")]
    MediaUploadFailed(String),

    #[error("media download failed: {0}")]
    MediaDownloadFailed(String),

    #[error("rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("invalid recipient: {0}")]
    InvalidRecipient(String),

    #[error("message too long: {len} > {max}")]
    MessageTooLong { len: usize, max: usize },

    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("bot blocked by user")]
    BotBlocked,

    #[error("chat not found: {0}")]
    ChatNotFound(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type ProviderResult<T> = Result<T, ProviderError>;
