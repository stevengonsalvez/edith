//! Bot manager for coordinating providers and LLM

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;
use edith_llm::{AnthropicClient, LlmClient};
use edith_providers::{MessagingProvider, TelegramProvider};
use edith_session::SessionManager;

use crate::error::{EdithError, EdithResult};
use crate::pipeline::{ContentFilterStage, MessagePipeline, PreprocessStage};

/// Bot state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotState {
    Stopped,
    Starting,
    Running,
    Stopping,
}

/// Bot manages messaging providers and LLM interaction
pub struct Bot {
    config: EdithConfig,
    state: Arc<RwLock<BotState>>,
    providers: Arc<RwLock<HashMap<String, Arc<dyn MessagingProvider>>>>,
    session_manager: Arc<SessionManager>,
    llm_client: Arc<dyn LlmClient>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Bot {
    /// Create a new bot
    pub fn new(config: EdithConfig) -> EdithResult<Self> {
        // Create session manager
        let session_manager = Arc::new(SessionManager::new(config.clone()));

        // Create LLM client
        let api_key = config
            .llm
            .as_ref()
            .and_then(|l| l.api_key.clone())
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or(EdithError::Internal("No API key configured".to_string()))?;

        let llm_client: Arc<dyn LlmClient> = Arc::new(
            AnthropicClient::new(&api_key).map_err(|e| EdithError::Llm(e))?,
        );

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(BotState::Stopped)),
            providers: Arc::new(RwLock::new(HashMap::new())),
            session_manager,
            llm_client,
            shutdown_tx: None,
        })
    }

    /// Get current state
    pub async fn state(&self) -> BotState {
        *self.state.read().await
    }

    /// Start the bot
    pub async fn start(&mut self) -> EdithResult<()> {
        {
            let mut state = self.state.write().await;
            if *state != BotState::Stopped {
                return Err(EdithError::Internal("Bot already started".to_string()));
            }
            *state = BotState::Starting;
        }

        info!("Starting bot...");

        // Initialize providers
        self.init_providers().await?;

        // Start all providers
        let providers = self.providers.read().await;
        for (name, provider) in providers.iter() {
            info!(provider = %name, "Starting provider");
            provider.start().await.map_err(EdithError::Provider)?;
        }

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start message processing loops
        let providers_clone = Arc::clone(&self.providers);
        let config = self.config.clone();
        let session_manager = Arc::clone(&self.session_manager);
        let llm_client = Arc::clone(&self.llm_client);
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            let pipeline = MessagePipeline::new(config, session_manager, llm_client)
                .add_stage(Box::new(PreprocessStage))
                .add_stage(Box::new(ContentFilterStage));

            // Subscribe to each provider
            let providers = providers_clone.read().await;
            for (name, provider) in providers.iter() {
                let mut rx = provider.subscribe();
                let pipeline = &pipeline;
                let provider = Arc::clone(provider);
                let name = name.clone();

                // Process messages from this provider
                while let Some(message) = rx.recv().await {
                    debug!(
                        provider = %name,
                        from = %message.from,
                        "Received message"
                    );

                    match pipeline.process(message, provider.as_ref()).await {
                        Ok(Some(_response)) => {
                            debug!(provider = %name, "Response sent");
                        }
                        Ok(None) => {
                            debug!(provider = %name, "No response needed");
                        }
                        Err(e) => {
                            error!(provider = %name, error = %e, "Pipeline error");
                        }
                    }
                }
            }

            // Wait for shutdown
            shutdown_rx.recv().await;
            *state.write().await = BotState::Stopped;
        });

        *self.state.write().await = BotState::Running;
        info!("Bot started");

        Ok(())
    }

    /// Stop the bot
    pub async fn stop(&mut self) -> EdithResult<()> {
        {
            let mut state = self.state.write().await;
            if *state != BotState::Running {
                return Ok(());
            }
            *state = BotState::Stopping;
        }

        info!("Stopping bot...");

        // Stop all providers
        let providers = self.providers.read().await;
        for (name, provider) in providers.iter() {
            info!(provider = %name, "Stopping provider");
            if let Err(e) = provider.stop().await {
                warn!(provider = %name, error = %e, "Error stopping provider");
            }
        }

        // Signal shutdown
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        *self.state.write().await = BotState::Stopped;
        info!("Bot stopped");

        Ok(())
    }

    /// Initialize messaging providers based on config
    async fn init_providers(&self) -> EdithResult<()> {
        let mut providers = self.providers.write().await;

        // Initialize Telegram if configured
        if let Some(ref telegram) = self.config.telegram {
            if telegram.enabled.unwrap_or(false) {
                let token = telegram
                    .bot_token
                    .clone()
                    .or_else(|| std::env::var("TELEGRAM_BOT_TOKEN").ok())
                    .ok_or_else(|| {
                        EdithError::Internal("Telegram token not configured".to_string())
                    })?;

                let provider = TelegramProvider::new(token, self.config.clone());
                providers.insert("telegram".to_string(), Arc::new(provider));
                info!("Telegram provider initialized");
            }
        }

        // Initialize WhatsApp if configured (stub for now)
        if let Some(ref whatsapp) = self.config.whatsapp {
            if whatsapp.enabled.unwrap_or(false) {
                use edith_providers::WhatsAppProvider;
                let provider = WhatsAppProvider::new(self.config.clone());
                if provider.has_bridge() {
                    providers.insert("whatsapp".to_string(), Arc::new(provider));
                    info!("WhatsApp provider initialized (bridge mode)");
                } else {
                    warn!("WhatsApp enabled but no bridge configured");
                }
            }
        }

        if providers.is_empty() {
            warn!("No messaging providers configured");
        }

        Ok(())
    }

    /// Get session manager
    pub fn session_manager(&self) -> Arc<SessionManager> {
        Arc::clone(&self.session_manager)
    }

    /// Get LLM client
    pub fn llm_client(&self) -> Arc<dyn LlmClient> {
        Arc::clone(&self.llm_client)
    }

    /// Check if a provider is connected
    pub async fn is_provider_connected(&self, name: &str) -> bool {
        let providers = self.providers.read().await;
        providers
            .get(name)
            .map(|p| p.is_connected())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bot_state() {
        // Can't fully test without config, but we can test the state machine
        let state = Arc::new(RwLock::new(BotState::Stopped));
        assert_eq!(*state.read().await, BotState::Stopped);

        *state.write().await = BotState::Starting;
        assert_eq!(*state.read().await, BotState::Starting);

        *state.write().await = BotState::Running;
        assert_eq!(*state.read().await, BotState::Running);
    }
}
