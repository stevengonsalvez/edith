//! Gateway frame types
//!
//! Defines the core frame types for the WebSocket protocol:
//! - Request frames (client → server)
//! - Response frames (server → client)
//! - Event frames (server → client, push notifications)

use serde::{Deserialize, Serialize};

use crate::error::ErrorShape;

/// State version for presence and health tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateVersion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<u64>,
}

/// Gateway frame - discriminated union of request, response, and event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GatewayFrame {
    /// Client request to server
    #[serde(rename = "req")]
    Request(RequestFrame),

    /// Server response to client
    #[serde(rename = "res")]
    Response(ResponseFrame),

    /// Server push event to client
    #[serde(rename = "event")]
    Event(EventFrame),
}

/// Request frame from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFrame {
    /// Unique request ID for correlation
    pub id: String,

    /// Method name to invoke
    pub method: String,

    /// Optional parameters for the method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,

    /// Optional idempotency key for deduplication
    #[serde(rename = "idempotencyKey", skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// Response frame from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFrame {
    /// Request ID this response correlates to
    pub id: String,

    /// Whether the request succeeded
    pub ok: bool,

    /// Success payload (when ok=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,

    /// Error details (when ok=false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

/// Event frame pushed from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFrame {
    /// Event type name
    pub event: String,

    /// Event payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,

    /// Monotonic sequence number for gap detection
    pub seq: u64,

    /// Optional state version for presence/health tracking
    #[serde(rename = "stateVersion", skip_serializing_if = "Option::is_none")]
    pub state_version: Option<StateVersion>,
}

impl RequestFrame {
    /// Create a new request frame
    pub fn new(id: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            method: method.into(),
            params: None,
            idempotency_key: None,
        }
    }

    /// Create a new request frame with parameters
    pub fn with_params<T: Serialize>(
        id: impl Into<String>,
        method: impl Into<String>,
        params: T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            id: id.into(),
            method: method.into(),
            params: Some(serde_json::to_value(params)?),
            idempotency_key: None,
        })
    }

    /// Set idempotency key
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

impl ResponseFrame {
    /// Create a success response
    pub fn success(id: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            payload: Some(payload),
            error: None,
        }
    }

    /// Create a success response without payload
    pub fn ok(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: true,
            payload: None,
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: impl Into<String>, error: ErrorShape) -> Self {
        Self {
            id: id.into(),
            ok: false,
            payload: None,
            error: Some(error),
        }
    }
}

impl EventFrame {
    /// Create a new event frame
    pub fn new(event: impl Into<String>, seq: u64) -> Self {
        Self {
            event: event.into(),
            payload: None,
            seq,
            state_version: None,
        }
    }

    /// Create a new event frame with payload
    pub fn with_payload<T: Serialize>(
        event: impl Into<String>,
        seq: u64,
        payload: T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            event: event.into(),
            payload: Some(serde_json::to_value(payload)?),
            seq,
            state_version: None,
        })
    }

    /// Set state version
    pub fn with_state_version(mut self, state_version: StateVersion) -> Self {
        self.state_version = Some(state_version);
        self
    }
}

impl GatewayFrame {
    /// Parse a gateway frame from JSON string
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize frame to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Check if this is a request frame
    pub fn is_request(&self) -> bool {
        matches!(self, GatewayFrame::Request(_))
    }

    /// Check if this is a response frame
    pub fn is_response(&self) -> bool {
        matches!(self, GatewayFrame::Response(_))
    }

    /// Check if this is an event frame
    pub fn is_event(&self) -> bool {
        matches!(self, GatewayFrame::Event(_))
    }

    /// Get the request if this is a request frame
    pub fn as_request(&self) -> Option<&RequestFrame> {
        match self {
            GatewayFrame::Request(req) => Some(req),
            _ => None,
        }
    }

    /// Get the response if this is a response frame
    pub fn as_response(&self) -> Option<&ResponseFrame> {
        match self {
            GatewayFrame::Response(res) => Some(res),
            _ => None,
        }
    }

    /// Get the event if this is an event frame
    pub fn as_event(&self) -> Option<&EventFrame> {
        match self {
            GatewayFrame::Event(evt) => Some(evt),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_frame_serialization() {
        let frame = GatewayFrame::Request(RequestFrame::new("1", "health"));
        let json = frame.to_json().unwrap();
        assert!(json.contains(r#""type":"req""#));
        assert!(json.contains(r#""method":"health""#));

        let parsed = GatewayFrame::parse(&json).unwrap();
        assert!(parsed.is_request());
    }

    #[test]
    fn test_response_frame_serialization() {
        let frame = GatewayFrame::Response(ResponseFrame::success(
            "1",
            serde_json::json!({"status": "ok"}),
        ));
        let json = frame.to_json().unwrap();
        assert!(json.contains(r#""type":"res""#));
        assert!(json.contains(r#""ok":true"#));

        let parsed = GatewayFrame::parse(&json).unwrap();
        assert!(parsed.is_response());
    }

    #[test]
    fn test_event_frame_serialization() {
        let frame = GatewayFrame::Event(EventFrame::new("tick", 42));
        let json = frame.to_json().unwrap();
        assert!(json.contains(r#""type":"event""#));
        assert!(json.contains(r#""event":"tick""#));
        assert!(json.contains(r#""seq":42"#));

        let parsed = GatewayFrame::parse(&json).unwrap();
        assert!(parsed.is_event());
    }

    #[test]
    fn test_error_response() {
        let error = ErrorShape::new("NOT_LINKED", "WhatsApp not authenticated");
        let frame = GatewayFrame::Response(ResponseFrame::error("1", error));
        let json = frame.to_json().unwrap();
        assert!(json.contains(r#""ok":false"#));
        assert!(json.contains(r#""code":"NOT_LINKED""#));
    }
}
