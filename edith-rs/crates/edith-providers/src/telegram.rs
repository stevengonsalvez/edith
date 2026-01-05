//! Telegram provider using teloxide

use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, InputFile, MessageId, ParseMode, ReplyParameters};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;

use crate::{
    ChatType, InboundMessage, MediaAttachment, MediaType, OutboundMessage, ProviderError,
    ProviderResult, ReplyContext, ReplyPayload,
};

/// Telegram provider
pub struct TelegramProvider {
    token: String,
    config: EdithConfig,
    bot: Arc<RwLock<Option<Bot>>>,
    connected: Arc<RwLock<bool>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<Option<mpsc::Receiver<InboundMessage>>>>,
}

impl TelegramProvider {
    /// Create a new Telegram provider
    pub fn new(token: String, config: EdithConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);

        Self {
            token,
            config,
            bot: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(Some(rx))),
        }
    }

    /// Get the bot instance
    fn bot(&self) -> ProviderResult<Bot> {
        self.bot
            .read()
            .clone()
            .ok_or_else(|| ProviderError::NotAvailable("bot not started".to_string()))
    }

    /// Get text chunk limit from config
    fn text_chunk_limit(&self) -> usize {
        self.config
            .telegram
            .as_ref()
            .and_then(|t| t.text_chunk_limit)
            .unwrap_or(4000) as usize
    }

    /// Get allow list from config
    fn allow_from(&self) -> Option<Vec<String>> {
        self.config
            .telegram
            .as_ref()
            .and_then(|t| t.allow_from.as_ref())
            .map(|list| {
                list.iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Number(n) => n.as_i64().map(|i| i.to_string()),
                        _ => None,
                    })
                    .collect()
            })
    }

    /// Check if a sender is allowed
    fn is_sender_allowed(&self, chat_id: i64) -> bool {
        match self.allow_from() {
            None => true, // No allowlist means allow all
            Some(list) if list.contains(&"*".to_string()) => true,
            Some(list) => {
                let id_str = chat_id.to_string();
                let prefixed = format!("telegram:{}", chat_id);
                list.contains(&id_str) || list.contains(&prefixed)
            }
        }
    }

    /// Check if group requires mention
    fn requires_group_mention(&self, chat_id: i64) -> bool {
        let groups = self
            .config
            .telegram
            .as_ref()
            .and_then(|t| t.groups.as_ref());

        if let Some(groups) = groups {
            let id_str = chat_id.to_string();

            // Check specific group config
            if let Some(group) = groups.get(&id_str) {
                if let Some(req) = group.require_mention {
                    return req;
                }
            }

            // Check wildcard default
            if let Some(default) = groups.get("*") {
                if let Some(req) = default.require_mention {
                    return req;
                }
            }
        }

        // Default to requiring mention in groups
        true
    }

    /// Check if bot was mentioned in message
    fn was_bot_mentioned(msg: &Message, bot_username: &str) -> bool {
        let text = msg.text().or(msg.caption()).unwrap_or("");
        let bot_mention = format!("@{}", bot_username.to_lowercase());

        if text.to_lowercase().contains(&bot_mention) {
            return true;
        }

        // Check entities for mentions
        if let Some(entities) = msg.entities().or(msg.caption_entities()) {
            for entity in entities {
                if entity.kind == teloxide::types::MessageEntityKind::Mention {
                    let mention = &text[entity.offset..entity.offset + entity.length];
                    if mention.to_lowercase() == bot_mention {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Convert teloxide message to our inbound message
    fn convert_message(&self, msg: &Message, bot_username: &str) -> Option<InboundMessage> {
        let chat_id = msg.chat.id.0;
        let is_group = msg.chat.is_group() || msg.chat.is_supergroup();

        // Check sender allowlist for direct chats
        if !is_group && !self.is_sender_allowed(chat_id) {
            debug!(chat_id = chat_id, "Blocked unauthorized sender");
            return None;
        }

        // Check mention requirement for groups
        let was_mentioned = is_group && Self::was_bot_mentioned(msg, bot_username);
        if is_group && self.requires_group_mention(chat_id) && !was_mentioned {
            debug!(chat_id = chat_id, "Skipping group message without mention");
            return None;
        }

        // Extract text
        let body = msg.text().or(msg.caption()).unwrap_or("").trim();
        if body.is_empty() && msg.photo().is_none() && msg.video().is_none() {
            return None;
        }

        // Build sender info
        let sender_name = msg.from().map(|u| {
            let first = &u.first_name;
            let last = u.last_name.as_deref().unwrap_or("");
            format!("{} {}", first, last).trim().to_string()
        });

        // Extract reply context
        let reply_context = msg.reply_to_message().map(|reply| {
            let reply_body = reply.text().or(reply.caption()).unwrap_or("").to_string();
            let reply_sender = reply.from().map(|u| u.first_name.clone());
            ReplyContext {
                message_id: reply.id.0.to_string(),
                body: Some(reply_body),
                sender: reply_sender,
            }
        });

        // Extract media
        let media = if let Some(photos) = msg.photo() {
            photos.last().map(|p| MediaAttachment {
                media_type: MediaType::Image,
                mime_type: "image/jpeg".to_string(),
                url: p.file.id.clone(),
                file_name: None,
                size: Some(p.file.size as u64),
                caption: msg.caption().map(|s| s.to_string()),
            })
        } else if let Some(video) = msg.video() {
            Some(MediaAttachment {
                media_type: MediaType::Video,
                mime_type: video.mime_type.as_ref().map(|m| m.to_string()).unwrap_or_else(|| "video/mp4".to_string()),
                url: video.file.id.clone(),
                file_name: video.file_name.clone(),
                size: Some(video.file.size as u64),
                caption: msg.caption().map(|s| s.to_string()),
            })
        } else if let Some(doc) = msg.document() {
            Some(MediaAttachment {
                media_type: MediaType::Document,
                mime_type: doc.mime_type.as_ref().map(|m| m.to_string()).unwrap_or_else(|| "application/octet-stream".to_string()),
                url: doc.file.id.clone(),
                file_name: doc.file_name.clone(),
                size: Some(doc.file.size as u64),
                caption: msg.caption().map(|s| s.to_string()),
            })
        } else {
            None
        };

        let group_subject = if is_group {
            msg.chat.title().map(|s| s.to_string())
        } else {
            None
        };

        Some(InboundMessage {
            id: Some(msg.id.0.to_string()),
            from: if is_group {
                format!("group:{}", chat_id)
            } else {
                format!("telegram:{}", chat_id)
            },
            conversation_id: chat_id.to_string(),
            to: format!("telegram:{}", chat_id),
            body: body.to_string(),
            sender_name,
            timestamp: Some(msg.date),
            chat_type: if is_group { ChatType::Group } else { ChatType::Direct },
            chat_id: chat_id.to_string(),
            sender_jid: None,
            sender_e164: None,
            reply_context,
            group_subject,
            group_participants: None,
            mentioned_ids: None,
            was_mentioned: Some(was_mentioned),
            media,
            surface: "telegram".to_string(),
            raw: None,
        })
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
impl crate::MessagingProvider for TelegramProvider {
    fn name(&self) -> &'static str {
        "telegram"
    }

    async fn start(&self) -> ProviderResult<()> {
        info!("Starting Telegram provider");

        let bot = Bot::new(&self.token);

        // Store bot instance
        *self.bot.write() = Some(bot.clone());
        *self.connected.write() = true;

        // Get bot info for username
        let me = bot.get_me().await.map_err(|e| {
            ProviderError::AuthFailed(format!("failed to get bot info: {}", e))
        })?;

        let bot_username = me.username().to_string();
        info!(username = %bot_username, "Telegram bot connected");

        // Start message handler in background
        let tx = self.message_tx.clone();
        let config = self.config.clone();
        let provider = Arc::new(Self::new(self.token.clone(), config));

        tokio::spawn(async move {
            let handler = Update::filter_message().endpoint(
                move |_bot: Bot, msg: Message| {
                    let tx = tx.clone();
                    let bot_username = bot_username.clone();
                    let provider = Arc::clone(&provider);
                    async move {
                        // Use a simple check here instead of full conversion
                        // since we need access to config
                        if let Some(inbound) = provider.convert_message(&msg, &bot_username) {
                            if tx.send(inbound).await.is_err() {
                                warn!("Failed to send inbound message to channel");
                            }
                        }
                        Ok::<(), std::convert::Infallible>(())
                    }
                },
            );

            Dispatcher::builder(bot, handler)
                .enable_ctrlc_handler()
                .build()
                .dispatch()
                .await;
        });

        Ok(())
    }

    async fn stop(&self) -> ProviderResult<()> {
        info!("Stopping Telegram provider");
        *self.connected.write() = false;
        *self.bot.write() = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }

    async fn send(&self, message: OutboundMessage) -> ProviderResult<String> {
        let bot = self.bot()?;
        let chat_id = ChatId(
            message
                .to
                .parse()
                .map_err(|e| ProviderError::InvalidRecipient(format!("invalid chat ID: {}", e)))?,
        );

        // Handle media first
        if let Some(media) = message.media {
            for attachment in media {
                let file = InputFile::url(
                    url::Url::parse(&attachment.url)
                        .map_err(|e| ProviderError::Internal(format!("invalid URL: {}", e)))?,
                );

                match attachment.media_type {
                    MediaType::Image => {
                        bot.send_photo(chat_id, file)
                            .caption(attachment.caption.unwrap_or_default())
                            .await
                            .map_err(|e| ProviderError::SendFailed(e.to_string()))?;
                    }
                    MediaType::Video => {
                        bot.send_video(chat_id, file)
                            .caption(attachment.caption.unwrap_or_default())
                            .await
                            .map_err(|e| ProviderError::SendFailed(e.to_string()))?;
                    }
                    MediaType::Audio => {
                        bot.send_audio(chat_id, file)
                            .caption(attachment.caption.unwrap_or_default())
                            .await
                            .map_err(|e| ProviderError::SendFailed(e.to_string()))?;
                    }
                    MediaType::Document | MediaType::Sticker => {
                        bot.send_document(chat_id, file)
                            .caption(attachment.caption.unwrap_or_default())
                            .await
                            .map_err(|e| ProviderError::SendFailed(e.to_string()))?;
                    }
                }
            }
        }

        // Send text
        if let Some(text) = message.text {
            let chunks = self.chunk_text(&text);
            let mut last_id = None;

            for chunk in chunks {
                let mut req = bot.send_message(chat_id, &chunk);

                if message.parse_mode.as_deref() == Some("Markdown") {
                    req = req.parse_mode(ParseMode::MarkdownV2);
                } else if message.parse_mode.as_deref() == Some("HTML") {
                    req = req.parse_mode(ParseMode::Html);
                }

                if let Some(ref reply_id) = message.reply_to_id {
                    if let Ok(id) = reply_id.parse::<i32>() {
                        req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                    }
                }

                let result = req.await;

                match result {
                    Ok(msg) => {
                        last_id = Some(msg.id.0.to_string());
                    }
                    Err(e) => {
                        // Retry without parse mode if markdown failed
                        if message.parse_mode.is_some() {
                            let retry = bot.send_message(chat_id, &chunk).await;
                            match retry {
                                Ok(msg) => {
                                    last_id = Some(msg.id.0.to_string());
                                }
                                Err(e) => {
                                    return Err(ProviderError::SendFailed(e.to_string()));
                                }
                            }
                        } else {
                            return Err(ProviderError::SendFailed(e.to_string()));
                        }
                    }
                }
            }

            return Ok(last_id.unwrap_or_else(|| "unknown".to_string()));
        }

        Ok("sent".to_string())
    }

    async fn send_reply(&self, chat_id: &str, reply: ReplyPayload) -> ProviderResult<String> {
        self.send(OutboundMessage {
            to: chat_id.to_string(),
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
        let bot = self.bot()?;
        let chat_id = ChatId(
            chat_id
                .parse()
                .map_err(|e| ProviderError::InvalidRecipient(format!("invalid chat ID: {}", e)))?,
        );

        bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
            .await
            .map_err(|e| ProviderError::Internal(format!("failed to send typing: {}", e)))?;

        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<InboundMessage> {
        // Take the receiver - can only be called once
        self.message_rx
            .write()
            .take()
            .expect("subscribe can only be called once")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text() {
        let config = EdithConfig::default();
        let provider = TelegramProvider::new("test".to_string(), config);

        let text = "Hello World";
        let chunks = provider.chunk_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }
}
