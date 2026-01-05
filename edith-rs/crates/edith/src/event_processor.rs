//! Event processor for handling gateway dispatched events
//!
//! This module consumes events from the gateway's EventReceivers and routes them
//! to the appropriate handlers (bot, session manager, providers).

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;
use edith_gateway::{AgentRunRequest, DeliveryChannel, EventReceivers, WakeMode};
use edith_llm::{CompletionRequest, LlmClient, Message as LlmMessage};
use edith_providers::MessagingProvider;
use edith_session::{ConversationMessage, SessionManager};

/// System event queue for heartbeat processing
pub struct SystemEventQueue {
    events: RwLock<Vec<String>>,
}

impl SystemEventQueue {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::new()),
        }
    }

    /// Enqueue a system event
    pub async fn enqueue(&self, text: String) {
        if text.is_empty() {
            return;
        }
        let mut events = self.events.write().await;
        // Deduplicate consecutive identical events
        if events.last().map(|s| s.as_str()) != Some(&text) {
            events.push(text);
        }
    }

    /// Drain all queued events
    pub async fn drain(&self) -> Vec<String> {
        let mut events = self.events.write().await;
        std::mem::take(&mut *events)
    }

    /// Check if there are pending events
    pub async fn has_pending(&self) -> bool {
        !self.events.read().await.is_empty()
    }
}

impl Default for SystemEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Event processor handles dispatched events from the gateway
pub struct EventProcessor {
    config: EdithConfig,
    system_events: Arc<SystemEventQueue>,
    session_manager: Arc<SessionManager>,
    llm_client: Arc<dyn LlmClient>,
    providers: Arc<RwLock<std::collections::HashMap<String, Arc<dyn MessagingProvider>>>>,
    heartbeat_tx: mpsc::Sender<HeartbeatRequest>,
    shutdown_tx: mpsc::Sender<()>,
}

/// Request to trigger a heartbeat
#[derive(Debug, Clone)]
pub struct HeartbeatRequest {
    pub reason: String,
}

impl EventProcessor {
    /// Create a new event processor
    pub fn new(
        config: EdithConfig,
        session_manager: Arc<SessionManager>,
        llm_client: Arc<dyn LlmClient>,
        providers: Arc<RwLock<std::collections::HashMap<String, Arc<dyn MessagingProvider>>>>,
    ) -> (Self, mpsc::Receiver<HeartbeatRequest>, mpsc::Receiver<()>) {
        let (heartbeat_tx, heartbeat_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let processor = Self {
            config,
            system_events: Arc::new(SystemEventQueue::new()),
            session_manager,
            llm_client,
            providers,
            heartbeat_tx,
            shutdown_tx,
        };

        (processor, heartbeat_rx, shutdown_rx)
    }

    /// Get the system event queue
    pub fn system_events(&self) -> Arc<SystemEventQueue> {
        Arc::clone(&self.system_events)
    }

    /// Start processing events from receivers
    pub async fn start(self, mut receivers: EventReceivers) {
        let system_events = Arc::clone(&self.system_events);
        let heartbeat_tx = self.heartbeat_tx.clone();
        let config = self.config.clone();
        let session_manager = Arc::clone(&self.session_manager);
        let llm_client = Arc::clone(&self.llm_client);
        let providers = Arc::clone(&self.providers);

        // Spawn wake event handler
        let system_events_wake = Arc::clone(&system_events);
        let heartbeat_tx_wake = heartbeat_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = receivers.wake_rx.recv().await {
                debug!(
                    text = %event.text,
                    mode = ?event.mode,
                    source = %event.source,
                    "Processing wake event"
                );

                // Enqueue the system event
                system_events_wake.enqueue(event.text).await;

                // Request heartbeat if mode is Now
                if matches!(event.mode, WakeMode::Now) {
                    if let Err(e) = heartbeat_tx_wake
                        .send(HeartbeatRequest {
                            reason: event.source,
                        })
                        .await
                    {
                        error!(error = %e, "Failed to send heartbeat request");
                    }
                }
            }
        });

        // Spawn agent run handler
        tokio::spawn(async move {
            while let Some(request) = receivers.agent_rx.recv().await {
                info!(
                    run_id = %request.run_id,
                    name = %request.name,
                    channel = ?request.channel,
                    "Processing agent run request"
                );

                // Process the agent run
                Self::process_agent_run(
                    request,
                    Arc::clone(&system_events),
                    heartbeat_tx.clone(),
                    config.clone(),
                    Arc::clone(&session_manager),
                    Arc::clone(&llm_client),
                    Arc::clone(&providers),
                )
                .await;
            }
        });
    }

    /// Process an agent run request
    async fn process_agent_run(
        request: AgentRunRequest,
        system_events: Arc<SystemEventQueue>,
        heartbeat_tx: mpsc::Sender<HeartbeatRequest>,
        _config: EdithConfig,
        session_manager: Arc<SessionManager>,
        llm_client: Arc<dyn LlmClient>,
        providers: Arc<RwLock<std::collections::HashMap<String, Arc<dyn MessagingProvider>>>>,
    ) {
        let run_id = request.run_id.clone();
        let name = request.name.clone();
        let wake_mode = request.wake_mode;

        // Get or create session
        let session_key = if request.session_key.is_empty() {
            format!("hook:{}", run_id)
        } else {
            request.session_key.clone()
        };

        // Run agent turn
        match Self::run_agent_turn(
            &request.message,
            &session_key,
            Arc::clone(&session_manager),
            Arc::clone(&llm_client),
        )
        .await
        {
            Ok(response) => {
                info!(run_id = %run_id, "Agent run completed");

                // Deliver response if configured
                if request.deliver {
                    if let Err(e) = Self::deliver_response(
                        &response,
                        request.channel,
                        request.to.as_deref(),
                        providers,
                    )
                    .await
                    {
                        error!(run_id = %run_id, error = %e, "Failed to deliver response");
                    }
                }

                // Enqueue summary to system events
                let summary = Self::summarize_response(&response);
                let event_text = format!("Hook {}: {}", name, summary);
                system_events.enqueue(event_text).await;
            }
            Err(e) => {
                error!(run_id = %run_id, error = %e, "Agent run failed");

                // Enqueue error to system events
                let event_text = format!("Hook {} (error): {}", name, e);
                system_events.enqueue(event_text).await;
            }
        }

        // Request heartbeat if mode is Now
        if matches!(wake_mode, WakeMode::Now) {
            if let Err(e) = heartbeat_tx
                .send(HeartbeatRequest {
                    reason: format!("hook:{}", run_id),
                })
                .await
            {
                warn!(error = %e, "Failed to send heartbeat request");
            }
        }
    }

    /// Run an agent turn with the LLM
    async fn run_agent_turn(
        message: &str,
        session_key: &str,
        session_manager: Arc<SessionManager>,
        llm_client: Arc<dyn LlmClient>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Get or create session
        let _session = session_manager.get_or_create(session_key).await?;

        // Add user message to history
        session_manager
            .add_message(session_key, ConversationMessage::user(message))
            .await?;

        // Get messages from session
        let history = session_manager.get_messages(session_key).await?;

        // Build messages for LLM
        let messages: Vec<LlmMessage> = history
            .iter()
            .map(|m| match m.role {
                edith_session::MessageRole::User => LlmMessage::user(&m.content),
                edith_session::MessageRole::Assistant => LlmMessage::assistant(&m.content),
                edith_session::MessageRole::System => LlmMessage::user(&m.content), // System as user for now
            })
            .collect();

        // Build completion request
        let request = CompletionRequest::builder("claude-sonnet-4-20250514")
            .messages(messages)
            .max_tokens(4096)
            .build();

        // Call LLM
        let response = llm_client.complete(request).await?;

        // Extract text from response
        let response_text = response.text().unwrap_or_default();

        // Add assistant response to history
        session_manager
            .add_message(session_key, ConversationMessage::assistant(&response_text))
            .await?;

        Ok(response_text)
    }

    /// Deliver response to a channel
    async fn deliver_response(
        response: &str,
        channel: DeliveryChannel,
        to: Option<&str>,
        providers: Arc<RwLock<std::collections::HashMap<String, Arc<dyn MessagingProvider>>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let providers = providers.read().await;

        let provider_name = match channel {
            DeliveryChannel::WhatsApp => "whatsapp",
            DeliveryChannel::Telegram => "telegram",
            DeliveryChannel::Discord => "discord",
            DeliveryChannel::Signal => "signal",
            DeliveryChannel::IMessage => "imessage",
            DeliveryChannel::Last => {
                // TODO: Track last used channel
                "telegram"
            }
        };

        if let Some(provider) = providers.get(provider_name) {
            if let Some(recipient) = to {
                let reply = edith_providers::ReplyPayload {
                    text: Some(response.to_string()),
                    media_url: None,
                    media_urls: None,
                    reply_to_id: None,
                    parse_mode: None,
                };
                provider.send_reply(recipient, reply).await?;
            } else {
                warn!("No recipient specified for delivery");
            }
        } else {
            warn!(provider = %provider_name, "Provider not available for delivery");
        }

        Ok(())
    }

    /// Summarize a response for system events
    fn summarize_response(response: &str) -> String {
        let trimmed = response.trim();
        if trimmed.len() <= 100 {
            trimmed.to_string()
        } else {
            format!("{}...", &trimmed[..97])
        }
    }
}
