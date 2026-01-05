//! Gateway authentication middleware

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};

use crate::{GatewayError, GatewayState};

/// Extract token from request (header or query parameter)
pub fn extract_token<B>(request: &axum::http::Request<B>) -> Option<String> {
    // Try Authorization header first
    if let Some(auth) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(value) = auth.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
            if let Some(token) = value.strip_prefix("bearer ") {
                return Some(token.to_string());
            }
            // Also accept raw token
            return Some(value.to_string());
        }
    }

    // Try X-Token header
    if let Some(token) = request.headers().get("x-token") {
        if let Ok(value) = token.to_str() {
            return Some(value.to_string());
        }
    }

    // Try query parameter
    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            if let Some(token) = pair.strip_prefix("token=") {
                return Some(token.to_string());
            }
        }
    }

    None
}

/// Middleware to require hook authentication
pub async fn require_hook_auth(
    State(state): State<GatewayState>,
    request: Request,
    next: Next,
) -> Result<Response, GatewayError> {
    let token = extract_token(&request).ok_or_else(|| {
        GatewayError::Unauthorized("missing token".to_string())
    })?;

    if !state.check_hook_token(&token) {
        return Err(GatewayError::Unauthorized("invalid token".to_string()));
    }

    Ok(next.run(request).await)
}

/// Middleware to require gateway authentication
pub async fn require_gateway_auth(
    State(state): State<GatewayState>,
    request: Request,
    next: Next,
) -> Result<Response, GatewayError> {
    let config = state.config();

    // Check if auth is required
    let auth_config = config.gateway.as_ref().and_then(|g| g.auth.as_ref());
    if auth_config.is_none() {
        return Ok(next.run(request).await);
    }

    // Check for Tailscale identity if allowed
    if auth_config.and_then(|a| a.allow_tailscale).unwrap_or(false) {
        if let Some(user) = request.headers().get("tailscale-user") {
            if user.to_str().map_or(false, |u| !u.is_empty()) {
                return Ok(next.run(request).await);
            }
        }
    }

    // Check token
    let token = extract_token(&request).ok_or_else(|| {
        GatewayError::Unauthorized("missing credentials".to_string())
    })?;

    if !state.check_auth_token(&token) {
        return Err(GatewayError::Unauthorized("invalid credentials".to_string()));
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn test_extract_token_from_bearer_header() {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "Bearer test-token")
            .body(())
            .unwrap();

        assert_eq!(extract_token(&request), Some("test-token".to_string()));
    }

    #[test]
    fn test_extract_token_from_x_token_header() {
        let request = Request::builder()
            .header("x-token", "test-token")
            .body(())
            .unwrap();

        assert_eq!(extract_token(&request), Some("test-token".to_string()));
    }

    #[test]
    fn test_extract_token_from_query() {
        let request = Request::builder()
            .uri("/path?token=query-token&other=value")
            .body(())
            .unwrap();

        assert_eq!(extract_token(&request), Some("query-token".to_string()));
    }

    #[test]
    fn test_extract_token_none() {
        let request = Request::builder()
            .uri("/path")
            .body(())
            .unwrap();

        assert_eq!(extract_token(&request), None);
    }
}
