//! Tool execution framework
//!
//! Provides a framework for defining, registering, and executing tools
//! for LLM function calling.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::error::{LlmError, LlmResult};
use crate::message::{
    CompletionRequest, CompletionResponse, ContentBlock, Message, MessageContent, StopReason, Tool,
};
use crate::LlmClient;

/// Result of executing a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution was successful
    pub success: bool,
    /// The result content (text)
    pub content: String,
    /// Optional structured data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ToolResult {
    /// Create a successful result
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: None,
        }
    }

    /// Create a successful result with structured data
    pub fn success_with_data(content: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: Some(data),
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            content: message.into(),
            data: None,
        }
    }
}

/// Trait for implementing a tool
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the input schema (JSON Schema)
    fn input_schema(&self) -> Value;

    /// Execute the tool with given input
    async fn execute(&self, input: Value) -> ToolResult;

    /// Convert to Tool definition for LLM
    fn to_tool_definition(&self) -> Tool {
        Tool {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

/// Registry of available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn ToolExecutor>) {
        let name = tool.name().to_string();
        debug!(tool = %name, "Registering tool");
        self.tools.insert(name, tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ToolExecutor>> {
        self.tools.get(name)
    }

    /// Get all registered tools
    pub fn all_tools(&self) -> Vec<&Arc<dyn ToolExecutor>> {
        self.tools.values().collect()
    }

    /// Get tool definitions for LLM request
    pub fn tool_definitions(&self) -> Vec<Tool> {
        self.tools.values().map(|t| t.to_tool_definition()).collect()
    }

    /// Check if a tool is registered
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

/// Configuration for the tool runner
#[derive(Debug, Clone)]
pub struct ToolRunnerConfig {
    /// Maximum number of tool call rounds
    pub max_rounds: usize,
    /// Whether to continue on tool errors
    pub continue_on_error: bool,
    /// Timeout per tool execution (ms)
    pub tool_timeout_ms: u64,
}

impl Default for ToolRunnerConfig {
    fn default() -> Self {
        Self {
            max_rounds: 10,
            continue_on_error: true,
            tool_timeout_ms: 30_000,
        }
    }
}

/// A pending tool call from the LLM
#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// Result of a tool run
#[derive(Debug)]
pub struct ToolRunResult {
    /// Final response from the LLM
    pub response: CompletionResponse,
    /// All messages in the conversation (including tool calls/results)
    pub messages: Vec<Message>,
    /// Tool calls that were executed
    pub tool_calls: Vec<(PendingToolCall, ToolResult)>,
    /// Number of rounds executed
    pub rounds: usize,
}

/// Runner that orchestrates tool calling loop
pub struct ToolRunner<C: LlmClient> {
    client: Arc<C>,
    registry: Arc<ToolRegistry>,
    config: ToolRunnerConfig,
}

impl<C: LlmClient> ToolRunner<C> {
    /// Create a new tool runner
    pub fn new(client: Arc<C>, registry: Arc<ToolRegistry>) -> Self {
        Self {
            client,
            registry,
            config: ToolRunnerConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(mut self, config: ToolRunnerConfig) -> Self {
        self.config = config;
        self
    }

    /// Run a completion with automatic tool execution
    pub async fn run(&self, mut request: CompletionRequest) -> LlmResult<ToolRunResult> {
        // Add tool definitions if not already present
        if request.tools.is_none() && !self.registry.is_empty() {
            request.tools = Some(self.registry.tool_definitions());
        }

        let mut messages = request.messages.clone();
        let mut all_tool_calls = Vec::new();
        let mut round = 0;

        loop {
            round += 1;
            if round > self.config.max_rounds {
                return Err(LlmError::Internal(format!(
                    "Exceeded maximum tool call rounds ({})",
                    self.config.max_rounds
                )));
            }

            info!(round = round, "Running LLM completion (tool loop)");

            // Update request with current messages
            let current_request = CompletionRequest {
                model: request.model.clone(),
                messages: messages.clone(),
                max_tokens: request.max_tokens,
                system: request.system.clone(),
                temperature: request.temperature,
                top_p: request.top_p,
                top_k: request.top_k,
                stop_sequences: request.stop_sequences.clone(),
                tools: request.tools.clone(),
                stream: None,
                metadata: request.metadata.clone(),
            };

            // Make the request
            let response = self.client.complete(current_request).await?;

            // Check if we need to execute tools
            if response.stop_reason != Some(StopReason::ToolUse) {
                // No more tools to call - done
                return Ok(ToolRunResult {
                    response,
                    messages,
                    tool_calls: all_tool_calls,
                    rounds: round,
                });
            }

            // Extract tool calls from response
            let tool_calls = self.extract_tool_calls(&response);
            if tool_calls.is_empty() {
                warn!("Stop reason was ToolUse but no tool calls found");
                return Ok(ToolRunResult {
                    response,
                    messages,
                    tool_calls: all_tool_calls,
                    rounds: round,
                });
            }

            // Add assistant message with tool calls
            messages.push(Message {
                role: crate::message::Role::Assistant,
                content: MessageContent::Blocks(response.content.clone()),
            });

            // Execute each tool
            let mut tool_results = Vec::new();
            for call in tool_calls {
                debug!(
                    tool = %call.name,
                    id = %call.id,
                    "Executing tool"
                );

                let result = self.execute_tool(&call).await;
                tool_results.push((call.clone(), result.clone()));

                // Create tool result block
                let result_block = ContentBlock::ToolResult {
                    tool_use_id: call.id.clone(),
                    content: result.content.clone(),
                    is_error: Some(!result.success),
                };

                // Add user message with tool result
                messages.push(Message {
                    role: crate::message::Role::User,
                    content: MessageContent::Blocks(vec![result_block]),
                });

                // Track for result
                all_tool_calls.push((call, result.clone()));

                // Check if we should stop on error
                if !result.success && !self.config.continue_on_error {
                    return Err(LlmError::Internal(format!(
                        "Tool execution failed: {}",
                        result.content
                    )));
                }
            }
        }
    }

    /// Extract tool calls from response
    fn extract_tool_calls(&self, response: &CompletionResponse) -> Vec<PendingToolCall> {
        response
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => Some(PendingToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    /// Execute a single tool
    async fn execute_tool(&self, call: &PendingToolCall) -> ToolResult {
        let tool = match self.registry.get(&call.name) {
            Some(t) => t,
            None => {
                error!(tool = %call.name, "Tool not found in registry");
                return ToolResult::error(format!("Unknown tool: {}", call.name));
            }
        };

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(self.config.tool_timeout_ms);
        match tokio::time::timeout(timeout, tool.execute(call.input.clone())).await {
            Ok(result) => result,
            Err(_) => {
                error!(tool = %call.name, "Tool execution timed out");
                ToolResult::error(format!(
                    "Tool '{}' timed out after {}ms",
                    call.name, self.config.tool_timeout_ms
                ))
            }
        }
    }
}

/// Builder for creating tools from closures
pub struct ToolBuilder {
    name: String,
    description: String,
    input_schema: Value,
}

impl ToolBuilder {
    /// Create a new tool builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set input schema
    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    /// Build with a sync executor function
    pub fn build_sync<F>(self, executor: F) -> FunctionTool<F>
    where
        F: Fn(Value) -> ToolResult + Send + Sync + 'static,
    {
        FunctionTool {
            name: self.name,
            description: self.description,
            input_schema: self.input_schema,
            executor,
        }
    }
}

/// A tool backed by a function
pub struct FunctionTool<F>
where
    F: Fn(Value) -> ToolResult + Send + Sync,
{
    name: String,
    description: String,
    input_schema: Value,
    executor: F,
}

#[async_trait]
impl<F> ToolExecutor for FunctionTool<F>
where
    F: Fn(Value) -> ToolResult + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult {
        (self.executor)(input)
    }
}

/// An async tool backed by an async function
pub struct AsyncFunctionTool<F, Fut>
where
    F: Fn(Value) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = ToolResult> + Send,
{
    name: String,
    description: String,
    input_schema: Value,
    executor: F,
}

impl<F, Fut> AsyncFunctionTool<F, Fut>
where
    F: Fn(Value) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ToolResult> + Send + 'static,
{
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        executor: F,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            executor,
        }
    }
}

#[async_trait]
impl<F, Fut> ToolExecutor for AsyncFunctionTool<F, Fut>
where
    F: Fn(Value) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ToolResult> + Send + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult {
        (self.executor)(input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success("done");
        assert!(success.success);
        assert_eq!(success.content, "done");

        let error = ToolResult::error("failed");
        assert!(!error.success);
        assert_eq!(error.content, "failed");
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());

        let tool = Arc::new(
            ToolBuilder::new("test_tool")
                .description("A test tool")
                .build_sync(|_| ToolResult::success("ok")),
        );

        registry.register(tool);
        assert_eq!(registry.len(), 1);
        assert!(registry.has_tool("test_tool"));
        assert!(!registry.has_tool("unknown"));
    }

    #[test]
    fn test_tool_definitions() {
        let mut registry = ToolRegistry::new();

        let tool = Arc::new(
            ToolBuilder::new("calculator")
                .description("Perform math calculations")
                .input_schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "The math expression"
                        }
                    },
                    "required": ["expression"]
                }))
                .build_sync(|_| ToolResult::success("42")),
        );

        registry.register(tool);

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "calculator");
        assert_eq!(defs[0].description, "Perform math calculations");
    }

    #[tokio::test]
    async fn test_function_tool_execute() {
        let tool = ToolBuilder::new("echo")
            .description("Echo input")
            .build_sync(|input| {
                let text = input["text"].as_str().unwrap_or("empty");
                ToolResult::success(format!("Echo: {}", text))
            });

        let result = tool
            .execute(serde_json::json!({"text": "hello"}))
            .await;
        assert!(result.success);
        assert_eq!(result.content, "Echo: hello");
    }
}
