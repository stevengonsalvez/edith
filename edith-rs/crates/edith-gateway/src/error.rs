//! Gateway error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Gateway errors
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("payload too large")]
    PayloadTooLarge,

    #[error("method not allowed")]
    MethodNotAllowed,

    #[error("internal error: {0}")]
    Internal(String),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("websocket error: {0}")]
    WebSocket(String),

    #[error("hook error: {0}")]
    Hook(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("configuration error: {0}")]
    Config(#[from] edith_config::ConfigError),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            GatewayError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            GatewayError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            GatewayError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            GatewayError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            GatewayError::PayloadTooLarge => {
                (StatusCode::PAYLOAD_TOO_LARGE, "payload too large".to_string())
            }
            GatewayError::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "method not allowed".to_string(),
            ),
            GatewayError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            GatewayError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            GatewayError::WebSocket(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            GatewayError::Hook(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            GatewayError::Session(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            GatewayError::Provider(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            GatewayError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "ok": false,
            "error": error_message
        }));

        (status, body).into_response()
    }
}

pub type GatewayResult<T> = Result<T, GatewayError>;
