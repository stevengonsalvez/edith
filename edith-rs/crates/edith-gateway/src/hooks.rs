//! HTTP hooks handling

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::dispatch::{DeliveryChannel, EventDispatcher, WakeEvent, WakeMode as DispatchWakeMode};
use crate::{GatewayError, GatewayResult, GatewayState};

/// Wake hook payload
#[derive(Debug, Deserialize)]
pub struct WakePayload {
    pub text: String,
    #[serde(default = "default_wake_mode")]
    pub mode: WakeMode,
}

fn default_wake_mode() -> WakeMode {
    WakeMode::Now
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WakeMode {
    Now,
    NextHeartbeat,
}

/// Agent hook payload
#[derive(Debug, Deserialize)]
pub struct AgentPayload {
    pub message: String,
    #[serde(default = "default_agent_name")]
    pub name: String,
    #[serde(default = "default_wake_mode")]
    #[serde(rename = "wakeMode")]
    pub wake_mode: WakeMode,
    #[serde(default)]
    #[serde(rename = "sessionKey")]
    pub session_key: String,
    #[serde(default)]
    pub deliver: bool,
    #[serde(default = "default_channel")]
    pub channel: Channel,
    pub to: Option<String>,
    pub thinking: Option<String>,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u64>,
}

fn default_agent_name() -> String {
    "Hook".to_string()
}

fn default_channel() -> Channel {
    Channel::Last
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Last,
    Whatsapp,
    Telegram,
    Discord,
    Signal,
    Imessage,
}

/// Hook response
#[derive(Debug, Serialize)]
pub struct HookResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<WakeMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "runId")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl HookResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            mode: None,
            run_id: None,
            error: None,
        }
    }

    pub fn ok_with_mode(mode: WakeMode) -> Self {
        Self {
            ok: true,
            mode: Some(mode),
            run_id: None,
            error: None,
        }
    }

    pub fn ok_with_run_id(run_id: String) -> Self {
        Self {
            ok: true,
            mode: None,
            run_id: Some(run_id),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            mode: None,
            run_id: None,
            error: Some(message.into()),
        }
    }
}

/// Handle wake hook
pub async fn handle_wake_hook(
    State(state): State<GatewayState>,
    Json(payload): Json<WakePayload>,
) -> GatewayResult<impl IntoResponse> {
    tracing::info!(text = %payload.text, mode = ?payload.mode, "wake hook received");

    // Convert to dispatch wake mode
    let dispatch_mode = match payload.mode {
        WakeMode::Now => DispatchWakeMode::Now,
        WakeMode::NextHeartbeat => DispatchWakeMode::NextHeartbeat,
    };

    // Dispatch wake event
    let event = WakeEvent {
        text: payload.text,
        mode: dispatch_mode,
        source: "hook:wake".to_string(),
    };

    if let Err(e) = state.dispatcher().dispatch_wake(event).await {
        tracing::error!(error = %e, "failed to dispatch wake event");
        return Err(GatewayError::Internal(format!("failed to dispatch: {}", e)));
    }

    Ok((
        StatusCode::OK,
        Json(HookResponse::ok_with_mode(payload.mode)),
    ))
}

/// Handle agent hook
pub async fn handle_agent_hook(
    State(state): State<GatewayState>,
    Json(payload): Json<AgentPayload>,
) -> GatewayResult<impl IntoResponse> {
    tracing::info!(
        message = %payload.message,
        name = %payload.name,
        channel = ?payload.channel,
        "agent hook received"
    );

    // Convert to dispatch types
    let dispatch_mode = match payload.wake_mode {
        WakeMode::Now => DispatchWakeMode::Now,
        WakeMode::NextHeartbeat => DispatchWakeMode::NextHeartbeat,
    };

    let dispatch_channel = match payload.channel {
        Channel::Last => DeliveryChannel::Last,
        Channel::Whatsapp => DeliveryChannel::WhatsApp,
        Channel::Telegram => DeliveryChannel::Telegram,
        Channel::Discord => DeliveryChannel::Discord,
        Channel::Signal => DeliveryChannel::Signal,
        Channel::Imessage => DeliveryChannel::IMessage,
    };

    // Create and dispatch agent run request
    let request = EventDispatcher::create_agent_run_request(
        payload.message,
        payload.name,
        dispatch_mode,
        payload.session_key,
        payload.deliver,
        dispatch_channel,
        payload.to,
        payload.thinking,
        payload.timeout_seconds,
    );

    let run_id = request.run_id.clone();

    if let Err(e) = state.dispatcher().dispatch_agent_run(request).await {
        tracing::error!(error = %e, "failed to dispatch agent run");
        return Err(GatewayError::Internal(format!("failed to dispatch: {}", e)));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(HookResponse::ok_with_run_id(run_id)),
    ))
}

/// Handle custom mapped hooks
pub async fn handle_custom_hook(
    State(state): State<GatewayState>,
    Path(path): Path<String>,
    Json(_payload): Json<serde_json::Value>,
) -> GatewayResult<impl IntoResponse> {
    let config = state.config();
    let hooks_config = config.hooks.as_ref();

    // Check if there's a mapping for this path
    if let Some(mappings) = hooks_config.and_then(|h| h.mappings.as_ref()) {
        for mapping in mappings {
            if let Some(ref match_config) = mapping.match_ {
                if let Some(ref match_path) = match_config.path {
                    if match_path == &path || match_path == "*" {
                        tracing::info!(path = %path, "matched hook mapping");

                        // TODO: Apply mapping transformation and dispatch
                        // For now, acknowledge with 202

                        let run_id = uuid::Uuid::new_v4().to_string();
                        return Ok((
                            StatusCode::ACCEPTED,
                            Json(HookResponse::ok_with_run_id(run_id)),
                        ));
                    }
                }
            }
        }
    }

    // No mapping found
    Err(GatewayError::NotFound(format!("no mapping for path: {}", path)))
}
