//! Gateway router configuration

use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    auth::{require_gateway_auth, require_hook_auth},
    hooks::{handle_agent_hook, handle_custom_hook, handle_wake_hook},
    websocket::{ws_handler, WebSocketState},
    GatewayState,
};

/// Create the main gateway router
pub fn create_router(gateway_state: GatewayState) -> Router {
    let hooks_base = gateway_state.hooks_base_path();

    // Create WebSocket state with protocol handling
    let ws_state = WebSocketState::new(gateway_state.clone());

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Hooks routes (require hook auth)
    let hooks_router = Router::new()
        .route("/wake", post(handle_wake_hook))
        .route("/agent", post(handle_agent_hook))
        .route("/{*path}", post(handle_custom_hook))
        .layer(middleware::from_fn_with_state(
            gateway_state.clone(),
            require_hook_auth,
        ))
        .with_state(gateway_state.clone());

    // API routes
    let api_router = Router::new()
        .route("/health", get(health_check))
        .route("/status", get(gateway_status))
        .route("/connections", get(list_connections))
        .layer(middleware::from_fn_with_state(
            gateway_state.clone(),
            require_gateway_auth,
        ))
        .with_state(gateway_state.clone());

    // WebSocket router (with protocol state)
    let ws_router = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(ws_state);

    // Build main router
    Router::new()
        // Health check (no auth, no state needed)
        .route("/health", get(health_check))
        // WebSocket endpoint with protocol state
        .merge(ws_router)
        // Hooks (with hook auth)
        .nest(&hooks_base, hooks_router)
        // API (with gateway auth)
        .nest("/api", api_router)
        // Fallback
        .fallback(not_found)
        // Middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "service": "edith-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Gateway status endpoint
async fn gateway_status(State(state): State<GatewayState>) -> impl IntoResponse {
    let config = state.config();
    let connections = state.connections();

    Json(json!({
        "ok": true,
        "connections": connections.len(),
        "hooks_enabled": state.hooks_enabled(),
        "providers": {
            "whatsapp": config.web.as_ref().and_then(|w| w.enabled).unwrap_or(true),
            "telegram": config.telegram.as_ref().and_then(|t| t.enabled).unwrap_or(true),
            "discord": config.discord.as_ref().and_then(|d| d.enabled).unwrap_or(true),
            "signal": config.signal.as_ref().and_then(|s| s.enabled).unwrap_or(true),
        }
    }))
}

/// List active WebSocket connections
async fn list_connections(State(state): State<GatewayState>) -> impl IntoResponse {
    let connections: Vec<_> = state
        .connections()
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id,
                "connected_at": c.connected_at.to_rfc3339(),
                "remote_addr": c.remote_addr,
                "user_agent": c.user_agent
            })
        })
        .collect();

    Json(json!({
        "ok": true,
        "connections": connections
    }))
}

/// 404 handler
async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "not found" })))
}
