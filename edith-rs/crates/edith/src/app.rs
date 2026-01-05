//! Main Edith application

use std::collections::HashMap;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use edith_config::EdithConfig;
use edith_gateway::GatewayServer;
use edith_providers::MessagingProvider;

use crate::bot::Bot;
use crate::error::{EdithError, EdithResult};
use crate::event_processor::EventProcessor;

/// Main Edith application
pub struct Edith {
    config: EdithConfig,
    bot: Arc<RwLock<Bot>>,
    gateway: Option<GatewayServer>,
}

impl Edith {
    /// Create a new Edith instance
    pub fn new(config: EdithConfig) -> EdithResult<Self> {
        let bot = Bot::new(config.clone())?;

        Ok(Self {
            config,
            bot: Arc::new(RwLock::new(bot)),
            gateway: None,
        })
    }

    /// Start and run the application
    pub async fn run(&mut self) -> EdithResult<()> {
        info!("Starting Edith...");

        // Start the bot
        {
            let mut bot = self.bot.write().await;
            bot.start().await?;
        }

        // Start the gateway if configured
        if let Some(ref gateway_config) = self.config.gateway {
            if gateway_config.enabled.unwrap_or(true) {
                info!("Starting HTTP gateway...");
                let init = GatewayServer::new(self.config.clone());
                self.gateway = Some(init.server);

                // The gateway runs in the background
                let gateway = self.gateway.as_ref().unwrap().clone();
                tokio::spawn(async move {
                    if let Err(e) = gateway.run().await {
                        error!(error = %e, "Gateway error");
                    }
                });

                // Start event processor to handle dispatched events
                let bot = self.bot.read().await;
                let session_manager = bot.session_manager();
                let llm_client = bot.llm_client();

                // Create empty providers map for now
                // TODO: Get providers from bot when we have proper accessor
                let providers: Arc<RwLock<HashMap<String, Arc<dyn MessagingProvider>>>> =
                    Arc::new(RwLock::new(HashMap::new()));

                let (event_processor, mut heartbeat_rx, _shutdown_rx) = EventProcessor::new(
                    self.config.clone(),
                    session_manager,
                    llm_client,
                    providers,
                );

                // Start the event processor
                tokio::spawn(async move {
                    event_processor.start(init.receivers).await;
                });

                // Handle heartbeat requests
                tokio::spawn(async move {
                    while let Some(request) = heartbeat_rx.recv().await {
                        debug!(reason = %request.reason, "Heartbeat requested");
                        // TODO: Trigger actual heartbeat/agent run
                    }
                });
            }
        }

        info!("Edith started successfully");

        // Wait for shutdown signal
        self.wait_for_shutdown().await?;

        Ok(())
    }

    /// Wait for shutdown signal (Ctrl+C or SIGTERM)
    async fn wait_for_shutdown(&self) -> EdithResult<()> {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C, shutting down...");
            }
            _ = terminate => {
                info!("Received SIGTERM, shutting down...");
            }
        }

        self.shutdown().await
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> EdithResult<()> {
        info!("Shutting down Edith...");

        // Stop the bot
        {
            let mut bot = self.bot.write().await;
            bot.stop().await?;
        }

        // Stop the gateway
        if let Some(ref gateway) = self.gateway {
            gateway.shutdown().await;
        }

        info!("Edith shutdown complete");
        Ok(())
    }

    /// Get a reference to the bot
    pub fn bot(&self) -> Arc<RwLock<Bot>> {
        Arc::clone(&self.bot)
    }

    /// Get the config
    pub fn config(&self) -> &EdithConfig {
        &self.config
    }
}

/// Builder for Edith
pub struct EdithBuilder {
    config: Option<EdithConfig>,
    config_path: Option<String>,
}

impl EdithBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            config_path: None,
        }
    }

    /// Set configuration directly
    pub fn config(mut self, config: EdithConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Load configuration from file
    pub fn config_file(mut self, path: impl Into<String>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Build the Edith instance
    pub fn build(self) -> EdithResult<Edith> {
        let config = if let Some(config) = self.config {
            config
        } else if let Some(path) = self.config_path {
            EdithConfig::from_file(&path).map_err(EdithError::Config)?
        } else {
            EdithConfig::default()
        };

        Edith::new(config)
    }
}

impl Default for EdithBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let builder = EdithBuilder::new().config(EdithConfig::default());
        // Can't build without API key, but builder works
        assert!(builder.config.is_some());
    }
}
