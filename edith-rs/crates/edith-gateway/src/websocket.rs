//! WebSocket handling with Clawdis-compatible protocol
//!
//! Implements the gateway WebSocket protocol with:
//! - Connect handshake (ConnectParams → HelloOk/HelloError)
//! - Request/Response/Event frame processing
//! - Method dispatch for 51 RPC methods
//! - Tick keepalives and state versioning

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use edith_protocol::{
    ConnectParams, EventFrame, GatewayFrame, HelloError, HealthSnapshot, PresenceEntry,
    ResponseFrame, StateVersion, TickEventPayload, HANDSHAKE_TIMEOUT_MS,
    TICK_INTERVAL_MS, codes, events,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::protocol::{process_handshake, ConnectionManager, ConnectionState, RequestHandler};
use crate::GatewayState;

/// Shared protocol state across all connections
pub struct ProtocolState {
    /// Connection manager
    pub connections: ConnectionManager,

    /// Request handler with deduplication
    pub request_handler: RequestHandler,

    /// Server start time
    pub server_start: Instant,

    /// Global event sequence counter
    event_seq: RwLock<u64>,

    /// Current presence state
    presence: RwLock<std::collections::HashMap<String, PresenceEntry>>,

    /// Current health snapshot
    health: RwLock<HealthSnapshot>,
}

impl ProtocolState {
    /// Create a new protocol state
    pub fn new(gateway_state: GatewayState) -> Self {
        let server_start = Instant::now();

        Self {
            connections: ConnectionManager::new(),
            request_handler: RequestHandler::new(gateway_state, server_start),
            server_start,
            event_seq: RwLock::new(0),
            presence: RwLock::new(std::collections::HashMap::new()),
            health: RwLock::new(HealthSnapshot::default()),
        }
    }

    /// Get next global event sequence
    pub fn next_event_seq(&self) -> u64 {
        let mut seq = self.event_seq.write();
        *seq += 1;
        *seq
    }

    /// Get current presence state
    pub fn presence(&self) -> std::collections::HashMap<String, PresenceEntry> {
        self.presence.read().clone()
    }

    /// Get current health snapshot
    pub fn health(&self) -> HealthSnapshot {
        let mut health = self.health.read().clone();
        health.uptime_ms = Some(self.server_start.elapsed().as_millis() as u64);
        health
    }

    /// Update presence for a session
    pub fn update_presence(&self, session_key: String, entry: PresenceEntry) {
        self.presence.write().insert(session_key, entry);
    }

    /// Remove presence for a session
    pub fn remove_presence(&self, session_key: &str) {
        self.presence.write().remove(session_key);
    }
}

/// Extended gateway state with protocol handling
#[derive(Clone)]
pub struct WebSocketState {
    pub gateway: GatewayState,
    pub protocol: Arc<ProtocolState>,
}

impl WebSocketState {
    pub fn new(gateway: GatewayState) -> Self {
        Self {
            protocol: Arc::new(ProtocolState::new(gateway.clone())),
            gateway,
        }
    }
}

/// Handle WebSocket upgrade
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection
async fn handle_socket(socket: WebSocket, state: WebSocketState) {
    let connection_id = uuid::Uuid::new_v4().to_string();
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Create channel for outgoing frames
    let (frame_tx, mut frame_rx) = mpsc::channel::<GatewayFrame>(100);

    // Register connection
    let conn_state = state.protocol.connections.register(connection_id.clone(), frame_tx.clone());

    info!(connection_id = %connection_id, "WebSocket connected, awaiting handshake");

    // Spawn frame sender task
    let sender_connection_id = connection_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            match serde_json::to_string(&frame) {
                Ok(json) => {
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to serialize frame");
                }
            }
        }
        debug!(connection_id = %sender_connection_id, "Frame sender task ended");
    });

    // Wait for connect handshake with timeout
    let handshake_result = tokio::time::timeout(
        Duration::from_millis(HANDSHAKE_TIMEOUT_MS),
        wait_for_handshake(&mut ws_receiver, &state, &connection_id, &conn_state, &frame_tx),
    )
    .await;

    let handshake_ok = match handshake_result {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            warn!(connection_id = %connection_id, error = %e, "Handshake failed");
            // Send hello-error
            let _ = frame_tx.send(GatewayFrame::Response(ResponseFrame {
                id: "handshake".to_string(),
                ok: false,
                payload: None,
                error: Some(edith_protocol::ErrorShape {
                    code: e.code,
                    message: e.message,
                    details: e.retry_hints,
                    retryable: Some(e.retry_ok),
                    retry_after_ms: None,
                }),
            })).await;
            false
        }
        Err(_) => {
            warn!(connection_id = %connection_id, "Handshake timeout");
            let _ = frame_tx.send(GatewayFrame::Response(ResponseFrame {
                id: "handshake".to_string(),
                ok: false,
                payload: None,
                error: Some(edith_protocol::ErrorShape::new(
                    codes::TIMEOUT,
                    "Handshake timeout",
                )),
            })).await;
            false
        }
    };

    if !handshake_ok {
        state.protocol.connections.remove(&connection_id);
        send_task.abort();
        return;
    }

    // Mark handshake complete
    conn_state.write().handshake_complete = true;
    info!(connection_id = %connection_id, "Handshake complete");

    // Spawn tick task
    let tick_connection_id = connection_id.clone();
    let tick_conn_state = conn_state.clone();
    let tick_state = state.clone();
    let tick_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(TICK_INTERVAL_MS));
        loop {
            ticker.tick().await;

            let seq = {
                let mut conn = tick_conn_state.write();
                conn.next_seq()
            };

            let tick_payload = TickEventPayload {
                ts: chrono::Utc::now().timestamp_millis(),
                uptime_ms: tick_state.protocol.server_start.elapsed().as_millis() as u64,
            };

            let frame = GatewayFrame::Event(EventFrame {
                event: events::TICK.to_string(),
                payload: Some(serde_json::to_value(tick_payload).unwrap_or(json!({}))),
                seq,
                state_version: Some(StateVersion {
                    presence: Some(chrono::Utc::now().timestamp_millis() as u64),
                    health: Some(chrono::Utc::now().timestamp_millis() as u64),
                }),
            });

            // Extract sender before await to avoid holding lock across await point
            let sender = tick_conn_state.read().frame_tx.clone();
            if sender.send(frame).await.is_err() {
                break;
            }
        }
        debug!(connection_id = %tick_connection_id, "Tick task ended");
    });

    // Main message loop
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<GatewayFrame>(&text) {
                    Ok(GatewayFrame::Request(request)) => {
                        let response = state
                            .protocol
                            .request_handler
                            .handle_request(request, &conn_state)
                            .await;

                        if frame_tx.send(GatewayFrame::Response(response)).await.is_err() {
                            break;
                        }
                    }
                    Ok(GatewayFrame::Response(_)) => {
                        debug!("Received response frame from client, ignoring");
                    }
                    Ok(GatewayFrame::Event(_)) => {
                        debug!("Received event frame from client, ignoring");
                    }
                    Err(e) => {
                        warn!(error = %e, "Invalid frame format");
                        let error_response = GatewayFrame::Response(ResponseFrame {
                            id: "parse_error".to_string(),
                            ok: false,
                            payload: None,
                            error: Some(edith_protocol::ErrorShape::new(
                                codes::INVALID_REQUEST,
                                &format!("Invalid frame: {}", e),
                            )),
                        });
                        if frame_tx.send(error_response).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Ok(Message::Binary(_)) => {
                debug!("Received binary message, ignoring");
            }
            Ok(Message::Ping(_)) => {
                debug!("Received ping");
            }
            Ok(Message::Pong(_)) => {
                debug!("Received pong");
            }
            Ok(Message::Close(_)) => {
                debug!(connection_id = %connection_id, "WebSocket close received");
                break;
            }
            Err(e) => {
                error!(connection_id = %connection_id, error = %e, "WebSocket error");
                break;
            }
        }
    }

    // Cleanup
    state.protocol.connections.remove(&connection_id);
    tick_task.abort();
    send_task.abort();
    info!(connection_id = %connection_id, "WebSocket disconnected");
}

/// Wait for and process the connect handshake
async fn wait_for_handshake(
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
    state: &WebSocketState,
    connection_id: &str,
    conn_state: &Arc<parking_lot::RwLock<ConnectionState>>,
    frame_tx: &mpsc::Sender<GatewayFrame>,
) -> Result<(), HelloError> {
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Try to parse as ConnectParams
                match serde_json::from_str::<ConnectParams>(&text) {
                    Ok(params) => {
                        let config = state.gateway.config();
                        let requires_auth = config
                            .gateway
                            .as_ref()
                            .and_then(|g| g.auth.as_ref())
                            .is_some();

                        // Process handshake
                        let (hello_ok, protocol_version) = process_handshake(
                            &params,
                            connection_id,
                            state.protocol.server_start,
                            &state.protocol.presence(),
                            &state.protocol.health(),
                            requires_auth,
                            |auth| {
                                // Check auth token
                                auth.token.as_ref()
                                    .map(|t| state.gateway.check_auth_token(t))
                                    .unwrap_or(false)
                            },
                        )?;

                        // Update connection state
                        {
                            let mut conn = conn_state.write();
                            conn.protocol_version = protocol_version;
                            conn.client_info = Some(params.client);
                            conn.capabilities = params.caps.unwrap_or_default();
                            conn.auth_info = params.auth;
                            conn.user_agent = params.user_agent;
                            conn.locale = params.locale;
                            conn.authenticated = !requires_auth || conn.auth_info.is_some();
                        }

                        // Send hello-ok
                        let _ = frame_tx.send(GatewayFrame::Response(ResponseFrame {
                            id: "handshake".to_string(),
                            ok: true,
                            payload: Some(serde_json::to_value(&hello_ok).unwrap_or(json!({}))),
                            error: None,
                        })).await;

                        return Ok(());
                    }
                    Err(e) => {
                        debug!(error = %e, "Message is not ConnectParams, ignoring");
                        // Could be legacy client, continue waiting
                    }
                }
            }
            Ok(Message::Close(_)) => {
                return Err(HelloError {
                    type_: "hello-error".to_string(),
                    code: codes::CONNECTION_CLOSED.to_string(),
                    message: "Connection closed during handshake".to_string(),
                    retry_ok: true,
                    retry_hints: None,
                });
            }
            Err(e) => {
                return Err(HelloError {
                    type_: "hello-error".to_string(),
                    code: codes::INTERNAL_ERROR.to_string(),
                    message: format!("WebSocket error: {}", e),
                    retry_ok: true,
                    retry_hints: None,
                });
            }
            _ => {
                // Ignore other message types during handshake
            }
        }
    }

    Err(HelloError {
        type_: "hello-error".to_string(),
        code: codes::CONNECTION_CLOSED.to_string(),
        message: "Connection closed before handshake".to_string(),
        retry_ok: true,
        retry_hints: None,
    })
}

/// Legacy WebSocket message types (for backwards compatibility)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LegacyWsMessage {
    /// Ping/pong for keepalive
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },

    /// Error message
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_key: Option<String>,
    },

    /// Connection info
    Connected {
        connection_id: String,
        server_time: u64,
    },
}
