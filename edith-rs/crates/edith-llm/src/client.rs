//! Generic LLM client trait and utilities

use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::error::LlmResult;
use crate::message::{CompletionRequest, CompletionResponse};
use crate::streaming::StreamDelta;

/// LLM client trait
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Get client name
    fn name(&self) -> &'static str;

    /// Check if client is configured
    fn is_configured(&self) -> bool;

    /// Send a completion request
    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse>;

    /// Send a streaming completion request
    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> LlmResult<mpsc::Receiver<LlmResult<StreamDelta>>>;

    /// Count tokens in a string (approximate)
    fn count_tokens(&self, text: &str) -> u64 {
        // Rough approximation: ~4 chars per token
        (text.len() as f64 / 4.0).ceil() as u64
    }
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2.0,
        }
    }
}

/// Execute with retry logic
pub async fn with_retry<F, Fut, T>(config: &RetryConfig, mut operation: F) -> LlmResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = LlmResult<T>>,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !e.is_retryable() || attempt >= config.max_retries {
                    return Err(e);
                }

                // Use retry_after if available
                let wait = e.retry_after().map(Duration::from_secs).unwrap_or(delay);

                warn!(
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    wait_secs = wait.as_secs(),
                    error = %e,
                    "Retrying after error"
                );

                tokio::time::sleep(wait).await;

                attempt += 1;
                delay = std::cmp::min(
                    Duration::from_secs_f64(delay.as_secs_f64() * config.backoff_factor),
                    config.max_delay,
                );
            }
        }
    }
}

/// Rate limiter for API calls
#[derive(Debug)]
pub struct RateLimiter {
    requests_per_minute: u32,
    tokens_per_minute: u64,
    last_request: std::sync::Mutex<std::time::Instant>,
    tokens_used: std::sync::atomic::AtomicU64,
    requests_made: std::sync::atomic::AtomicU32,
    window_start: std::sync::Mutex<std::time::Instant>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32, tokens_per_minute: u64) -> Self {
        Self {
            requests_per_minute,
            tokens_per_minute,
            last_request: std::sync::Mutex::new(std::time::Instant::now()),
            tokens_used: std::sync::atomic::AtomicU64::new(0),
            requests_made: std::sync::atomic::AtomicU32::new(0),
            window_start: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }

    /// Wait for rate limit if needed
    pub async fn acquire(&self, estimated_tokens: u64) -> LlmResult<()> {
        use std::sync::atomic::Ordering;

        // Check if we need to reset the window
        {
            let mut window_start = self.window_start.lock().unwrap();
            if window_start.elapsed() >= Duration::from_secs(60) {
                *window_start = std::time::Instant::now();
                self.tokens_used.store(0, Ordering::SeqCst);
                self.requests_made.store(0, Ordering::SeqCst);
            }
        }

        // Check request limit
        let requests = self.requests_made.load(Ordering::SeqCst);
        if requests >= self.requests_per_minute {
            let remaining = {
                let window_start = self.window_start.lock().unwrap();
                Duration::from_secs(60).saturating_sub(window_start.elapsed())
            };
            if !remaining.is_zero() {
                debug!(wait_secs = remaining.as_secs(), "Rate limit: waiting for request slot");
                tokio::time::sleep(remaining).await;
            }
        }

        // Check token limit
        let tokens = self.tokens_used.load(Ordering::SeqCst);
        if tokens + estimated_tokens > self.tokens_per_minute {
            let remaining = {
                let window_start = self.window_start.lock().unwrap();
                Duration::from_secs(60).saturating_sub(window_start.elapsed())
            };
            if !remaining.is_zero() {
                debug!(wait_secs = remaining.as_secs(), "Rate limit: waiting for token budget");
                tokio::time::sleep(remaining).await;
            }
        }

        // Update counters
        self.requests_made.fetch_add(1, Ordering::SeqCst);
        self.tokens_used.fetch_add(estimated_tokens, Ordering::SeqCst);

        // Update last request time
        *self.last_request.lock().unwrap() = std::time::Instant::now();

        Ok(())
    }

    /// Report actual token usage
    pub fn report_usage(&self, actual_tokens: u64, estimated_tokens: u64) {
        use std::sync::atomic::Ordering;

        // Adjust for difference between estimate and actual
        if actual_tokens > estimated_tokens {
            self.tokens_used
                .fetch_add(actual_tokens - estimated_tokens, Ordering::SeqCst);
        }
    }
}

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub max_tokens: u32,
    pub context_window: u32,
    pub supports_vision: bool,
    pub supports_tools: bool,
}

impl ModelInfo {
    pub fn claude_3_opus() -> Self {
        Self {
            id: "claude-3-opus-20240229".to_string(),
            name: "Claude 3 Opus".to_string(),
            max_tokens: 4096,
            context_window: 200_000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    pub fn claude_3_sonnet() -> Self {
        Self {
            id: "claude-3-sonnet-20240229".to_string(),
            name: "Claude 3 Sonnet".to_string(),
            max_tokens: 4096,
            context_window: 200_000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    pub fn claude_3_haiku() -> Self {
        Self {
            id: "claude-3-haiku-20240307".to_string(),
            name: "Claude 3 Haiku".to_string(),
            max_tokens: 4096,
            context_window: 200_000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    pub fn claude_3_5_sonnet() -> Self {
        Self {
            id: "claude-3-5-sonnet-20241022".to_string(),
            name: "Claude 3.5 Sonnet".to_string(),
            max_tokens: 8192,
            context_window: 200_000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    pub fn claude_3_5_haiku() -> Self {
        Self {
            id: "claude-3-5-haiku-20241022".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
            max_tokens: 8192,
            context_window: 200_000,
            supports_vision: true,
            supports_tools: true,
        }
    }
}

/// Get model info by ID
pub fn get_model_info(model_id: &str) -> Option<ModelInfo> {
    match model_id {
        "claude-3-opus-20240229" => Some(ModelInfo::claude_3_opus()),
        "claude-3-sonnet-20240229" => Some(ModelInfo::claude_3_sonnet()),
        "claude-3-haiku-20240307" => Some(ModelInfo::claude_3_haiku()),
        "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet-latest" => {
            Some(ModelInfo::claude_3_5_sonnet())
        }
        "claude-3-5-haiku-20241022" | "claude-3-5-haiku-latest" => {
            Some(ModelInfo::claude_3_5_haiku())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LlmError;

    #[test]
    fn test_model_info() {
        let info = get_model_info("claude-3-5-sonnet-20241022").unwrap();
        assert_eq!(info.name, "Claude 3.5 Sonnet");
        assert_eq!(info.max_tokens, 8192);
        assert!(info.supports_vision);
    }

    #[tokio::test]
    async fn test_retry_success() {
        let config = RetryConfig::default();
        let result: LlmResult<i32> = with_retry(&config, || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_non_retryable() {
        let config = RetryConfig::default();
        let result: LlmResult<i32> = with_retry(&config, || async {
            Err(LlmError::AuthFailed("invalid key".to_string()))
        })
        .await;
        assert!(result.is_err());
    }
}
