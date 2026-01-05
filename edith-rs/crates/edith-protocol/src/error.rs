//! Protocol error types
//!
//! Defines error codes and shapes matching Clawdis protocol errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Standard error codes used in the protocol
pub mod codes {
    /// WhatsApp not authenticated
    pub const NOT_LINKED: &str = "NOT_LINKED";

    /// Agent did not respond within deadline
    pub const AGENT_TIMEOUT: &str = "AGENT_TIMEOUT";

    /// Schema/param validation failed
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";

    /// Invalid parameters for method
    pub const INVALID_PARAMS: &str = "INVALID_PARAMS";

    /// Gateway shutting down or dependency unavailable
    pub const UNAVAILABLE: &str = "UNAVAILABLE";

    /// Rate limited - retry after delay
    pub const RATE_LIMITED: &str = "RATE_LIMITED";

    /// Method not found
    pub const METHOD_NOT_FOUND: &str = "METHOD_NOT_FOUND";

    /// Authentication required
    pub const UNAUTHORIZED: &str = "UNAUTHORIZED";

    /// Authentication required (alias)
    pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";

    /// Authentication failed
    pub const AUTH_FAILED: &str = "AUTH_FAILED";

    /// Permission denied
    pub const FORBIDDEN: &str = "FORBIDDEN";

    /// Resource not found
    pub const NOT_FOUND: &str = "NOT_FOUND";

    /// Internal server error
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";

    /// Provider error (Telegram, Discord, etc.)
    pub const PROVIDER_ERROR: &str = "PROVIDER_ERROR";

    /// Session not found
    pub const SESSION_NOT_FOUND: &str = "SESSION_NOT_FOUND";

    /// Agent already running
    pub const AGENT_BUSY: &str = "AGENT_BUSY";

    /// Node not found
    pub const NODE_NOT_FOUND: &str = "NODE_NOT_FOUND";

    /// Pairing failed
    pub const PAIRING_FAILED: &str = "PAIRING_FAILED";

    /// Invalid protocol version
    pub const PROTOCOL_MISMATCH: &str = "PROTOCOL_MISMATCH";

    /// Handshake timeout
    pub const HANDSHAKE_TIMEOUT: &str = "HANDSHAKE_TIMEOUT";

    /// General timeout
    pub const TIMEOUT: &str = "TIMEOUT";

    /// Connection closed
    pub const CONNECTION_CLOSED: &str = "CONNECTION_CLOSED";

    /// Payload too large
    pub const PAYLOAD_TOO_LARGE: &str = "PAYLOAD_TOO_LARGE";

    /// Duplicate request (idempotency)
    pub const DUPLICATE_REQUEST: &str = "DUPLICATE_REQUEST";
}

/// Error shape returned in response frames
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorShape {
    /// Error code (e.g., "NOT_LINKED", "AGENT_TIMEOUT")
    pub code: String,

    /// Human-readable error message
    pub message: String,

    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// Whether this error is retryable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,

    /// Suggested retry delay in milliseconds
    #[serde(rename = "retryAfterMs", skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl ErrorShape {
    /// Create a new error shape
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            retryable: None,
            retry_after_ms: None,
        }
    }

    /// Create an error with details
    pub fn with_details<T: Serialize>(
        code: impl Into<String>,
        message: impl Into<String>,
        details: T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            code: code.into(),
            message: message.into(),
            details: Some(serde_json::to_value(details)?),
            retryable: None,
            retry_after_ms: None,
        })
    }

    /// Mark this error as retryable
    pub fn retryable(mut self) -> Self {
        self.retryable = Some(true);
        self
    }

    /// Set retry delay
    pub fn retry_after(mut self, ms: u64) -> Self {
        self.retryable = Some(true);
        self.retry_after_ms = Some(ms);
        self
    }

    // Common error constructors

    /// WhatsApp not linked
    pub fn not_linked() -> Self {
        Self::new(codes::NOT_LINKED, "WhatsApp not authenticated")
    }

    /// Agent timeout
    pub fn agent_timeout() -> Self {
        Self::new(codes::AGENT_TIMEOUT, "Agent did not respond within deadline")
    }

    /// Invalid request
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_REQUEST, message)
    }

    /// Service unavailable
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(codes::UNAVAILABLE, message)
    }

    /// Rate limited
    pub fn rate_limited(retry_after_ms: u64) -> Self {
        Self::new(codes::RATE_LIMITED, "Rate limited, please retry later")
            .retry_after(retry_after_ms)
    }

    /// Method not found
    pub fn method_not_found(method: &str) -> Self {
        Self::new(codes::METHOD_NOT_FOUND, format!("Method not found: {}", method))
    }

    /// Unauthorized
    pub fn unauthorized() -> Self {
        Self::new(codes::UNAUTHORIZED, "Authentication required")
    }

    /// Forbidden
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(codes::FORBIDDEN, message)
    }

    /// Not found
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::new(codes::NOT_FOUND, format!("Not found: {}", resource.into()))
    }

    /// Internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(codes::INTERNAL_ERROR, message)
    }

    /// Provider error
    pub fn provider_error(provider: &str, message: impl Into<String>) -> Self {
        Self::new(
            codes::PROVIDER_ERROR,
            format!("{} error: {}", provider, message.into()),
        )
    }

    /// Session not found
    pub fn session_not_found(session_key: &str) -> Self {
        Self::new(
            codes::SESSION_NOT_FOUND,
            format!("Session not found: {}", session_key),
        )
    }

    /// Agent busy
    pub fn agent_busy(session_key: &str) -> Self {
        Self::new(
            codes::AGENT_BUSY,
            format!("Agent already running for session: {}", session_key),
        )
    }

    /// Protocol mismatch
    pub fn protocol_mismatch(server_version: u32, client_min: u32, client_max: u32) -> Self {
        Self::new(
            codes::PROTOCOL_MISMATCH,
            format!(
                "Protocol version mismatch: server={}, client=[{}-{}]",
                server_version, client_min, client_max
            ),
        )
    }

    /// Handshake timeout
    pub fn handshake_timeout() -> Self {
        Self::new(codes::HANDSHAKE_TIMEOUT, "Handshake timeout - connect first")
    }

    /// Payload too large
    pub fn payload_too_large(size: usize, max: usize) -> Self {
        Self::new(
            codes::PAYLOAD_TOO_LARGE,
            format!("Payload too large: {} bytes (max {})", size, max),
        )
    }
}

/// Protocol error enum for internal use
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid frame: {0}")]
    InvalidFrame(String),

    #[error("Handshake required")]
    HandshakeRequired,

    #[error("Protocol version mismatch")]
    ProtocolMismatch,

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout")]
    Timeout,
}

impl From<ProtocolError> for ErrorShape {
    fn from(err: ProtocolError) -> Self {
        match err {
            ProtocolError::Serialization(e) => {
                ErrorShape::invalid_request(format!("JSON error: {}", e))
            }
            ProtocolError::InvalidFrame(msg) => ErrorShape::invalid_request(msg),
            ProtocolError::HandshakeRequired => ErrorShape::handshake_timeout(),
            ProtocolError::ProtocolMismatch => {
                ErrorShape::new(codes::PROTOCOL_MISMATCH, "Protocol version mismatch")
            }
            ProtocolError::AuthenticationFailed => ErrorShape::unauthorized(),
            ProtocolError::MethodNotFound(method) => ErrorShape::method_not_found(&method),
            ProtocolError::InvalidParams(msg) => ErrorShape::invalid_request(msg),
            ProtocolError::ConnectionClosed => {
                ErrorShape::new(codes::CONNECTION_CLOSED, "Connection closed")
            }
            ProtocolError::Timeout => ErrorShape::agent_timeout(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_shape_serialization() {
        let error = ErrorShape::new("TEST_ERROR", "Test message");
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains(r#""code":"TEST_ERROR""#));
        assert!(json.contains(r#""message":"Test message""#));
    }

    #[test]
    fn test_rate_limited_error() {
        let error = ErrorShape::rate_limited(5000);
        assert_eq!(error.code, codes::RATE_LIMITED);
        assert_eq!(error.retryable, Some(true));
        assert_eq!(error.retry_after_ms, Some(5000));
    }

    #[test]
    fn test_error_with_details() {
        let error = ErrorShape::with_details(
            "VALIDATION_ERROR",
            "Invalid field",
            serde_json::json!({"field": "name", "reason": "too short"}),
        )
        .unwrap();

        assert!(error.details.is_some());
        let details = error.details.unwrap();
        assert_eq!(details["field"], "name");
    }
}
