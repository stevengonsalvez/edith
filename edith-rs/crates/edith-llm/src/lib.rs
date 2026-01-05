//! Edith LLM Client
//!
//! This crate provides clients for various LLM APIs:
//! - Anthropic Claude (Messages API)
//! - OpenAI (Chat Completions) - future
//! - Custom/proxy providers
//!
//! # Example
//!
//! ```no_run
//! use edith_llm::{AnthropicClient, LlmClient, Message, CompletionRequest};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = AnthropicClient::new("your-api-key")?;
//!
//!     let request = CompletionRequest::builder("claude-3-5-sonnet-20241022")
//!         .add_message(Message::user("Hello!"))
//!         .max_tokens(1024)
//!         .build();
//!
//!     let response = client.complete(request).await?;
//!     println!("{}", response.text().unwrap_or_default());
//!
//!     Ok(())
//! }
//! ```

pub mod anthropic;
pub mod client;
pub mod error;
pub mod message;
pub mod streaming;
pub mod tools;

// Re-export main types
pub use anthropic::{AnthropicClient, AnthropicClientBuilder};
pub use client::{get_model_info, LlmClient, ModelInfo, RateLimiter, RetryConfig};
pub use error::{LlmError, LlmResult};
pub use message::{
    CompletionRequest, CompletionRequestBuilder, CompletionResponse, ContentBlock, ImageSource,
    Message, MessageContent, RequestMetadata, Role, StopReason, Tool, Usage,
};
pub use streaming::{
    CompletionStream, ContentDelta, SseParser, StreamAccumulator, StreamDelta, StreamEvent,
};
pub use tools::{
    AsyncFunctionTool, FunctionTool, PendingToolCall, ToolBuilder, ToolExecutor, ToolRegistry,
    ToolResult, ToolRunResult, ToolRunner, ToolRunnerConfig,
};
