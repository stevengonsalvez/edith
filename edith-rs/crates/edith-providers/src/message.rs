//! Common message types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Message direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Inbound,
    Outbound,
}

/// Chat type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    Direct,
    Group,
}

impl Default for ChatType {
    fn default() -> Self {
        Self::Direct
    }
}

/// Media type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    Sticker,
}

impl MediaType {
    pub fn from_mime(mime: &str) -> Option<Self> {
        if mime.starts_with("image/") {
            Some(Self::Image)
        } else if mime.starts_with("video/") {
            Some(Self::Video)
        } else if mime.starts_with("audio/") {
            Some(Self::Audio)
        } else {
            Some(Self::Document)
        }
    }
}

/// Media attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    /// Media type
    pub media_type: MediaType,
    /// MIME type
    pub mime_type: String,
    /// URL to the media (local file:// or remote http://)
    pub url: String,
    /// File name
    pub file_name: Option<String>,
    /// File size in bytes
    pub size: Option<u64>,
    /// Caption for the media
    pub caption: Option<String>,
}

/// Reply context for threaded replies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyContext {
    /// ID of the message being replied to
    pub message_id: String,
    /// Body of the replied message (for display)
    pub body: Option<String>,
    /// Sender of the replied message
    pub sender: Option<String>,
}

/// An inbound message from a messaging platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Unique message ID from the platform
    pub id: Option<String>,

    /// Sender identifier (E.164 for phone, platform ID otherwise)
    pub from: String,

    /// Conversation/chat identifier
    pub conversation_id: String,

    /// Recipient (usually us)
    pub to: String,

    /// Message body/text
    pub body: String,

    /// Sender's display name (push name)
    pub sender_name: Option<String>,

    /// Timestamp of the message
    pub timestamp: Option<DateTime<Utc>>,

    /// Chat type
    pub chat_type: ChatType,

    /// Chat/conversation identifier
    pub chat_id: String,

    /// Sender JID (for WhatsApp groups)
    pub sender_jid: Option<String>,

    /// Sender E.164 (for groups)
    pub sender_e164: Option<String>,

    /// Reply context if this is a reply
    pub reply_context: Option<ReplyContext>,

    /// Group subject/title
    pub group_subject: Option<String>,

    /// Group participants
    pub group_participants: Option<Vec<String>>,

    /// Mentioned JIDs/IDs
    pub mentioned_ids: Option<Vec<String>>,

    /// Whether the bot was mentioned
    pub was_mentioned: Option<bool>,

    /// Media attachment if present
    pub media: Option<MediaAttachment>,

    /// Source platform
    pub surface: String,

    /// Raw platform-specific data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

impl InboundMessage {
    /// Check if this message is from a group
    pub fn is_group(&self) -> bool {
        self.chat_type == ChatType::Group
    }

    /// Get the session key for routing
    pub fn session_key(&self) -> String {
        if self.is_group() {
            format!("{}:group:{}", self.surface, self.chat_id)
        } else {
            "main".to_string()
        }
    }
}

/// A reply to send back
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyPayload {
    /// Text content
    pub text: Option<String>,

    /// Single media URL
    pub media_url: Option<String>,

    /// Multiple media URLs
    pub media_urls: Option<Vec<String>>,

    /// Message ID to reply to (for threading)
    pub reply_to_id: Option<String>,

    /// Parse mode for text (markdown, html, etc.)
    pub parse_mode: Option<String>,
}

impl ReplyPayload {
    /// Create a text-only reply
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            media_url: None,
            media_urls: None,
            reply_to_id: None,
            parse_mode: None,
        }
    }

    /// Create a reply with media
    pub fn media(url: impl Into<String>, caption: Option<String>) -> Self {
        Self {
            text: caption,
            media_url: Some(url.into()),
            media_urls: None,
            reply_to_id: None,
            parse_mode: None,
        }
    }

    /// Set reply-to ID
    pub fn with_reply_to(mut self, id: impl Into<String>) -> Self {
        self.reply_to_id = Some(id.into());
        self
    }

    /// Set parse mode
    pub fn with_parse_mode(mut self, mode: impl Into<String>) -> Self {
        self.parse_mode = Some(mode.into());
        self
    }
}

/// Outbound message to send
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Recipient identifier
    pub to: String,

    /// Text content
    pub text: Option<String>,

    /// Media attachments
    pub media: Option<Vec<MediaAttachment>>,

    /// Message to reply to
    pub reply_to_id: Option<String>,

    /// Parse mode
    pub parse_mode: Option<String>,
}
