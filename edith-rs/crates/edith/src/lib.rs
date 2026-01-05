//! Edith - Personal AI Assistant Gateway
//!
//! Edith is a self-hosted gateway that connects multiple messaging platforms
//! (Telegram, WhatsApp, Discord, Signal, iMessage) to LLM APIs like Claude.
//!
//! # Architecture
//!
//! The system consists of several crates:
//!
//! - `edith-config`: Configuration management and validation
//! - `edith-gateway`: HTTP/WebSocket server (Axum-based)
//! - `edith-session`: Session state management
//! - `edith-providers`: Messaging platform integrations
//! - `edith-llm`: LLM API clients (Claude, etc.)
//!
//! # Example
//!
//! ```no_run
//! use edith::{Edith, EdithConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Load configuration
//!     let config = EdithConfig::from_file("config.json5")?;
//!
//!     // Create and run the gateway
//!     let mut gateway = Edith::new(config)?;
//!     gateway.run().await?;
//!
//!     Ok(())
//! }
//! ```

pub mod app;
pub mod bot;
pub mod error;
pub mod event_processor;
pub mod pipeline;

// Re-export from sub-crates
pub use edith_config::{EdithConfig, ConfigError};
pub use edith_gateway::{GatewayServer, GatewayState};
pub use edith_llm::{AnthropicClient, CompletionRequest, LlmClient, LlmError, Message as LlmMessage};
pub use edith_providers::{
    InboundMessage, MessagingProvider, OutboundMessage, ProviderError, TelegramProvider,
};
pub use edith_session::{Session, SessionManager, SessionStore};

// Re-export main types
pub use app::Edith;
pub use bot::Bot;
pub use error::{EdithError, EdithResult};
pub use event_processor::{EventProcessor, SystemEventQueue, HeartbeatRequest};
pub use pipeline::{MessagePipeline, PipelineContext};
