//! Message processing pipeline
//!
//! The pipeline handles the flow of messages from providers through
//! preprocessing, LLM interaction, and response delivery.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error, warn};

use edith_config::EdithConfig;
use edith_llm::{
    CompletionRequest, CompletionResponse, LlmClient, Message,
};
use edith_providers::{InboundMessage, MessagingProvider, ReplyPayload};
use edith_session::{ConversationMessage, MessageRole, Session, SessionManager};

use crate::error::{EdithError, EdithResult};

/// Context for pipeline processing
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// The inbound message being processed
    pub message: InboundMessage,
    /// Session for this conversation
    pub session: Session,
    /// Provider name that received the message
    pub provider: String,
    /// Whether this is a group conversation
    pub is_group: bool,
    /// Bot was mentioned (for groups)
    pub was_mentioned: bool,
}

/// Pipeline stage trait
#[async_trait]
pub trait PipelineStage: Send + Sync {
    /// Stage name for logging
    fn name(&self) -> &'static str;

    /// Process the context, returning modified context or error
    async fn process(&self, ctx: PipelineContext) -> EdithResult<PipelineContext>;
}

/// Message processing pipeline
pub struct MessagePipeline {
    config: EdithConfig,
    session_manager: Arc<SessionManager>,
    llm_client: Arc<dyn LlmClient>,
    stages: Vec<Box<dyn PipelineStage>>,
}

impl MessagePipeline {
    /// Create a new pipeline
    pub fn new(
        config: EdithConfig,
        session_manager: Arc<SessionManager>,
        llm_client: Arc<dyn LlmClient>,
    ) -> Self {
        Self {
            config,
            session_manager,
            llm_client,
            stages: Vec::new(),
        }
    }

    /// Add a pipeline stage
    pub fn add_stage(mut self, stage: Box<dyn PipelineStage>) -> Self {
        self.stages.push(stage);
        self
    }

    /// Process an inbound message
    pub async fn process(
        &self,
        message: InboundMessage,
        provider: &dyn MessagingProvider,
    ) -> EdithResult<Option<String>> {
        let provider_name = provider.name().to_string();

        debug!(
            provider = %provider_name,
            from = %message.from,
            body_len = message.body.len(),
            "Processing inbound message"
        );

        // Resolve session
        let session_key = self.resolve_session_key(&message);
        let session = self
            .session_manager
            .get_or_create(&session_key)
            .await
            .map_err(EdithError::Session)?;

        // Build context
        let is_group = matches!(message.chat_type, edith_providers::ChatType::Group);
        let was_mentioned = message.was_mentioned.unwrap_or(false);

        let mut ctx = PipelineContext {
            message,
            session,
            provider: provider_name.clone(),
            is_group,
            was_mentioned,
        };

        // Run through stages
        for stage in &self.stages {
            debug!(stage = stage.name(), "Running pipeline stage");
            ctx = stage.process(ctx).await?;
        }

        // Send typing indicator
        if let Err(e) = provider.send_typing(&ctx.message.chat_id).await {
            warn!(error = %e, "Failed to send typing indicator");
        }

        // Build LLM request
        let response = self.call_llm(&ctx).await?;

        // Extract text response
        let text = response.text();

        // Send response back
        if let Some(ref reply_text) = text {
            let reply = ReplyPayload {
                text: Some(reply_text.clone()),
                reply_to_id: ctx.message.id.clone(),
                media_url: None,
                media_urls: None,
                parse_mode: Some("Markdown".to_string()),
            };

            match provider.send_reply(&ctx.message.chat_id, reply).await {
                Ok(msg_id) => {
                    debug!(msg_id = %msg_id, "Reply sent successfully");
                }
                Err(e) => {
                    error!(error = %e, "Failed to send reply");
                    return Err(EdithError::Provider(e));
                }
            }
        }

        // Update session with new messages
        self.session_manager
            .add_message(
                &ctx.session.key,
                ConversationMessage::user(&ctx.message.body),
            )
            .await
            .map_err(EdithError::Session)?;

        if let Some(ref reply_text) = text {
            self.session_manager
                .add_message(&ctx.session.key, ConversationMessage::assistant(reply_text))
                .await
                .map_err(EdithError::Session)?;
        }

        Ok(text)
    }

    /// Resolve session key from message
    fn resolve_session_key(&self, message: &InboundMessage) -> String {
        // Use conversation_id if available, otherwise construct from from/to
        if !message.conversation_id.is_empty() {
            format!("{}:{}", message.surface, message.conversation_id)
        } else {
            format!("{}:{}", message.surface, message.from)
        }
    }

    /// Call the LLM with the context
    async fn call_llm(&self, ctx: &PipelineContext) -> EdithResult<CompletionResponse> {
        // Get conversation history and convert to LLM messages
        let history = self
            .session_manager
            .get_messages(&ctx.session.key)
            .await
            .map_err(EdithError::Session)?;

        // Build system prompt
        let system = self.build_system_prompt(ctx);

        // Convert ConversationMessage to Message
        let mut messages: Vec<Message> = history
            .into_iter()
            .map(|m| match m.role {
                MessageRole::User => Message::user(m.content),
                MessageRole::Assistant => Message::assistant(m.content),
                MessageRole::System => Message::user(m.content), // System messages go as user context
            })
            .collect();
        messages.push(Message::user(&ctx.message.body));

        // Get model from config
        let model = self
            .config
            .llm
            .as_ref()
            .and_then(|l| l.model.clone())
            .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

        let max_tokens = self
            .config
            .llm
            .as_ref()
            .and_then(|l| l.max_tokens)
            .unwrap_or(4096);

        let request = CompletionRequest::builder(&model)
            .messages(messages)
            .max_tokens(max_tokens)
            .system(system)
            .build();

        self.llm_client
            .complete(request)
            .await
            .map_err(EdithError::Llm)
    }

    /// Build system prompt from config and context
    fn build_system_prompt(&self, ctx: &PipelineContext) -> String {
        let mut prompt = self
            .config
            .identity
            .as_ref()
            .and_then(|i| i.system_prompt.clone())
            .unwrap_or_else(|| {
                "You are a helpful AI assistant. Be concise and helpful.".to_string()
            });

        // Add context about the conversation
        if ctx.is_group {
            prompt.push_str(&format!(
                "\n\nYou are in a group chat. The user's name is: {}",
                ctx.message.sender_name.as_deref().unwrap_or("Unknown")
            ));
        }

        // Add identity info if available
        if let Some(ref identity) = self.config.identity {
            if let Some(ref name) = identity.name {
                prompt.push_str(&format!("\n\nYour name is: {}", name));
            }
        }

        prompt
    }
}

/// Default preprocessing stage
pub struct PreprocessStage;

#[async_trait]
impl PipelineStage for PreprocessStage {
    fn name(&self) -> &'static str {
        "preprocess"
    }

    async fn process(&self, mut ctx: PipelineContext) -> EdithResult<PipelineContext> {
        // Trim whitespace
        ctx.message.body = ctx.message.body.trim().to_string();

        // Remove bot mentions from message text
        if ctx.was_mentioned {
            // Remove @botname patterns
            let body = ctx.message.body.clone();
            ctx.message.body = body
                .split_whitespace()
                .filter(|word| !word.starts_with('@'))
                .collect::<Vec<_>>()
                .join(" ");
        }

        Ok(ctx)
    }
}

/// Rate limiting stage
pub struct RateLimitStage {
    #[allow(dead_code)]
    requests_per_minute: u32,
}

impl RateLimitStage {
    pub fn new(requests_per_minute: u32) -> Self {
        Self { requests_per_minute }
    }
}

#[async_trait]
impl PipelineStage for RateLimitStage {
    fn name(&self) -> &'static str {
        "rate_limit"
    }

    async fn process(&self, ctx: PipelineContext) -> EdithResult<PipelineContext> {
        // TODO: Implement actual rate limiting
        // For now, just pass through
        Ok(ctx)
    }
}

/// Content filtering stage
pub struct ContentFilterStage;

#[async_trait]
impl PipelineStage for ContentFilterStage {
    fn name(&self) -> &'static str {
        "content_filter"
    }

    async fn process(&self, ctx: PipelineContext) -> EdithResult<PipelineContext> {
        // Check for blocked patterns
        let blocked_patterns = ["ignore previous", "disregard", "new instructions"];

        for pattern in blocked_patterns {
            if ctx.message.body.to_lowercase().contains(pattern) {
                warn!(pattern = pattern, "Blocked message with injection attempt");
                return Err(EdithError::Pipeline(
                    "Message contains blocked content".to_string(),
                ));
            }
        }

        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_preprocess_stage() {
        let stage = PreprocessStage;

        let message = InboundMessage {
            id: Some("1".to_string()),
            from: "user".to_string(),
            conversation_id: "conv".to_string(),
            to: "bot".to_string(),
            body: "  @bot hello world  ".to_string(),
            sender_name: None,
            timestamp: None,
            chat_type: edith_providers::ChatType::Direct,
            chat_id: "chat".to_string(),
            sender_jid: None,
            sender_e164: None,
            reply_context: None,
            group_subject: None,
            group_participants: None,
            mentioned_ids: None,
            was_mentioned: Some(true),
            media: None,
            surface: "test".to_string(),
            raw: None,
        };

        let ctx = PipelineContext {
            message,
            session: Session::default(),
            provider: "test".to_string(),
            is_group: false,
            was_mentioned: true,
        };

        let result = stage.process(ctx).await.unwrap();
        assert_eq!(result.message.body, "hello world");
    }
}
