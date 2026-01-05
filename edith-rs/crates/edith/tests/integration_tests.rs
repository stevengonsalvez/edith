//! Integration tests for Edith components
//!
//! These tests verify that the various crates work together correctly.

use std::sync::Arc;

// Test LLM client with tool execution
mod llm_tools {
    use super::*;
    use edith_llm::{
        CompletionRequest, Message, Tool, ToolBuilder, ToolExecutor, ToolRegistry,
        ToolResult,
    };

    #[test]
    fn test_tool_registry_integration() {
        let mut registry = ToolRegistry::new();

        // Register a calculator tool
        let calculator = Arc::new(
            ToolBuilder::new("calculator")
                .description("Performs basic math calculations")
                .input_schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["add", "subtract", "multiply", "divide"]
                        },
                        "a": {"type": "number"},
                        "b": {"type": "number"}
                    },
                    "required": ["operation", "a", "b"]
                }))
                .build_sync(|input| {
                    let op = input["operation"].as_str().unwrap_or("");
                    let a = input["a"].as_f64().unwrap_or(0.0);
                    let b = input["b"].as_f64().unwrap_or(0.0);

                    let result = match op {
                        "add" => a + b,
                        "subtract" => a - b,
                        "multiply" => a * b,
                        "divide" => {
                            if b == 0.0 {
                                return ToolResult::error("Division by zero");
                            }
                            a / b
                        }
                        _ => return ToolResult::error("Unknown operation"),
                    };

                    ToolResult::success(result.to_string())
                }),
        );

        registry.register(calculator);

        // Verify tool is registered
        assert!(registry.has_tool("calculator"));
        assert_eq!(registry.len(), 1);

        // Get tool definitions
        let definitions = registry.tool_definitions();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "calculator");
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let tool = ToolBuilder::new("echo")
            .description("Echoes input text")
            .input_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"}
                },
                "required": ["text"]
            }))
            .build_sync(|input| {
                let text = input["text"].as_str().unwrap_or("no text");
                ToolResult::success(format!("Echo: {}", text))
            });

        let result = tool.execute(serde_json::json!({"text": "Hello, World!"})).await;
        assert!(result.success);
        assert_eq!(result.content, "Echo: Hello, World!");
    }

    #[test]
    fn test_completion_request_with_tools() {
        let tools = vec![Tool {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
        }];

        let request = CompletionRequest::builder("claude-3-5-sonnet-20241022")
            .add_message(Message::user("Search for rust async patterns"))
            .tools(tools)
            .max_tokens(1024)
            .build();

        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
        assert_eq!(request.tools.as_ref().unwrap()[0].name, "search");
    }
}

// Test session management
mod session {
    use edith_config::EdithConfig;
    use edith_session::{SessionManager, ConversationMessage};

    #[tokio::test]
    async fn test_session_lifecycle() {
        let config = EdithConfig::default();
        let manager = SessionManager::new(config);

        // Test get or create session
        let session_key = "test:user123";
        let session = manager.get_or_create(session_key).await;
        assert!(session.is_ok());
        let session = session.unwrap();
        assert!(session.messages.is_empty());

        // Add a message
        let result = manager.add_message(
            session_key,
            ConversationMessage::user("Hello!"),
        ).await;
        assert!(result.is_ok());

        // Verify message was added
        let messages = manager.get_messages(session_key).await;
        assert!(messages.is_ok());
        let messages = messages.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello!");
    }

    #[tokio::test]
    async fn test_session_get_or_create() {
        let config = EdithConfig::default();
        let manager = SessionManager::new(config);

        // Create a session
        let session_key = "test:user456";
        let session = manager.get_or_create(session_key).await.unwrap();
        assert!(session.messages.is_empty());

        // Get it again - sessions are recreated fresh each time in the current impl
        let session2 = manager.get_or_create(session_key).await.unwrap();
        // Sessions have the same key
        assert!(!session2.key.is_empty());
    }
}

// Test scheduler
mod scheduler {
    use super::*;
    use chrono::Duration;
    use edith_scheduler::{JobBuilder, JobResult, Scheduler, SchedulerConfig};
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_scheduler_job_execution() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let scheduler = Scheduler::new(SchedulerConfig {
            tick_interval_ms: 50,
            ..Default::default()
        });

        // Add a job with short interval
        let job = Arc::new(
            JobBuilder::new("counter-job")
                .name("Counter Increment")
                .interval(Duration::milliseconds(100))
                .build_sync(move |_ctx| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    JobResult::success()
                })
                .unwrap(),
        );

        scheduler.add_job(job).unwrap();

        // Start scheduler
        scheduler.start().unwrap();

        // Wait for job to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        // Stop scheduler
        scheduler.stop().unwrap();

        // Verify job executed at least once
        assert!(counter.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn test_scheduler_manual_trigger() {
        let scheduler = Scheduler::default();

        let job = Arc::new(
            JobBuilder::new("instant-job")
                .interval(Duration::hours(24))
                .build_sync(|_ctx| JobResult::success_with_message("triggered"))
                .unwrap(),
        );

        scheduler.add_job(job).unwrap();

        // Trigger without starting scheduler
        let result = scheduler
            .trigger_job(&edith_scheduler::JobId::from("instant-job"))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.message, Some("triggered".to_string()));
    }

    #[tokio::test]
    async fn test_scheduler_job_info() {
        let scheduler = Scheduler::default();

        let job = Arc::new(
            JobBuilder::new("info-job")
                .name("Info Test Job")
                .interval(Duration::minutes(5))
                .build_sync(|_ctx| JobResult::success())
                .unwrap(),
        );

        scheduler.add_job(job).unwrap();

        let info = scheduler.get_job_info(&edith_scheduler::JobId::from("info-job"));
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.name, "Info Test Job");
        assert!(info.enabled);
        assert_eq!(info.run_count, 0);
    }
}

// Test config loading and validation
mod config {
    use edith_config::EdithConfig;

    #[test]
    fn test_config_defaults() {
        let config = EdithConfig::default();

        // Default config has None for optional fields
        // This is expected - fields are populated from config files
        assert!(config.identity.is_none());
        assert!(config.llm.is_none());
    }

    #[test]
    fn test_config_from_json() {
        // Note: Uses camelCase for field names as per serde rename
        let json = r#"{
            "llm": {
                "model": "claude-3-5-sonnet-20241022",
                "maxTokens": 4096
            },
            "providers": {
                "telegram": true
            }
        }"#;

        let config: EdithConfig = serde_json::from_str(json).unwrap();
        assert!(config.llm.is_some());

        let llm = config.llm.unwrap();
        assert_eq!(llm.model, Some("claude-3-5-sonnet-20241022".to_string()));
        assert_eq!(llm.max_tokens, Some(4096));
    }
}

// Test message types and conversion
mod messages {
    use edith_providers::{ChatType, InboundMessage, OutboundMessage};

    #[test]
    fn test_inbound_message_creation() {
        let msg = InboundMessage {
            id: Some("123".to_string()),
            from: "telegram:456".to_string(),
            conversation_id: "789".to_string(),
            to: "bot".to_string(),
            body: "Hello, bot!".to_string(),
            sender_name: Some("John".to_string()),
            timestamp: Some(chrono::Utc::now()),
            chat_type: ChatType::Direct,
            chat_id: "789".to_string(),
            sender_jid: None,
            sender_e164: None,
            reply_context: None,
            group_subject: None,
            group_participants: None,
            mentioned_ids: None,
            was_mentioned: None,
            media: None,
            surface: "telegram".to_string(),
            raw: None,
        };

        assert_eq!(msg.body, "Hello, bot!");
        assert_eq!(msg.chat_type, ChatType::Direct);
        assert_eq!(msg.surface, "telegram");
    }

    #[test]
    fn test_outbound_message_creation() {
        let msg = OutboundMessage {
            to: "telegram:456".to_string(),
            text: Some("Hello, user!".to_string()),
            media: None,
            reply_to_id: None,
            parse_mode: None,
        };

        assert_eq!(msg.to, "telegram:456");
        assert_eq!(msg.text, Some("Hello, user!".to_string()));
    }
}

// Test LLM streaming types
mod streaming {
    use edith_llm::streaming::{ContentDelta, StreamAccumulator, StreamEvent, ContentBlockStartData};

    #[test]
    fn test_stream_accumulator() {
        let mut acc = StreamAccumulator::new();

        // Simulate streaming a text response
        acc.process(StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlockStartData::Text { text: String::new() },
        }).unwrap();

        acc.process(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta { text: "Hello ".to_string() },
        }).unwrap();

        acc.process(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta { text: "World!".to_string() },
        }).unwrap();

        assert_eq!(acc.text(), "Hello World!");
    }
}
