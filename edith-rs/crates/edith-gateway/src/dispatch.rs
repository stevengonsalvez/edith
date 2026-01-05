//! Event dispatch system for hooks and agent runs

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Wake event for triggering the heartbeat/agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeEvent {
    /// Text to enqueue as a system event
    pub text: String,
    /// When to wake
    pub mode: WakeMode,
    /// Source of the wake event
    pub source: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WakeMode {
    /// Execute immediately
    #[default]
    Now,
    /// Wait for next heartbeat
    NextHeartbeat,
}

/// Agent run request from hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunRequest {
    /// Unique run ID
    pub run_id: String,
    /// The message to process
    pub message: String,
    /// Name of the agent/source
    pub name: String,
    /// When to wake
    pub wake_mode: WakeMode,
    /// Session key for context
    pub session_key: String,
    /// Whether to deliver response to a channel
    pub deliver: bool,
    /// Target channel for delivery
    pub channel: DeliveryChannel,
    /// Specific recipient (phone/chat ID)
    pub to: Option<String>,
    /// Thinking level for the LLM
    pub thinking: Option<String>,
    /// Timeout in seconds
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryChannel {
    /// Last used channel
    #[default]
    Last,
    WhatsApp,
    Telegram,
    Discord,
    Signal,
    IMessage,
}

/// Event dispatcher for the gateway
#[derive(Clone)]
pub struct EventDispatcher {
    /// Channel for wake events
    wake_tx: mpsc::Sender<WakeEvent>,
    /// Channel for agent run requests
    agent_tx: mpsc::Sender<AgentRunRequest>,
}

/// Receivers for event channels
pub struct EventReceivers {
    /// Wake event receiver
    pub wake_rx: mpsc::Receiver<WakeEvent>,
    /// Agent run receiver
    pub agent_rx: mpsc::Receiver<AgentRunRequest>,
}

impl EventDispatcher {
    /// Create a new event dispatcher with its receivers
    pub fn new(buffer_size: usize) -> (Self, EventReceivers) {
        let (wake_tx, wake_rx) = mpsc::channel(buffer_size);
        let (agent_tx, agent_rx) = mpsc::channel(buffer_size);

        let dispatcher = Self { wake_tx, agent_tx };
        let receivers = EventReceivers { wake_rx, agent_rx };

        (dispatcher, receivers)
    }

    /// Dispatch a wake event
    pub async fn dispatch_wake(&self, event: WakeEvent) -> Result<(), DispatchError> {
        self.wake_tx
            .send(event)
            .await
            .map_err(|_| DispatchError::ChannelClosed)
    }

    /// Dispatch an agent run request
    pub async fn dispatch_agent_run(&self, request: AgentRunRequest) -> Result<(), DispatchError> {
        self.agent_tx
            .send(request)
            .await
            .map_err(|_| DispatchError::ChannelClosed)
    }

    /// Create a wake event with defaults
    pub fn create_wake_event(text: String, mode: WakeMode) -> WakeEvent {
        WakeEvent {
            text,
            mode,
            source: "hook".to_string(),
        }
    }

    /// Create an agent run request with a new run ID
    pub fn create_agent_run_request(
        message: String,
        name: String,
        wake_mode: WakeMode,
        session_key: String,
        deliver: bool,
        channel: DeliveryChannel,
        to: Option<String>,
        thinking: Option<String>,
        timeout_seconds: Option<u64>,
    ) -> AgentRunRequest {
        AgentRunRequest {
            run_id: uuid::Uuid::new_v4().to_string(),
            message,
            name,
            wake_mode,
            session_key,
            deliver,
            channel,
            to,
            thinking,
            timeout_seconds,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("dispatch channel closed")]
    ChannelClosed,
}
