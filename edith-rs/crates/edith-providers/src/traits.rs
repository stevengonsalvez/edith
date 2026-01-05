//! Provider traits

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{InboundMessage, OutboundMessage, ProviderResult, ReplyPayload};

/// Trait for messaging providers
#[async_trait]
pub trait MessagingProvider: Send + Sync {
    /// Provider name
    fn name(&self) -> &'static str;

    /// Start the provider
    async fn start(&self) -> ProviderResult<()>;

    /// Stop the provider
    async fn stop(&self) -> ProviderResult<()>;

    /// Check if the provider is connected
    fn is_connected(&self) -> bool;

    /// Send a message
    async fn send(&self, message: OutboundMessage) -> ProviderResult<String>;

    /// Send a reply to a specific chat
    async fn send_reply(
        &self,
        chat_id: &str,
        reply: ReplyPayload,
    ) -> ProviderResult<String>;

    /// Send a typing indicator
    async fn send_typing(&self, chat_id: &str) -> ProviderResult<()>;

    /// Subscribe to incoming messages
    fn subscribe(&self) -> mpsc::Receiver<InboundMessage>;
}

/// Trait for providers that support webhooks
#[async_trait]
pub trait WebhookProvider: MessagingProvider {
    /// Get the webhook path
    fn webhook_path(&self) -> &str;

    /// Handle a webhook request
    async fn handle_webhook(
        &self,
        body: bytes::Bytes,
        headers: std::collections::HashMap<String, String>,
    ) -> ProviderResult<()>;
}

/// Trait for providers that support media
#[async_trait]
pub trait MediaProvider: MessagingProvider {
    /// Download media by ID
    async fn download_media(&self, media_id: &str) -> ProviderResult<bytes::Bytes>;

    /// Upload media
    async fn upload_media(
        &self,
        data: bytes::Bytes,
        mime_type: &str,
        file_name: Option<&str>,
    ) -> ProviderResult<String>;

    /// Get media URL
    fn get_media_url(&self, media_id: &str) -> Option<String>;
}

/// Provider factory
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a provider from config
    pub fn create_telegram(
        config: &edith_config::EdithConfig,
    ) -> ProviderResult<Box<dyn MessagingProvider>> {
        let token = edith_config::get_telegram_token(config)
            .ok_or_else(|| crate::ProviderError::NotConfigured("telegram".to_string()))?;

        Ok(Box::new(crate::telegram::TelegramProvider::new(token, config.clone())))
    }

    /// Create all enabled providers from config
    pub fn create_all(
        config: &edith_config::EdithConfig,
    ) -> Vec<Box<dyn MessagingProvider>> {
        let mut providers: Vec<Box<dyn MessagingProvider>> = Vec::new();

        // Telegram
        if let Ok(provider) = Self::create_telegram(config) {
            providers.push(provider);
        }

        // TODO: Add other providers as they're implemented

        providers
    }
}
