//! RPC method parameter and result types
//!
//! Defines typed parameters and results for all 51 gateway methods.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Health & System Methods
// ============================================================================

/// Parameters for `health` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthParams {
    /// Whether to probe providers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<bool>,
}

/// Result for `health` method
pub type HealthResult = crate::HealthSnapshot;

/// Parameters for `status` method (no params)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusParams {}

/// Result for `status` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResult {
    pub ok: bool,
    pub linked: bool,
    #[serde(rename = "uptimeMs")]
    pub uptime_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// ============================================================================
// Configuration Methods
// ============================================================================

/// Parameters for `config.get` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigGetParams {
    /// Specific path to get (dot-notation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Parameters for `config.set` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetParams {
    /// Path to set (dot-notation)
    pub path: String,
    /// Value to set
    pub value: serde_json::Value,
}

// ============================================================================
// Session Methods
// ============================================================================

/// Parameters for `sessions.list` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionsListParams {
    /// Filter by prefix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Limit results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Session entry in list results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub key: String,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Result for `sessions.list` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsListResult {
    pub sessions: Vec<SessionEntry>,
    pub total: usize,
}

/// Parameters for `sessions.patch` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsPatchParams {
    /// Session key
    pub key: String,
    /// Thinking level to set
    #[serde(rename = "thinkingLevel", skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
    /// Verbose level
    #[serde(rename = "verboseLevel", skip_serializing_if = "Option::is_none")]
    pub verbose_level: Option<String>,
    /// Model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Parameters for `sessions.reset` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsResetParams {
    /// Session key to reset
    pub key: String,
}

/// Parameters for `sessions.delete` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsDeleteParams {
    /// Session key to delete
    pub key: String,
}

/// Parameters for `sessions.compact` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsCompactParams {
    /// Session key to compact
    pub key: String,
}

// ============================================================================
// Messaging Methods
// ============================================================================

/// Delivery channel for messages
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryChannel {
    #[default]
    Last,
    WhatsApp,
    Telegram,
    Discord,
    Signal,
    IMessage,
    Slack,
}

/// Parameters for `send` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendParams {
    /// Recipient identifier
    pub to: String,
    /// Message text
    pub message: String,
    /// Idempotency key (required)
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    /// Channel to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<DeliveryChannel>,
    /// Media URLs to attach
    #[serde(rename = "mediaUrls", skip_serializing_if = "Option::is_none")]
    pub media_urls: Option<Vec<String>>,
    /// Reply to message ID
    #[serde(rename = "replyToId", skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<String>,
}

/// Result for `send` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    /// Message ID
    #[serde(rename = "messageId")]
    pub message_id: String,
    /// Provider used
    pub provider: String,
    /// Timestamp
    pub timestamp: i64,
}

/// Thinking level for agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
}

/// Parameters for `agent` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentParams {
    /// Message/prompt for the agent
    pub message: String,
    /// Idempotency key (required)
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    /// Session key
    #[serde(rename = "sessionKey", skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    /// Thinking level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingLevel>,
    /// Model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Timeout in seconds
    #[serde(rename = "timeoutSeconds", skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// Whether to deliver response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deliver: Option<bool>,
    /// Delivery channel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<DeliveryChannel>,
    /// Recipient for delivery
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// Initial ack for `agent` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAck {
    /// Run ID
    #[serde(rename = "runId")]
    pub run_id: String,
    /// Status
    pub status: String, // "accepted"
}

/// Final result for `agent` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Run ID
    #[serde(rename = "runId")]
    pub run_id: String,
    /// Status (ok, error, aborted)
    pub status: String,
    /// Summary text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Duration in milliseconds
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Parameters for `agent.wait` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWaitParams {
    /// Run ID to wait for
    #[serde(rename = "runId")]
    pub run_id: String,
    /// Timeout in milliseconds
    #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

// ============================================================================
// Chat Methods
// ============================================================================

/// Parameters for `chat.send` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendParams {
    /// Session ID
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Message content
    pub content: String,
    /// Idempotency key
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    /// Media URLs
    #[serde(rename = "mediaUrls", skip_serializing_if = "Option::is_none")]
    pub media_urls: Option<Vec<String>>,
}

/// Parameters for `chat.abort` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAbortParams {
    /// Session ID
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Run ID to abort
    #[serde(rename = "runId", skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

/// Parameters for `chat.history` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistoryParams {
    /// Session ID
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Limit messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Offset for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

// ============================================================================
// Cron Methods
// ============================================================================

/// Parameters for `cron.list` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronListParams {}

/// Cron job entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    pub enabled: bool,
    #[serde(rename = "lastRunAt", skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<i64>,
    #[serde(rename = "nextRunAt", skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<i64>,
}

/// Parameters for `cron.add` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronAddParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    pub prompt: String,
    #[serde(rename = "sessionKey", skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
}

/// Parameters for `cron.update` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronUpdateParams {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Parameters for `cron.remove` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronRemoveParams {
    pub id: String,
}

/// Parameters for `cron.run` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronRunParams {
    pub id: String,
}

// ============================================================================
// Node Methods
// ============================================================================

/// Parameters for `node.list` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeListParams {}

/// Node entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeEntry {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Vec<String>>,
    #[serde(rename = "deviceFamily", skip_serializing_if = "Option::is_none")]
    pub device_family: Option<String>,
    #[serde(rename = "modelIdentifier", skip_serializing_if = "Option::is_none")]
    pub model_identifier: Option<String>,
    pub paired: bool,
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
}

/// Parameters for `node.invoke` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInvokeParams {
    /// Node ID
    #[serde(rename = "nodeId")]
    pub node_id: String,
    /// Command to invoke
    pub command: String,
    /// Command arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    /// Idempotency key
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

/// Parameters for `node.pair.request` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePairRequestParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Vec<String>>,
}

/// Parameters for `node.pair.approve` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePairApproveParams {
    /// Request ID
    #[serde(rename = "requestId")]
    pub request_id: String,
}

/// Parameters for `node.pair.reject` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePairRejectParams {
    /// Request ID
    #[serde(rename = "requestId")]
    pub request_id: String,
}

/// Parameters for `node.rename` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRenameParams {
    /// Node ID
    #[serde(rename = "nodeId")]
    pub node_id: String,
    /// New name
    pub name: String,
}

// ============================================================================
// Model & Skills Methods
// ============================================================================

/// Parameters for `models.list` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelsListParams {}

/// Model entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(rename = "contextWindow", skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
}

/// Parameters for `skills.install` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsInstallParams {
    /// Skill name or URL
    pub skill: String,
}

/// Parameters for `skills.update` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsUpdateParams {
    /// Skill name
    pub name: String,
    /// New configuration
    pub config: serde_json::Value,
}

// ============================================================================
// Wizard Methods
// ============================================================================

/// Parameters for `wizard.start` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WizardStartParams {
    /// Resume from step
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

/// Parameters for `wizard.next` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardNextParams {
    /// Current step response
    pub response: serde_json::Value,
}

// ============================================================================
// Voice/Talk Methods
// ============================================================================

/// Parameters for `talk.mode` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalkModeParams {
    /// Enable or disable
    pub enabled: bool,
}

/// Parameters for `voicewake.set` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceWakeSetParams {
    /// Enable or disable
    pub enabled: bool,
    /// Wake phrases
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrases: Option<Vec<String>>,
}

// ============================================================================
// System Methods
// ============================================================================

/// Parameters for `system-presence` method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemPresenceParams {}

/// Result for `system-presence` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPresenceResult {
    pub presence: HashMap<String, crate::PresenceEntry>,
    #[serde(rename = "stateVersion")]
    pub state_version: crate::StateVersion,
}

/// Parameters for `system-event` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEventParams {
    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,
    /// Event payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Wake mode
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WakeMode {
    #[default]
    Now,
    NextHeartbeat,
}

/// Parameters for `wake` method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeParams {
    /// Wake mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<WakeMode>,
    /// Custom prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_params_serialization() {
        let params = SendParams {
            to: "+1234567890".to_string(),
            message: "Hello".to_string(),
            idempotency_key: "key-123".to_string(),
            channel: Some(DeliveryChannel::WhatsApp),
            media_urls: None,
            reply_to_id: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains(r#""to":"+1234567890""#));
        assert!(json.contains(r#""idempotencyKey":"key-123""#));
    }

    #[test]
    fn test_agent_params_serialization() {
        let params = AgentParams {
            message: "Test prompt".to_string(),
            idempotency_key: "idem-456".to_string(),
            session_key: Some("main".to_string()),
            thinking: Some(ThinkingLevel::High),
            model: None,
            timeout_seconds: Some(300),
            deliver: Some(true),
            channel: Some(DeliveryChannel::Telegram),
            to: Some("@username".to_string()),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains(r#""thinking":"high""#));
        assert!(json.contains(r#""deliver":true"#));
    }
}
