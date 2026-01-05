//! Session types and management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chat type for sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    Direct,
    Group,
    Room,
}

impl Default for ChatType {
    fn default() -> Self {
        Self::Direct
    }
}

/// Queue mode for message handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueueMode {
    Steer,
    Followup,
    Collect,
    SteerBacklog,
    #[serde(rename = "steer+backlog")]
    SteerPlusBacklog,
    Queue,
    Interrupt,
}

/// Queue drop policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueDropPolicy {
    Old,
    New,
    Summarize,
}

/// Send policy action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SendPolicy {
    Allow,
    Deny,
}

impl Default for SendPolicy {
    fn default() -> Self {
        Self::Allow
    }
}

/// Group activation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupActivation {
    Mention,
    Always,
}

impl Default for GroupActivation {
    fn default() -> Self {
        Self::Mention
    }
}

/// Last channel used for routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LastChannel {
    WhatsApp,
    Telegram,
    Discord,
    Signal,
    IMessage,
    WebChat,
}

/// Session entry stored in the session store
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    /// Unique session ID (UUID)
    pub session_id: String,

    /// Last update timestamp (epoch ms)
    pub updated_at: i64,

    /// Whether the system prompt has been sent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_sent: Option<bool>,

    /// Whether the last run was aborted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aborted_last_run: Option<bool>,

    /// Chat type (direct, group, room)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_type: Option<ChatType>,

    /// Current thinking level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,

    /// Verbose level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbose_level: Option<String>,

    /// Provider override (e.g., "anthropic", "openai")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_override: Option<String>,

    /// Model override (e.g., "claude-3-5-sonnet")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,

    /// Group activation mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_activation: Option<GroupActivation>,

    /// Whether group needs system intro
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_activation_needs_system_intro: Option<bool>,

    /// Send policy override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_policy: Option<SendPolicy>,

    /// Queue mode override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_mode: Option<QueueMode>,

    /// Queue debounce ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_debounce_ms: Option<u64>,

    /// Queue cap
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_cap: Option<u64>,

    /// Queue drop policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_drop: Option<QueueDropPolicy>,

    /// Input tokens used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,

    /// Output tokens used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,

    /// Total tokens used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,

    /// Current model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Context window tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,

    /// Display name for the session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Surface (whatsapp, telegram, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,

    /// Group subject/title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Room name (for Discord)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,

    /// Space/server name (for Discord/Slack)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,

    /// Last channel used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_channel: Option<LastChannel>,

    /// Last recipient
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_to: Option<String>,

    /// Skills snapshot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_snapshot: Option<SkillsSnapshot>,
}

impl SessionEntry {
    /// Create a new session entry
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            updated_at: Utc::now().timestamp_millis(),
            system_sent: None,
            aborted_last_run: None,
            chat_type: None,
            thinking_level: None,
            verbose_level: None,
            provider_override: None,
            model_override: None,
            group_activation: None,
            group_activation_needs_system_intro: None,
            send_policy: None,
            queue_mode: None,
            queue_debounce_ms: None,
            queue_cap: None,
            queue_drop: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            model: None,
            context_tokens: None,
            display_name: None,
            surface: None,
            subject: None,
            room: None,
            space: None,
            last_channel: None,
            last_to: None,
            skills_snapshot: None,
        }
    }

    /// Check if the session is expired
    pub fn is_expired(&self, idle_minutes: u64) -> bool {
        let now = Utc::now().timestamp_millis();
        let idle_ms = (idle_minutes * 60 * 1000) as i64;
        now - self.updated_at > idle_ms
    }

    /// Touch the session (update timestamp)
    pub fn touch(&mut self) {
        self.updated_at = Utc::now().timestamp_millis();
    }

    /// Update last route information
    pub fn update_last_route(&mut self, channel: LastChannel, to: Option<String>) {
        self.last_channel = Some(channel);
        self.last_to = to;
        self.touch();
    }

    /// Add token usage
    pub fn add_token_usage(&mut self, input: u64, output: u64) {
        self.input_tokens = Some(self.input_tokens.unwrap_or(0) + input);
        self.output_tokens = Some(self.output_tokens.unwrap_or(0) + output);
        self.total_tokens = Some(self.total_tokens.unwrap_or(0) + input + output);
        self.touch();
    }
}

impl Default for SessionEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Skills snapshot for session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsSnapshot {
    pub prompt: String,
    pub skills: Vec<SkillInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_skills: Option<Vec<serde_json::Value>>,
}

/// Skill information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_env: Option<String>,
}

/// Session store type
pub type SessionStore = HashMap<String, SessionEntry>;
