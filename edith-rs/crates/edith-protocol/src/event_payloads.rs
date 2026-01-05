//! Event payload types
//!
//! Defines the payload structures for all server-push events.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::PresenceEntry;

/// Agent event payload (streamed during agent runs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEventPayload {
    /// Run ID
    #[serde(rename = "runId")]
    pub run_id: String,

    /// Event sequence within this run
    pub seq: u64,

    /// Stream type (job, tool, assistant, error)
    pub stream: String,

    /// Timestamp in milliseconds
    pub ts: i64,

    /// Event data
    pub data: serde_json::Value,

    /// Session key
    #[serde(rename = "sessionKey", skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
}

/// Agent event stream types
pub mod agent_stream {
    pub const JOB: &str = "job";
    pub const TOOL: &str = "tool";
    pub const ASSISTANT: &str = "assistant";
    pub const ERROR: &str = "error";
}

/// Tool event phases
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolPhase {
    Start,
    Update,
    Result,
}

/// Tool event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEventData {
    /// Event phase
    pub phase: ToolPhase,

    /// Tool name
    pub name: String,

    /// Tool call ID
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,

    /// Tool arguments (for start phase)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,

    /// Partial result (for update phase)
    #[serde(rename = "partialResult", skip_serializing_if = "Option::is_none")]
    pub partial_result: Option<serde_json::Value>,

    /// Final result (for result phase)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Whether result is an error
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Assistant text event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEventType {
    TextStart,
    TextDelta,
    TextEnd,
}

/// Assistant text event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTextData {
    /// Event type
    #[serde(rename = "type")]
    pub event_type: TextEventType,

    /// Text delta (for delta events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,

    /// Full text (for end events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Chat event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatEventPayload {
    /// Session ID
    #[serde(rename = "sessionId")]
    pub session_id: String,

    /// Run ID
    #[serde(rename = "runId")]
    pub run_id: String,

    /// Event type (delta, complete, error)
    #[serde(rename = "type")]
    pub event_type: String,

    /// Text delta or full text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Media URLs
    #[serde(rename = "mediaUrls", skip_serializing_if = "Option::is_none")]
    pub media_urls: Option<Vec<String>>,

    /// Error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Presence event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEventPayload {
    /// Current presence entries
    pub presence: HashMap<String, PresenceEntry>,
}

/// Tick event payload (keepalive)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TickEventPayload {
    /// Server timestamp
    pub ts: i64,

    /// Server uptime in milliseconds
    #[serde(rename = "uptimeMs")]
    pub uptime_ms: u64,
}

/// Shutdown event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownEventPayload {
    /// Shutdown reason
    pub reason: String,

    /// Expected restart time in milliseconds (if applicable)
    #[serde(rename = "restartExpectedMs", skip_serializing_if = "Option::is_none")]
    pub restart_expected_ms: Option<u64>,
}

/// Health event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEventPayload {
    /// Health snapshot
    pub health: crate::HealthSnapshot,
}

/// Heartbeat event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatEventPayload {
    /// Session key
    #[serde(rename = "sessionKey")]
    pub session_key: String,

    /// Message text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Target channel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Cron event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronEventPayload {
    /// Job ID
    #[serde(rename = "jobId")]
    pub job_id: String,

    /// Job name
    pub name: String,

    /// Event type (started, completed, failed)
    #[serde(rename = "type")]
    pub event_type: String,

    /// Timestamp
    pub ts: i64,

    /// Duration in milliseconds (for completed/failed)
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Error message (for failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Node pairing request event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePairRequestedPayload {
    /// Request ID
    #[serde(rename = "requestId")]
    pub request_id: String,

    /// Node name
    pub name: String,

    /// Capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Vec<String>>,

    /// Timestamp
    pub ts: i64,
}

/// Node pairing resolved event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePairResolvedPayload {
    /// Request ID
    #[serde(rename = "requestId")]
    pub request_id: String,

    /// Resolution (approved, rejected)
    pub resolution: String,

    /// Node ID (if approved)
    #[serde(rename = "nodeId", skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

/// Voice wake changed event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceWakeChangedPayload {
    /// Whether voice wake is enabled
    pub enabled: bool,

    /// Wake phrases
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrases: Option<Vec<String>>,
}

/// Talk mode changed event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalkModePayload {
    /// Whether talk mode is enabled
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_event_serialization() {
        let event = AgentEventPayload {
            run_id: "run-123".to_string(),
            seq: 1,
            stream: "tool".to_string(),
            ts: 1234567890,
            data: serde_json::json!({
                "phase": "start",
                "name": "bash",
                "toolCallId": "tc-1"
            }),
            session_key: Some("main".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""runId":"run-123""#));
        assert!(json.contains(r#""stream":"tool""#));
    }

    #[test]
    fn test_shutdown_event() {
        let event = ShutdownEventPayload {
            reason: "Restart requested".to_string(),
            restart_expected_ms: Some(5000),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""restartExpectedMs":5000"#));
    }
}
