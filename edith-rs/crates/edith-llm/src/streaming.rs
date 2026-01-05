//! Streaming response handling for LLM APIs

use bytes::Bytes;
use futures_util::Stream;
use serde::Deserialize;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

use crate::error::{LlmError, LlmResult};
use crate::message::{ContentBlock, StopReason, Usage};

/// Server-sent event types for streaming
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStartData,
    },
    Ping,
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: Option<Usage>,
    },
    MessageStop,
    Error {
        error: StreamErrorData,
    },
}

/// Message start data
#[derive(Debug, Clone, Deserialize)]
pub struct MessageStartData {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: String,
    pub model: String,
    pub usage: Option<Usage>,
}

/// Content block start data
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockStartData {
    Text { text: String },
    ToolUse { id: String, name: String },
}

/// Content delta
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

/// Message delta
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
}

/// Stream error data
#[derive(Debug, Clone, Deserialize)]
pub struct StreamErrorData {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Accumulated stream state
#[derive(Debug, Default)]
pub struct StreamAccumulator {
    pub message_id: Option<String>,
    pub model: Option<String>,
    pub content_blocks: Vec<AccumulatingBlock>,
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

/// Block being accumulated during streaming
#[derive(Debug, Clone)]
pub enum AccumulatingBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a stream event
    pub fn process(&mut self, event: StreamEvent) -> LlmResult<Option<StreamDelta>> {
        match event {
            StreamEvent::MessageStart { message } => {
                self.message_id = Some(message.id);
                self.model = Some(message.model);
                if let Some(usage) = message.usage {
                    self.usage = usage;
                }
                Ok(None)
            }
            StreamEvent::ContentBlockStart { index, content_block } => {
                while self.content_blocks.len() <= index {
                    self.content_blocks.push(AccumulatingBlock::Text(String::new()));
                }
                match content_block {
                    ContentBlockStartData::Text { text } => {
                        self.content_blocks[index] = AccumulatingBlock::Text(text);
                    }
                    ContentBlockStartData::ToolUse { id, name } => {
                        self.content_blocks[index] = AccumulatingBlock::ToolUse {
                            id,
                            name,
                            input_json: String::new(),
                        };
                    }
                }
                Ok(None)
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                if index >= self.content_blocks.len() {
                    return Ok(None);
                }
                match (&mut self.content_blocks[index], delta) {
                    (AccumulatingBlock::Text(text), ContentDelta::TextDelta { text: delta }) => {
                        text.push_str(&delta);
                        Ok(Some(StreamDelta::Text(delta)))
                    }
                    (
                        AccumulatingBlock::ToolUse { input_json, .. },
                        ContentDelta::InputJsonDelta { partial_json },
                    ) => {
                        input_json.push_str(&partial_json);
                        Ok(Some(StreamDelta::ToolInput(partial_json)))
                    }
                    _ => Ok(None),
                }
            }
            StreamEvent::ContentBlockStop { .. } => Ok(None),
            StreamEvent::MessageDelta { delta, usage } => {
                self.stop_reason = delta.stop_reason;
                self.stop_sequence = delta.stop_sequence;
                if let Some(u) = usage {
                    self.usage.output_tokens = u.output_tokens;
                }
                Ok(None)
            }
            StreamEvent::MessageStop => Ok(Some(StreamDelta::Done)),
            StreamEvent::Ping => Ok(None),
            StreamEvent::Error { error } => Err(LlmError::StreamError(format!(
                "{}: {}",
                error.error_type, error.message
            ))),
        }
    }

    /// Get final content blocks
    pub fn into_content_blocks(self) -> Vec<ContentBlock> {
        self.content_blocks
            .into_iter()
            .filter_map(|block| match block {
                AccumulatingBlock::Text(text) if !text.is_empty() => {
                    Some(ContentBlock::Text { text })
                }
                AccumulatingBlock::ToolUse {
                    id,
                    name,
                    input_json,
                } => {
                    let input = serde_json::from_str(&input_json).unwrap_or(serde_json::Value::Null);
                    Some(ContentBlock::ToolUse { id, name, input })
                }
                _ => None,
            })
            .collect()
    }

    /// Get accumulated text
    pub fn text(&self) -> String {
        self.content_blocks
            .iter()
            .filter_map(|b| match b {
                AccumulatingBlock::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Delta update during streaming
#[derive(Debug, Clone)]
pub enum StreamDelta {
    Text(String),
    ToolInput(String),
    Done,
}

/// Parse SSE line into event
pub fn parse_sse_line(line: &str) -> LlmResult<Option<StreamEvent>> {
    let line = line.trim();

    if line.is_empty() || line.starts_with(':') {
        return Ok(None);
    }

    if let Some(data) = line.strip_prefix("data: ") {
        if data == "[DONE]" {
            return Ok(Some(StreamEvent::MessageStop));
        }

        let event: StreamEvent = serde_json::from_str(data)?;
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

/// Stream of completion deltas
pub struct CompletionStream {
    rx: mpsc::Receiver<LlmResult<StreamDelta>>,
    accumulator: StreamAccumulator,
}

impl CompletionStream {
    pub fn new(rx: mpsc::Receiver<LlmResult<StreamDelta>>) -> Self {
        Self {
            rx,
            accumulator: StreamAccumulator::new(),
        }
    }

    /// Get accumulated text so far
    pub fn current_text(&self) -> String {
        self.accumulator.text()
    }

    /// Consume stream and get final accumulator
    pub async fn collect(mut self) -> LlmResult<StreamAccumulator> {
        while let Some(result) = self.rx.recv().await {
            match result {
                Ok(StreamDelta::Done) => break,
                Ok(_) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(self.accumulator)
    }
}

impl Stream for CompletionStream {
    type Item = LlmResult<StreamDelta>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_recv(cx)
    }
}

/// SSE parser for byte streams
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed bytes and get parsed events
    pub fn feed(&mut self, bytes: &Bytes) -> Vec<LlmResult<StreamEvent>> {
        let text = String::from_utf8_lossy(bytes);
        self.buffer.push_str(&text);

        let mut events = Vec::new();

        while let Some(pos) = self.buffer.find("\n\n") {
            let chunk = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 2..].to_string();

            for line in chunk.lines() {
                match parse_sse_line(line) {
                    Ok(Some(event)) => events.push(Ok(event)),
                    Ok(None) => {}
                    Err(e) => events.push(Err(e)),
                }
            }
        }

        events
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_line() {
        let line = r#"data: {"type":"ping"}"#;
        let event = parse_sse_line(line).unwrap().unwrap();
        assert!(matches!(event, StreamEvent::Ping));
    }

    #[test]
    fn test_parse_sse_done() {
        let line = "data: [DONE]";
        let event = parse_sse_line(line).unwrap().unwrap();
        assert!(matches!(event, StreamEvent::MessageStop));
    }

    #[test]
    fn test_accumulator_text() {
        let mut acc = StreamAccumulator::new();

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
            delta: ContentDelta::TextDelta { text: "World".to_string() },
        }).unwrap();

        assert_eq!(acc.text(), "Hello World");
    }
}
