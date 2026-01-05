//! WebSocket connection state and handshake

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use edith_protocol::{
    AuthInfo, ClientInfo, ConnectParams, ErrorShape, Features, GatewayFrame, HelloError,
    HelloOk, HealthSnapshot, Policy, PresenceEntry, ResponseFrame,
    ServerInfo, Snapshot, StateVersion, PROTOCOL_VERSION, codes,
};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Connection state for a single WebSocket client
pub struct ConnectionState {
    /// Unique connection ID
    pub id: String,

    /// When the connection was established
    pub connected_at: Instant,

    /// Whether handshake is complete
    pub handshake_complete: bool,

    /// Negotiated protocol version
    pub protocol_version: u32,

    /// Client information from connect params
    pub client_info: Option<ClientInfo>,

    /// Client capabilities
    pub capabilities: Vec<String>,

    /// Whether the client is authenticated
    pub authenticated: bool,

    /// Auth info if provided
    pub auth_info: Option<AuthInfo>,

    /// User agent string
    pub user_agent: Option<String>,

    /// Locale preference
    pub locale: Option<String>,

    /// Event sequence number (monotonically increasing)
    pub event_seq: u64,

    /// Sender for outgoing frames
    pub frame_tx: mpsc::Sender<GatewayFrame>,
}

impl ConnectionState {
    /// Create a new connection state
    pub fn new(id: String, frame_tx: mpsc::Sender<GatewayFrame>) -> Self {
        Self {
            id,
            connected_at: Instant::now(),
            handshake_complete: false,
            protocol_version: 0,
            client_info: None,
            capabilities: Vec::new(),
            authenticated: false,
            auth_info: None,
            user_agent: None,
            locale: None,
            event_seq: 0,
            frame_tx,
        }
    }

    /// Get next event sequence number
    pub fn next_seq(&mut self) -> u64 {
        self.event_seq += 1;
        self.event_seq
    }

    /// Check if connection supports a capability
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.contains(&cap.to_string())
    }
}

/// Process connect handshake
pub fn process_handshake(
    params: &ConnectParams,
    connection_id: &str,
    server_start_time: Instant,
    presence: &HashMap<String, PresenceEntry>,
    health: &HealthSnapshot,
    requires_auth: bool,
    check_auth: impl Fn(&AuthInfo) -> bool,
) -> Result<(HelloOk, u32), HelloError> {
    // Check protocol version negotiation
    let negotiated_version = negotiate_protocol_version(
        params.min_protocol,
        params.max_protocol,
    )?;

    // Check authentication if required
    if requires_auth {
        if let Some(auth) = &params.auth {
            if !check_auth(auth) {
                return Err(HelloError {
                    type_: "hello-error".to_string(),
                    code: codes::AUTH_FAILED.to_string(),
                    message: "Authentication failed".to_string(),
                    retry_ok: true,
                    retry_hints: Some(serde_json::json!({
                        "suggestion": "Check token and try again"
                    })),
                });
            }
        } else {
            return Err(HelloError {
                type_: "hello-error".to_string(),
                code: codes::AUTH_REQUIRED.to_string(),
                message: "Authentication required".to_string(),
                retry_ok: true,
                retry_hints: Some(serde_json::json!({
                    "auth_types": ["token", "bearer"]
                })),
            });
        }
    }

    // Build server info
    let server = ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: option_env!("GIT_COMMIT").map(String::from),
        host: hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string()),
        conn_id: connection_id.to_string(),
    };

    // Build features with all supported methods and events
    let features = Features::default();

    let uptime_ms = server_start_time.elapsed().as_millis() as u64;

    // Build current snapshot
    let snapshot = Snapshot {
        presence: presence.clone(),
        health: health.clone(),
        state_version: StateVersion {
            presence: Some(chrono::Utc::now().timestamp_millis() as u64),
            health: Some(chrono::Utc::now().timestamp_millis() as u64),
        },
        uptime_ms,
        config_path: None,
        state_dir: None,
    };

    // Build policy with defaults
    let policy = Policy::default();

    let hello_ok = HelloOk {
        type_: "hello-ok".to_string(),
        protocol: negotiated_version,
        server,
        features,
        snapshot,
        canvas_host_url: None,
        policy,
    };

    Ok((hello_ok, negotiated_version))
}

/// Negotiate protocol version
fn negotiate_protocol_version(min: u32, max: u32) -> Result<u32, HelloError> {
    // Server supports only PROTOCOL_VERSION
    if PROTOCOL_VERSION >= min && PROTOCOL_VERSION <= max {
        Ok(PROTOCOL_VERSION)
    } else if PROTOCOL_VERSION < min {
        Err(HelloError {
            type_: "hello-error".to_string(),
            code: codes::PROTOCOL_MISMATCH.to_string(),
            message: format!(
                "Server protocol version {} is below client minimum {}",
                PROTOCOL_VERSION, min
            ),
            retry_ok: false,
            retry_hints: Some(serde_json::json!({
                "server_version": PROTOCOL_VERSION,
                "client_min": min,
                "client_max": max
            })),
        })
    } else {
        Err(HelloError {
            type_: "hello-error".to_string(),
            code: codes::PROTOCOL_MISMATCH.to_string(),
            message: format!(
                "Server protocol version {} is above client maximum {}",
                PROTOCOL_VERSION, max
            ),
            retry_ok: false,
            retry_hints: Some(serde_json::json!({
                "server_version": PROTOCOL_VERSION,
                "client_min": min,
                "client_max": max
            })),
        })
    }
}

/// Connection manager for all active connections
pub struct ConnectionManager {
    connections: RwLock<HashMap<String, Arc<RwLock<ConnectionState>>>>,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new connection
    pub fn register(&self, id: String, frame_tx: mpsc::Sender<GatewayFrame>) -> Arc<RwLock<ConnectionState>> {
        let state = Arc::new(RwLock::new(ConnectionState::new(id.clone(), frame_tx)));
        self.connections.write().insert(id, state.clone());
        state
    }

    /// Remove a connection
    pub fn remove(&self, id: &str) {
        self.connections.write().remove(id);
    }

    /// Get a connection by ID
    pub fn get(&self, id: &str) -> Option<Arc<RwLock<ConnectionState>>> {
        self.connections.read().get(id).cloned()
    }

    /// Get all connection IDs
    pub fn connection_ids(&self) -> Vec<String> {
        self.connections.read().keys().cloned().collect()
    }

    /// Get connection count
    pub fn count(&self) -> usize {
        self.connections.read().len()
    }

    /// Broadcast event to all connections
    pub async fn broadcast_event(&self, event: &str, payload: serde_json::Value, state_version: Option<StateVersion>) {
        let connections = self.connections.read().clone();
        for (id, conn_state) in connections {
            let conn = conn_state.read();
            if !conn.handshake_complete {
                continue;
            }

            let seq = {
                drop(conn);
                let mut conn = conn_state.write();
                conn.next_seq()
            };

            let frame = GatewayFrame::Event(edith_protocol::EventFrame {
                event: event.to_string(),
                payload: Some(payload.clone()),
                seq,
                state_version: state_version.clone(),
            });

            let conn = conn_state.read();
            if conn.frame_tx.send(frame).await.is_err() {
                debug!(connection_id = %id, "Failed to send event to connection");
            }
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_negotiation() {
        // Server at version 2, client supports 1-3
        let result = negotiate_protocol_version(1, 3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PROTOCOL_VERSION);

        // Client only supports version 1
        let result = negotiate_protocol_version(1, 1);
        if PROTOCOL_VERSION > 1 {
            assert!(result.is_err());
        }

        // Client only supports version 10+
        let result = negotiate_protocol_version(10, 15);
        assert!(result.is_err());
    }
}
