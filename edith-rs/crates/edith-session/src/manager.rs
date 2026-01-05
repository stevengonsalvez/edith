//! Session manager for coordinating sessions and messages

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

use edith_config::EdithConfig;

use crate::error::{SessionError, SessionResult};
use crate::session::SessionEntry;

/// Role in a conversation message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Simple conversation message stored in session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: i64,
}

impl ConversationMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Session with conversation history
#[derive(Debug, Clone, Default)]
pub struct Session {
    /// Session key
    pub key: String,
    /// Session entry (metadata)
    pub entry: SessionEntry,
    /// Conversation messages
    pub messages: Vec<ConversationMessage>,
}

/// Session manager handles session lifecycle and message storage
pub struct SessionManager {
    config: EdithConfig,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    max_messages: usize,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: EdithConfig) -> Self {
        // Default to 100 messages max per session
        let max_messages = 100usize;

        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            max_messages,
        }
    }

    /// Get or create a session by key
    pub async fn get_or_create(&self, key: &str) -> SessionResult<Session> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(key) {
            return Ok(session.clone());
        }
        drop(sessions);

        // Create new session
        let session = Session {
            key: key.to_string(),
            entry: SessionEntry::new(),
            messages: Vec::new(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(key.to_string(), session.clone());

        info!(key = %key, "Created new session");
        Ok(session)
    }

    /// Get a session by key
    pub async fn get(&self, key: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(key).cloned()
    }

    /// Add a message to a session
    pub async fn add_message(&self, key: &str, message: ConversationMessage) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(key) {
            session.messages.push(message);

            // Trim if exceeds max
            if session.messages.len() > self.max_messages {
                let remove_count = session.messages.len() - self.max_messages;
                session.messages.drain(0..remove_count);
            }

            session.entry.touch();
        } else {
            return Err(SessionError::NotFound { key: key.to_string() });
        }

        Ok(())
    }

    /// Get messages from a session
    pub async fn get_messages(&self, key: &str) -> SessionResult<Vec<ConversationMessage>> {
        let sessions = self.sessions.read().await;

        if let Some(session) = sessions.get(key) {
            Ok(session.messages.clone())
        } else {
            Err(SessionError::NotFound { key: key.to_string() })
        }
    }

    /// Clear messages from a session
    pub async fn clear_messages(&self, key: &str) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(key) {
            session.messages.clear();
            session.entry.touch();
            Ok(())
        } else {
            Err(SessionError::NotFound { key: key.to_string() })
        }
    }

    /// Delete a session
    pub async fn delete(&self, key: &str) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(key);
        debug!(key = %key, "Deleted session");
        Ok(())
    }

    /// List all session keys
    pub async fn list_keys(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_manager() {
        let config = EdithConfig::default();
        let manager = SessionManager::new(config);

        // Create session
        let session = manager.get_or_create("test").await.unwrap();
        assert_eq!(session.key, "test");

        // Add message
        manager.add_message("test", ConversationMessage::user("Hello")).await.unwrap();

        // Get messages
        let messages = manager.get_messages("test").await.unwrap();
        assert_eq!(messages.len(), 1);
    }
}
