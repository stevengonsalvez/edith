//! Connection handshake types
//!
//! The first message from client must be a `connect` request.
//! Server responds with `HelloOk` containing snapshot and policy.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{EVENTS, METHODS, PROTOCOL_VERSION};

/// Client information sent during connect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name (e.g., "swift-app", "webchat-ui", "edith-cli")
    pub name: String,

    /// Client version (e.g., "1.2.3")
    pub version: String,

    /// Platform (e.g., "iOS", "Android", "macOS", "Linux", "Windows")
    pub platform: String,

    /// Device family (e.g., "iPhone", "iPad", "MacBook Pro")
    #[serde(rename = "deviceFamily", skip_serializing_if = "Option::is_none")]
    pub device_family: Option<String>,

    /// Model identifier (e.g., "iPhone14,2")
    #[serde(rename = "modelIdentifier", skip_serializing_if = "Option::is_none")]
    pub model_identifier: Option<String>,

    /// Client mode (e.g., "app", "cli", "webchat", "remote")
    pub mode: String,

    /// Unique instance ID for this client
    #[serde(rename = "instanceId", skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// Authentication info for connect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthInfo {
    /// Authentication token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Password (alternative to token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Connect request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectParams {
    /// Minimum protocol version supported by client
    #[serde(rename = "minProtocol")]
    pub min_protocol: u32,

    /// Maximum protocol version supported by client
    #[serde(rename = "maxProtocol")]
    pub max_protocol: u32,

    /// Client information
    pub client: ClientInfo,

    /// Client capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Vec<String>>,

    /// Authentication info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthInfo>,

    /// Client locale (e.g., "en-US")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,

    /// User agent string
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

impl ConnectParams {
    /// Check if server protocol version is compatible
    pub fn is_protocol_compatible(&self, server_version: u32) -> bool {
        self.min_protocol <= server_version && server_version <= self.max_protocol
    }

    /// Get auth token if present
    pub fn token(&self) -> Option<&str> {
        self.auth.as_ref().and_then(|a| a.token.as_deref())
    }
}

/// Server information in HelloOk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server version
    pub version: String,

    /// Git commit hash
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,

    /// Server hostname
    pub host: String,

    /// Connection ID assigned to this client
    #[serde(rename = "connId")]
    pub conn_id: String,
}

/// Server features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    /// Supported methods
    pub methods: Vec<String>,

    /// Supported events
    pub events: Vec<String>,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            methods: METHODS.iter().map(|s| s.to_string()).collect(),
            events: EVENTS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Connection policy limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Maximum payload size in bytes
    #[serde(rename = "maxPayload")]
    pub max_payload: usize,

    /// Maximum buffered bytes per connection
    #[serde(rename = "maxBufferedBytes")]
    pub max_buffered_bytes: usize,

    /// Tick interval in milliseconds
    #[serde(rename = "tickIntervalMs")]
    pub tick_interval_ms: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_payload: crate::MAX_PAYLOAD_BYTES,
            max_buffered_bytes: crate::MAX_BUFFERED_BYTES,
            tick_interval_ms: crate::TICK_INTERVAL_MS,
        }
    }
}

/// Presence entry for a connected client or node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEntry {
    /// Hostname
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,

    /// Version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Platform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,

    /// Device family
    #[serde(rename = "deviceFamily", skip_serializing_if = "Option::is_none")]
    pub device_family: Option<String>,

    /// Model identifier
    #[serde(rename = "modelIdentifier", skip_serializing_if = "Option::is_none")]
    pub model_identifier: Option<String>,

    /// Mode (cli, app, remote, webchat)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Seconds since last input
    #[serde(rename = "lastInputSeconds", skip_serializing_if = "Option::is_none")]
    pub last_input_seconds: Option<u64>,

    /// Timestamp
    pub ts: i64,

    /// Reason for presence update
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Free-form text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Instance ID
    #[serde(rename = "instanceId", skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// Health summary snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthSnapshot {
    /// Overall health status
    pub ok: bool,

    /// Provider statuses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub providers: Option<HashMap<String, ProviderHealth>>,

    /// Last error message
    #[serde(rename = "lastError", skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// Uptime in milliseconds
    #[serde(rename = "uptimeMs", skip_serializing_if = "Option::is_none")]
    pub uptime_ms: Option<u64>,
}

/// Provider health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    /// Whether provider is connected/healthy
    pub ok: bool,

    /// Whether provider is linked (authenticated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked: Option<bool>,

    /// Last error
    #[serde(rename = "lastError", skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Initial snapshot sent in HelloOk
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// Current presence entries
    pub presence: HashMap<String, PresenceEntry>,

    /// Health status
    pub health: HealthSnapshot,

    /// Current state version
    #[serde(rename = "stateVersion")]
    pub state_version: crate::StateVersion,

    /// Server uptime in milliseconds
    #[serde(rename = "uptimeMs")]
    pub uptime_ms: u64,

    /// Config file path
    #[serde(rename = "configPath", skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,

    /// State directory
    #[serde(rename = "stateDir", skip_serializing_if = "Option::is_none")]
    pub state_dir: Option<String>,
}

/// HelloOk response payload for successful connect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloOk {
    /// Response type marker
    #[serde(rename = "type")]
    pub type_: String,

    /// Negotiated protocol version
    pub protocol: u32,

    /// Server information
    pub server: ServerInfo,

    /// Server features
    pub features: Features,

    /// Initial snapshot
    pub snapshot: Snapshot,

    /// Canvas host URL if available
    #[serde(rename = "canvasHostUrl", skip_serializing_if = "Option::is_none")]
    pub canvas_host_url: Option<String>,

    /// Connection policy
    pub policy: Policy,
}

impl HelloOk {
    /// Create a new HelloOk response
    pub fn new(
        server: ServerInfo,
        snapshot: Snapshot,
        canvas_host_url: Option<String>,
    ) -> Self {
        Self {
            type_: "hello-ok".to_string(),
            protocol: PROTOCOL_VERSION,
            server,
            features: Features::default(),
            snapshot,
            canvas_host_url,
            policy: Policy::default(),
        }
    }
}

/// HelloError response payload for failed connect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloError {
    /// Response type marker
    #[serde(rename = "type")]
    pub type_: String,

    /// Error code
    pub code: String,

    /// Error message
    pub message: String,

    /// Whether client should retry
    #[serde(rename = "retryOk")]
    pub retry_ok: bool,

    /// Hints for retry
    #[serde(rename = "retryHints", skip_serializing_if = "Option::is_none")]
    pub retry_hints: Option<serde_json::Value>,
}

impl HelloError {
    /// Create a new HelloError
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            type_: "hello-error".to_string(),
            code: code.into(),
            message: message.into(),
            retry_ok: true,
            retry_hints: None,
        }
    }

    /// Create a protocol mismatch error
    pub fn protocol_mismatch(server_version: u32, client_min: u32, client_max: u32) -> Self {
        Self {
            type_: "hello-error".to_string(),
            code: crate::codes::PROTOCOL_MISMATCH.to_string(),
            message: format!(
                "Protocol version mismatch: server={}, client=[{}-{}]",
                server_version, client_min, client_max
            ),
            retry_ok: false,
            retry_hints: Some(serde_json::json!({
                "server_version": server_version,
                "client_min": client_min,
                "client_max": client_max
            })),
        }
    }

    /// Create an auth required error
    pub fn auth_required() -> Self {
        Self {
            type_: "hello-error".to_string(),
            code: crate::codes::AUTH_REQUIRED.to_string(),
            message: "Authentication required".to_string(),
            retry_ok: true,
            retry_hints: Some(serde_json::json!({
                "auth_types": ["token", "bearer"]
            })),
        }
    }

    /// Create an auth failed error
    pub fn auth_failed() -> Self {
        Self {
            type_: "hello-error".to_string(),
            code: crate::codes::AUTH_FAILED.to_string(),
            message: "Authentication failed".to_string(),
            retry_ok: true,
            retry_hints: Some(serde_json::json!({
                "suggestion": "Check token and try again"
            })),
        }
    }
}

impl std::fmt::Display for HelloError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for HelloError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect_params_protocol_check() {
        let params = ConnectParams {
            min_protocol: 1,
            max_protocol: 3,
            client: ClientInfo {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
                platform: "test".to_string(),
                device_family: None,
                model_identifier: None,
                mode: "cli".to_string(),
                instance_id: None,
            },
            caps: None,
            auth: None,
            locale: None,
            user_agent: None,
        };

        assert!(params.is_protocol_compatible(1));
        assert!(params.is_protocol_compatible(2));
        assert!(params.is_protocol_compatible(3));
        assert!(!params.is_protocol_compatible(0));
        assert!(!params.is_protocol_compatible(4));
    }

    #[test]
    fn test_hello_ok_serialization() {
        let server = ServerInfo {
            version: "1.0.0".to_string(),
            commit: Some("abc123".to_string()),
            host: "localhost".to_string(),
            conn_id: "conn-1".to_string(),
        };

        let hello = HelloOk::new(server, Snapshot::default(), None);
        let json = serde_json::to_string(&hello).unwrap();

        assert!(json.contains(r#""type":"hello-ok""#));
        assert!(json.contains(&format!(r#""protocol":{}"#, PROTOCOL_VERSION)));
    }
}
