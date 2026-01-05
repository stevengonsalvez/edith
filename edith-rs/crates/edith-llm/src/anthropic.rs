//! Anthropic Claude API client

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::debug;

use crate::client::{LlmClient, RateLimiter, RetryConfig};
use crate::error::{LlmError, LlmResult};
use crate::message::{CompletionRequest, CompletionResponse};
use crate::streaming::{SseParser, StreamAccumulator, StreamDelta};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Anthropic Claude API client
pub struct AnthropicClient {
    api_key: String,
    base_url: String,
    http_client: Client,
    rate_limiter: Option<RateLimiter>,
    retry_config: RetryConfig,
}

impl AnthropicClient {
    /// Create a new Anthropic client
    pub fn new(api_key: impl Into<String>) -> LlmResult<Self> {
        let api_key = api_key.into();
        if api_key.is_empty() {
            return Err(LlmError::MissingApiKey);
        }

        let http_client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| LlmError::Internal(format!("failed to create HTTP client: {}", e)))?;

        Ok(Self {
            api_key,
            base_url: ANTHROPIC_API_URL.to_string(),
            http_client,
            rate_limiter: None,
            retry_config: RetryConfig::default(),
        })
    }

    /// Create client with custom base URL (for proxies)
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set rate limiter
    pub fn with_rate_limiter(mut self, requests_per_minute: u32, tokens_per_minute: u64) -> Self {
        self.rate_limiter = Some(RateLimiter::new(requests_per_minute, tokens_per_minute));
        self
    }

    /// Set retry config
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Parse error response
    fn parse_error(&self, status: StatusCode, body: &str) -> LlmError {
        #[derive(serde::Deserialize)]
        struct ErrorResponse {
            error: ErrorDetail,
        }

        #[derive(serde::Deserialize)]
        struct ErrorDetail {
            #[serde(rename = "type")]
            error_type: String,
            message: String,
        }

        if let Ok(error_resp) = serde_json::from_str::<ErrorResponse>(body) {
            match error_resp.error.error_type.as_str() {
                "authentication_error" => LlmError::AuthFailed(error_resp.error.message),
                "rate_limit_error" => LlmError::RateLimited {
                    retry_after_secs: 60,
                },
                "overloaded_error" => LlmError::Overloaded(error_resp.error.message),
                "invalid_request_error" => {
                    if error_resp.error.message.contains("context length") {
                        // Try to parse token counts from message
                        LlmError::ContextLengthExceeded { used: 0, max: 0 }
                    } else {
                        LlmError::InvalidRequest(error_resp.error.message)
                    }
                }
                "not_found_error" => LlmError::ModelNotFound(error_resp.error.message),
                _ => LlmError::Http {
                    status: status.as_u16(),
                    message: error_resp.error.message,
                },
            }
        } else {
            LlmError::Http {
                status: status.as_u16(),
                message: body.to_string(),
            }
        }
    }

    /// Send request and handle response
    async fn send_request(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
        // Apply rate limiting if configured
        if let Some(ref limiter) = self.rate_limiter {
            let estimated = self.estimate_tokens(request);
            limiter.acquire(estimated).await?;
        }

        debug!(model = %request.model, "Sending completion request");

        let response = self
            .http_client
            .post(&self.base_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmError::Timeout(DEFAULT_TIMEOUT.as_secs())
                } else if e.is_connect() {
                    LlmError::Connection(e.to_string())
                } else {
                    LlmError::Request(e)
                }
            })?;

        let status = response.status();
        let body = response.text().await.map_err(LlmError::Request)?;

        if !status.is_success() {
            return Err(self.parse_error(status, &body));
        }

        let completion: CompletionResponse = serde_json::from_str(&body)?;

        // Report actual usage
        if let Some(ref limiter) = self.rate_limiter {
            let actual = completion.usage.input_tokens + completion.usage.output_tokens;
            let estimated = self.estimate_tokens(request);
            limiter.report_usage(actual, estimated);
        }

        debug!(
            model = %completion.model,
            input_tokens = completion.usage.input_tokens,
            output_tokens = completion.usage.output_tokens,
            "Completion received"
        );

        Ok(completion)
    }

    /// Estimate token count for rate limiting
    fn estimate_tokens(&self, request: &CompletionRequest) -> u64 {
        let mut estimate = 0u64;

        // System prompt
        if let Some(ref system) = request.system {
            estimate += self.count_tokens(system);
        }

        // Messages
        for msg in &request.messages {
            estimate += self.count_tokens(&msg.all_text());
        }

        // Add overhead for tools
        if let Some(ref tools) = request.tools {
            estimate += (tools.len() * 100) as u64;
        }

        // Add expected output
        estimate += request.max_tokens as u64;

        estimate
    }

    /// Send streaming request
    async fn send_streaming_request(
        &self,
        request: CompletionRequest,
    ) -> LlmResult<mpsc::Receiver<LlmResult<StreamDelta>>> {
        // Apply rate limiting
        if let Some(ref limiter) = self.rate_limiter {
            let estimated = self.estimate_tokens(&request);
            limiter.acquire(estimated).await?;
        }

        // Ensure stream is enabled
        let mut request = request;
        request.stream = Some(true);

        debug!(model = %request.model, "Starting streaming completion");

        let response = self
            .http_client
            .post(&self.base_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmError::Timeout(DEFAULT_TIMEOUT.as_secs())
                } else if e.is_connect() {
                    LlmError::Connection(e.to_string())
                } else {
                    LlmError::Request(e)
                }
            })?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.map_err(LlmError::Request)?;
            return Err(self.parse_error(status, &body));
        }

        // Create channel for streaming events
        let (tx, rx) = mpsc::channel(100);

        // Spawn task to process stream
        let stream = response.bytes_stream();
        tokio::spawn(async move {
            let mut parser = SseParser::new();
            let mut accumulator = StreamAccumulator::new();

            let mut stream = std::pin::pin!(stream);

            while let Some(result) = stream.next().await {
                match result {
                    Ok(bytes) => {
                        for event_result in parser.feed(&bytes) {
                            match event_result {
                                Ok(event) => {
                                    match accumulator.process(event) {
                                        Ok(Some(delta)) => {
                                            if tx.send(Ok(delta)).await.is_err() {
                                                return; // Receiver dropped
                                            }
                                        }
                                        Ok(None) => {}
                                        Err(e) => {
                                            let _ = tx.send(Err(e)).await;
                                            return;
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(LlmError::StreamError(e.to_string()))).await;
                        return;
                    }
                }
            }

            // Send final done signal
            let _ = tx.send(Ok(StreamDelta::Done)).await;
        });

        Ok(rx)
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse> {
        crate::client::with_retry(&self.retry_config, || self.send_request(&request)).await
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> LlmResult<mpsc::Receiver<LlmResult<StreamDelta>>> {
        self.send_streaming_request(request).await
    }

    fn count_tokens(&self, text: &str) -> u64 {
        // Claude tokenization is roughly 4 chars per token
        // This is a rough estimate - for accurate counts, use the tokenizer API
        (text.len() as f64 / 4.0).ceil() as u64
    }
}

/// Create an Anthropic client from environment
pub fn from_env() -> LlmResult<AnthropicClient> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| LlmError::MissingApiKey)?;
    AnthropicClient::new(api_key)
}

/// Builder for Anthropic client
pub struct AnthropicClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    requests_per_minute: Option<u32>,
    tokens_per_minute: Option<u64>,
    retry_config: RetryConfig,
    timeout: Duration,
}

impl AnthropicClientBuilder {
    pub fn new() -> Self {
        Self {
            api_key: None,
            base_url: None,
            requests_per_minute: None,
            tokens_per_minute: None,
            retry_config: RetryConfig::default(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn rate_limit(mut self, requests_per_minute: u32, tokens_per_minute: u64) -> Self {
        self.requests_per_minute = Some(requests_per_minute);
        self.tokens_per_minute = Some(tokens_per_minute);
        self
    }

    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> LlmResult<AnthropicClient> {
        let api_key = self
            .api_key
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or(LlmError::MissingApiKey)?;

        let http_client = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| LlmError::Internal(format!("failed to create HTTP client: {}", e)))?;

        let mut client = AnthropicClient {
            api_key,
            base_url: self.base_url.unwrap_or_else(|| ANTHROPIC_API_URL.to_string()),
            http_client,
            rate_limiter: None,
            retry_config: self.retry_config,
        };

        if let (Some(rpm), Some(tpm)) = (self.requests_per_minute, self.tokens_per_minute) {
            client.rate_limiter = Some(RateLimiter::new(rpm, tpm));
        }

        Ok(client)
    }
}

impl Default for AnthropicClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LlmError;

    #[test]
    fn test_client_creation() {
        let client = AnthropicClient::new("test-key");
        assert!(client.is_ok());

        let client = AnthropicClient::new("");
        assert!(matches!(client, Err(LlmError::MissingApiKey)));
    }

    #[test]
    fn test_builder() {
        let client = AnthropicClientBuilder::new()
            .api_key("test-key")
            .base_url("https://custom.proxy.com")
            .rate_limit(60, 100000)
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.base_url, "https://custom.proxy.com");
    }

    #[test]
    fn test_token_count() {
        let client = AnthropicClient::new("test-key").unwrap();
        let count = client.count_tokens("Hello, world!");
        assert!(count > 0);
    }
}
