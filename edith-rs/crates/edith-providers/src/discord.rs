//! Discord provider using serenity

use async_trait::async_trait;
use serenity::all::{
    ChannelId, Client, Context, EventHandler, GatewayIntents,
    Message, Ready,
};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;

use crate::{
    ChatType, InboundMessage, MediaAttachment, MediaType, OutboundMessage, ProviderError,
    ProviderResult, ReplyContext, ReplyPayload,
};

const DISCORD_TEXT_LIMIT: usize = 2000;

/// Discord event handler
struct DiscordHandler {
    message_tx: mpsc::Sender<InboundMessage>,
    bot_user_id: std::sync::RwLock<Option<u64>>,
    config: EdithConfig,
}

impl DiscordHandler {
    fn new(message_tx: mpsc::Sender<InboundMessage>, config: EdithConfig) -> Self {
        Self {
            message_tx,
            bot_user_id: std::sync::RwLock::new(None),
            config,
        }
    }

    /// Check if sender is allowed
    fn is_sender_allowed(&self, user_id: u64, _channel_id: u64) -> bool {
        let discord_config = match self.config.discord.as_ref() {
            Some(c) => c,
            None => return true,
        };

        // Check DM allow list
        if let Some(ref dm) = discord_config.dm {
            if let Some(ref allow_from) = dm.allow_from {
                // Check for wildcard
                let has_wildcard = allow_from.iter().any(|v| {
                    v.as_str().map_or(false, |s| s == "*")
                });

                if !allow_from.is_empty() && !has_wildcard {
                    let user_str = user_id.to_string();
                    let prefixed = format!("discord:{}", user_id);

                    let is_allowed = allow_from.iter().any(|v| {
                        match v {
                            serde_json::Value::String(s) => s == &user_str || s == &prefixed,
                            serde_json::Value::Number(n) => n.as_u64() == Some(user_id),
                            _ => false,
                        }
                    });

                    if !is_allowed {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Check if message requires bot mention in guild
    fn requires_mention(&self, guild_id: Option<u64>) -> bool {
        if guild_id.is_none() {
            return false; // DMs don't require mention
        }

        let discord_config = match self.config.discord.as_ref() {
            Some(c) => c,
            None => return true, // Default to requiring mention in guilds
        };

        // Check guild-specific config
        if let Some(ref guilds) = discord_config.guilds {
            if let Some(guild_id) = guild_id {
                let guild_str = guild_id.to_string();
                if let Some(guild_config) = guilds.get(&guild_str) {
                    if let Some(require_mention) = guild_config.require_mention {
                        return require_mention;
                    }
                }

                // Check wildcard
                if let Some(default_config) = guilds.get("*") {
                    if let Some(require_mention) = default_config.require_mention {
                        return require_mention;
                    }
                }
            }
        }

        true // Default to requiring mention
    }

    /// Check if bot was mentioned in message
    fn was_bot_mentioned(&self, msg: &Message) -> bool {
        let bot_id = self.bot_user_id.read().ok().and_then(|g| *g);
        if let Some(id) = bot_id {
            msg.mentions.iter().any(|u| u.id.get() == id)
        } else {
            false
        }
    }

    /// Convert Discord message to InboundMessage
    fn convert_message(&self, msg: &Message) -> Option<InboundMessage> {
        // Skip bot messages
        if msg.author.bot {
            return None;
        }

        let user_id = msg.author.id.get();
        let channel_id = msg.channel_id.get();
        let guild_id = msg.guild_id.map(|g| g.get());

        // Check if sender is allowed
        if !self.is_sender_allowed(user_id, channel_id) {
            debug!(user_id = user_id, "Blocked unauthorized Discord user");
            return None;
        }

        // Check mention requirement for guilds
        let is_dm = guild_id.is_none();
        let was_mentioned = self.was_bot_mentioned(msg);
        if !is_dm && self.requires_mention(guild_id) && !was_mentioned {
            debug!(channel_id = channel_id, "Skipping guild message without mention");
            return None;
        }

        // Extract text content
        let body = msg.content.trim();
        if body.is_empty() && msg.attachments.is_empty() {
            return None;
        }

        // Build sender name
        let sender_name = Some(
            if let Some(disc) = msg.author.discriminator {
                format!("{}#{}", msg.author.name, disc)
            } else {
                msg.author.name.clone()
            }
        );

        // Extract reply context
        let reply_context = msg.referenced_message.as_ref().map(|reply| ReplyContext {
            message_id: reply.id.get().to_string(),
            body: Some(reply.content.clone()),
            sender: Some(reply.author.name.clone()),
        });

        // Extract media attachments
        let media = if !msg.attachments.is_empty() {
            Some(
                msg.attachments
                    .first()
                    .map(|a| {
                        let media_type = if a.content_type.as_deref().map_or(false, |t| t.starts_with("image/")) {
                            MediaType::Image
                        } else if a.content_type.as_deref().map_or(false, |t| t.starts_with("video/")) {
                            MediaType::Video
                        } else if a.content_type.as_deref().map_or(false, |t| t.starts_with("audio/")) {
                            MediaType::Audio
                        } else {
                            MediaType::Document
                        };

                        MediaAttachment {
                            media_type,
                            mime_type: a.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string()),
                            url: a.url.clone(),
                            file_name: Some(a.filename.clone()),
                            size: Some(a.size as u64),
                            caption: None,
                        }
                    })
                    .unwrap(),
            )
        } else {
            None
        };

        let chat_type = if is_dm {
            ChatType::Direct
        } else {
            ChatType::Group
        };

        Some(InboundMessage {
            id: Some(msg.id.get().to_string()),
            from: format!("discord:{}", user_id),
            conversation_id: channel_id.to_string(),
            to: format!("channel:{}", channel_id),
            body: body.to_string(),
            sender_name,
            timestamp: Some(chrono::DateTime::from_timestamp(msg.timestamp.unix_timestamp(), 0)
                .unwrap_or_else(chrono::Utc::now)),
            chat_type,
            chat_id: channel_id.to_string(),
            sender_jid: None,
            sender_e164: None,
            reply_context,
            group_subject: None, // Could fetch guild/channel name
            group_participants: None,
            mentioned_ids: Some(msg.mentions.iter().map(|u| u.id.get().to_string()).collect()),
            was_mentioned: Some(was_mentioned),
            media,
            surface: "discord".to_string(),
            raw: None,
        })
    }
}

#[async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!(username = %ready.user.name, "Discord bot connected");
        if let Ok(mut guard) = self.bot_user_id.write() {
            *guard = Some(ready.user.id.get());
        }
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        if let Some(inbound) = self.convert_message(&msg) {
            if self.message_tx.send(inbound).await.is_err() {
                warn!("Failed to send Discord inbound message to channel");
            }
        }
    }
}

/// Discord provider
pub struct DiscordProvider {
    token: String,
    config: EdithConfig,
    client: Arc<RwLock<Option<Client>>>,
    connected: Arc<RwLock<bool>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<Option<mpsc::Receiver<InboundMessage>>>>,
}

impl DiscordProvider {
    /// Create a new Discord provider
    pub fn new(token: String, config: EdithConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);

        Self {
            token,
            config,
            client: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(Some(rx))),
        }
    }

    /// Get text chunk limit from config
    fn text_chunk_limit(&self) -> usize {
        self.config
            .discord
            .as_ref()
            .and_then(|d| d.text_chunk_limit)
            .unwrap_or(DISCORD_TEXT_LIMIT as u64) as usize
    }

    /// Chunk text to respect platform limits
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let limit = self.text_chunk_limit();
        if text.len() <= limit {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for line in text.lines() {
            if current.len() + line.len() + 1 > limit {
                if !current.is_empty() {
                    chunks.push(current);
                    current = String::new();
                }
                // Handle lines longer than limit
                if line.len() > limit {
                    for chunk in line.as_bytes().chunks(limit) {
                        chunks.push(String::from_utf8_lossy(chunk).to_string());
                    }
                    continue;
                }
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }
}

#[async_trait]
impl crate::MessagingProvider for DiscordProvider {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn start(&self) -> ProviderResult<()> {
        info!("Starting Discord provider");

        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILD_MESSAGE_REACTIONS
            | GatewayIntents::DIRECT_MESSAGE_REACTIONS;

        let handler = DiscordHandler::new(self.message_tx.clone(), self.config.clone());

        let client = Client::builder(&self.token, intents)
            .event_handler(handler)
            .await
            .map_err(|e| ProviderError::AuthFailed(format!("Discord client creation failed: {}", e)))?;

        *self.client.write().await = Some(client);
        *self.connected.write().await = true;

        // Start the client in a background task
        let client = self.client.clone();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            // Take the client outside the lock scope
            let maybe_client = {
                let mut guard = client.write().await;
                guard.take()
            };

            if let Some(mut c) = maybe_client {
                if let Err(e) = c.start().await {
                    error!(error = %e, "Discord client error");
                }
            }
            *connected.write().await = false;
        });

        Ok(())
    }

    async fn stop(&self) -> ProviderResult<()> {
        info!("Stopping Discord provider");
        *self.connected.write().await = false;
        // Client will be dropped when the task ends
        *self.client.write().await = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Use try_read to avoid blocking
        self.connected.try_read().map(|g| *g).unwrap_or(false)
    }

    async fn send(&self, message: OutboundMessage) -> ProviderResult<String> {
        // Parse channel ID from recipient
        let channel_id: u64 = message
            .to
            .strip_prefix("channel:")
            .unwrap_or(&message.to)
            .parse()
            .map_err(|e| ProviderError::InvalidRecipient(format!("invalid channel ID: {}", e)))?;

        let channel = ChannelId::new(channel_id);

        // Get HTTP client from serenity
        // Note: For sending without an active client connection, we'd need to use Discord REST API directly
        // For now, we'll use the HTTP client from reqwest

        let http_client = reqwest::Client::new();
        let base_url = "https://discord.com/api/v10";

        // Send text message(s)
        if let Some(text) = message.text {
            let chunks = self.chunk_text(&text);
            let mut last_id = None;

            for (i, chunk) in chunks.iter().enumerate() {
                let body = serde_json::json!({
                    "content": chunk,
                    "message_reference": if i == 0 {
                        message.reply_to_id.as_ref().map(|id| serde_json::json!({
                            "message_id": id,
                            "fail_if_not_exists": false
                        }))
                    } else {
                        None
                    }
                });

                let response = http_client
                    .post(format!("{}/channels/{}/messages", base_url, channel_id))
                    .header("Authorization", format!("Bot {}", self.token))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| ProviderError::SendFailed(e.to_string()))?;

                if !response.status().is_success() {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    return Err(ProviderError::SendFailed(format!(
                        "Discord API error {}: {}",
                        status, text
                    )));
                }

                let result: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| ProviderError::SendFailed(e.to_string()))?;

                if let Some(id) = result["id"].as_str() {
                    last_id = Some(id.to_string());
                }
            }

            return Ok(last_id.unwrap_or_else(|| "sent".to_string()));
        }

        Ok("sent".to_string())
    }

    async fn send_reply(&self, chat_id: &str, reply: ReplyPayload) -> ProviderResult<String> {
        self.send(OutboundMessage {
            to: format!("channel:{}", chat_id),
            text: reply.text,
            media: reply.media_url.map(|url| {
                vec![MediaAttachment {
                    media_type: MediaType::Document,
                    mime_type: "application/octet-stream".to_string(),
                    url,
                    file_name: None,
                    size: None,
                    caption: None,
                }]
            }),
            reply_to_id: reply.reply_to_id,
            parse_mode: reply.parse_mode,
        })
        .await
    }

    async fn send_typing(&self, chat_id: &str) -> ProviderResult<()> {
        let channel_id: u64 = chat_id
            .parse()
            .map_err(|e| ProviderError::InvalidRecipient(format!("invalid channel ID: {}", e)))?;

        let http_client = reqwest::Client::new();
        let base_url = "https://discord.com/api/v10";

        http_client
            .post(format!("{}/channels/{}/typing", base_url, channel_id))
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
            .map_err(|e| ProviderError::Internal(format!("failed to send typing: {}", e)))?;

        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<InboundMessage> {
        // Block on the async lock for subscribe (called during initialization)
        futures::executor::block_on(async {
            self.message_rx
                .write()
                .await
                .take()
                .expect("subscribe can only be called once")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text() {
        let config = EdithConfig::default();
        let provider = DiscordProvider::new("test".to_string(), config);

        let text = "Hello World";
        let chunks = provider.chunk_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }
}
