//! Edith error types

use thiserror::Error;

/// Main error type for Edith
#[derive(Debug, Error)]
pub enum EdithError {
    #[error("configuration error: {0}")]
    Config(#[from] edith_config::ConfigError),

    #[error("gateway error: {0}")]
    Gateway(#[from] edith_gateway::GatewayError),

    #[error("session error: {0}")]
    Session(#[from] edith_session::SessionError),

    #[error("provider error: {0}")]
    Provider(#[from] edith_providers::ProviderError),

    #[error("LLM error: {0}")]
    Llm(#[from] edith_llm::LlmError),

    #[error("pipeline error: {0}")]
    Pipeline(String),

    #[error("bot not started")]
    BotNotStarted,

    #[error("shutdown requested")]
    Shutdown,

    #[error("internal error: {0}")]
    Internal(String),
}

pub type EdithResult<T> = Result<T, EdithError>;
