//! Signal provider using signal-cli JSON-RPC interface
//!
//! This provider communicates with signal-cli's JSON-RPC HTTP daemon.
//! The daemon must be running separately (e.g., via `signal-cli daemon`).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;

use crate::{
    ChatType, InboundMessage, MediaAttachment, MediaType, OutboundMessage, ProviderError,
    ProviderResult, ReplyPayload,
};

const SIGNAL_TEXT_LIMIT: usize = 6000;

/// Signal envelope from JSON-RPC
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalEnvelope {
    source_number: Option<String>,
    source_name: Option<String>,
    timestamp: Option<i64>,
    data_message: Option<SignalDataMessage>,
    #[serde(default)]
    sync_message: Option<serde_json::Value>,
}

/// Signal data message
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalDataMessage {
    timestamp: Option<i64>,
    message: Option<String>,
    #[serde(default)]
    attachments: Vec<SignalAttachment>,
    group_info: Option<SignalGroupInfo>,
    quote: Option<SignalQuote>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalGroupInfo {
    group_id: Option<String>,
    group_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SignalQuote {
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalAttachment {
    id: Option<String>,
    content_type: Option<String>,
    filename: Option<String>,
    size: Option<u64>,
}

/// Signal receive payload from SSE
#[derive(Debug, Clone, Deserialize)]
struct SignalReceivePayload {
    account: Option<String>,
    envelope: Option<SignalEnvelope>,
    exception: Option<SignalException>,
}

#[derive(Debug, Clone, Deserialize)]
struct SignalException {
    message: Option<String>,
}

/// JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: Serialize> {
    jsonrpc: &'static str,
    method: &'static str,
    id: String,
    params: T,
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    id: String,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Send message params
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendMessageParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<String>,
    recipients: Vec<String>,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<String>>,
}

/// Signal provider
pub struct SignalProvider {
    config: EdithConfig,
    base_url: String,
    account: Option<String>,
    connected: Arc<RwLock<bool>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<Option<mpsc::Receiver<InboundMessage>>>>,
    http_client: reqwest::Client,
}

impl SignalProvider {
    /// Create a new Signal provider
    pub fn new(config: EdithConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Resolve base URL from config
        let signal_config = config.signal.as_ref();
        let host = signal_config
            .and_then(|s| s.http_host.clone())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port = signal_config.and_then(|s| s.http_port).unwrap_or(8080);
        let base_url = signal_config
            .and_then(|s| s.http_url.clone())
            .unwrap_or_else(|| format!("http://{}:{}", host, port));

        let account = signal_config.and_then(|s| s.account.clone());

        Self {
            config,
            base_url,
            account,
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(Some(rx))),
            http_client: reqwest::Client::new(),
        }
    }

    /// Check if daemon is available
    async fn check_daemon(&self) -> bool {
        let url = format!("{}/v1/about", self.base_url);
        match self.http_client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Get allow list from config
    fn get_allow_list(&self) -> Vec<String> {
        self.config
            .signal
            .as_ref()
            .and_then(|s| s.allow_from.as_ref())
            .map(|list| {
                list.iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Number(n) => Some(n.to_string()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if sender is allowed
    fn is_sender_allowed(&self, sender: &str) -> bool {
        let allow_list = self.get_allow_list();
        if allow_list.is_empty() || allow_list.contains(&"*".to_string()) {
            return true;
        }

        let normalized_sender = Self::normalize_e164(sender);
        allow_list.iter().any(|entry| {
            let normalized = Self::normalize_e164(entry.trim_start_matches("signal:"));
            normalized == normalized_sender
        })
    }

    /// Normalize E.164 phone number
    fn normalize_e164(phone: &str) -> String {
        phone
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect()
    }

    /// Get text chunk limit from config
    fn text_chunk_limit(&self) -> usize {
        self.config
            .signal
            .as_ref()
            .and_then(|s| s.text_chunk_limit)
            .unwrap_or(SIGNAL_TEXT_LIMIT as u64) as usize
    }

    /// Chunk text to respect platform limits
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let limit = self.text_chunk_limit();
        if text.len() <= limit {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for line in text.lines() {
            if current.len() + line.len() + 1 > limit {
                if !current.is_empty() {
                    chunks.push(current);
                    current = String::new();
                }
                if line.len() > limit {
                    for chunk in line.as_bytes().chunks(limit) {
                        chunks.push(String::from_utf8_lossy(chunk).to_string());
                    }
                    continue;
                }
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }

    /// Convert Signal envelope to InboundMessage
    fn convert_envelope(&self, payload: SignalReceivePayload) -> Option<InboundMessage> {
        let envelope = payload.envelope?;
        let data_message = envelope.data_message?;

        let source = envelope.source_number.as_deref()?;
        let body = data_message.message.as_deref().unwrap_or("").trim();

        // Check allow list
        if !self.is_sender_allowed(source) {
            debug!(source = source, "Blocked unauthorized Signal sender");
            return None;
        }

        // Skip empty messages without attachments
        if body.is_empty() && data_message.attachments.is_empty() {
            return None;
        }

        let is_group = data_message.group_info.is_some();
        let group_id = data_message
            .group_info
            .as_ref()
            .and_then(|g| g.group_id.clone());
        let group_name = data_message
            .group_info
            .as_ref()
            .and_then(|g| g.group_name.clone());

        // Extract media
        let media = data_message.attachments.first().map(|a| {
            let content_type = a.content_type.as_deref().unwrap_or("application/octet-stream");
            let media_type = if content_type.starts_with("image/") {
                MediaType::Image
            } else if content_type.starts_with("video/") {
                MediaType::Video
            } else if content_type.starts_with("audio/") {
                MediaType::Audio
            } else {
                MediaType::Document
            };

            MediaAttachment {
                media_type,
                mime_type: content_type.to_string(),
                url: a.id.clone().unwrap_or_default(),
                file_name: a.filename.clone(),
                size: a.size,
                caption: None,
            }
        });

        let chat_id = group_id.unwrap_or_else(|| source.to_string());
        let timestamp = envelope
            .timestamp
            .or(data_message.timestamp)
            .and_then(|ts| chrono::DateTime::from_timestamp_millis(ts))
            .unwrap_or_else(chrono::Utc::now);

        Some(InboundMessage {
            id: envelope.timestamp.map(|t| t.to_string()),
            from: format!("signal:{}", source),
            conversation_id: chat_id.clone(),
            to: format!("signal:{}", chat_id),
            body: body.to_string(),
            sender_name: envelope.source_name,
            timestamp: Some(timestamp),
            chat_type: if is_group {
                ChatType::Group
            } else {
                ChatType::Direct
            },
            chat_id,
            sender_jid: None,
            sender_e164: Some(source.to_string()),
            reply_context: data_message.quote.map(|q| crate::ReplyContext {
                message_id: String::new(),
                body: q.text,
                sender: None,
            }),
            group_subject: group_name,
            group_participants: None,
            mentioned_ids: None,
            was_mentioned: None,
            media,
            surface: "signal".to_string(),
            raw: None,
        })
    }

    /// Start SSE event stream
    async fn start_event_stream(&self) {
        let base_url = self.base_url.clone();
        let account = self.account.clone();
        let message_tx = self.message_tx.clone();
        let connected = self.connected.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let provider = SignalProvider {
                config,
                base_url: base_url.clone(),
                account: account.clone(),
                connected: connected.clone(),
                message_tx: message_tx.clone(),
                message_rx: Arc::new(RwLock::new(None)),
                http_client: reqwest::Client::new(),
            };

            let url = match &account {
                Some(acc) => format!("{}/v1/receive/{}", base_url, acc),
                None => format!("{}/v1/receive", base_url),
            };

            loop {
                if !*connected.read().await {
                    break;
                }

                match reqwest::get(&url).await {
                    Ok(response) => {
                        let mut stream = response.bytes_stream();
                        use futures_util::StreamExt;

                        while let Some(chunk_result) = stream.next().await {
                            if !*connected.read().await {
                                break;
                            }

                            if let Ok(chunk) = chunk_result {
                                let text = String::from_utf8_lossy(&chunk);
                                for line in text.lines() {
                                    if line.starts_with("data:") {
                                        let json = line.trim_start_matches("data:").trim();
                                        if let Ok(payload) =
                                            serde_json::from_str::<SignalReceivePayload>(json)
                                        {
                                            if let Some(inbound) =
                                                provider.convert_envelope(payload)
                                            {
                                                if message_tx.send(inbound).await.is_err() {
                                                    warn!("Failed to send Signal message");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Signal SSE connection failed");
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }
}

#[async_trait]
impl crate::MessagingProvider for SignalProvider {
    fn name(&self) -> &'static str {
        "signal"
    }

    async fn start(&self) -> ProviderResult<()> {
        info!("Starting Signal provider");

        // Check if daemon is available
        if !self.check_daemon().await {
            return Err(ProviderError::NotAvailable(
                "Signal daemon not running".to_string(),
            ));
        }

        *self.connected.write().await = true;
        info!(base_url = %self.base_url, "Signal daemon connected");

        // Start event stream
        self.start_event_stream().await;

        Ok(())
    }

    async fn stop(&self) -> ProviderResult<()> {
        info!("Stopping Signal provider");
        *self.connected.write().await = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.try_read().map(|g| *g).unwrap_or(false)
    }

    async fn send(&self, message: OutboundMessage) -> ProviderResult<String> {
        // Parse recipient
        let recipient = message
            .to
            .strip_prefix("signal:")
            .unwrap_or(&message.to)
            .to_string();

        // Send text messages
        if let Some(text) = message.text {
            let chunks = self.chunk_text(&text);

            for chunk in chunks {
                let params = SendMessageParams {
                    account: self.account.clone(),
                    recipients: vec![recipient.clone()],
                    message: chunk,
                    quote_timestamp: None,
                    quote_author: None,
                    attachments: None,
                };

                let request = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: "send",
                    id: uuid::Uuid::new_v4().to_string(),
                    params,
                };

                let url = format!("{}/v1/rpc", self.base_url);
                let response = self
                    .http_client
                    .post(&url)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| ProviderError::SendFailed(e.to_string()))?;

                if !response.status().is_success() {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    return Err(ProviderError::SendFailed(format!(
                        "Signal RPC error {}: {}",
                        status, text
                    )));
                }
            }

            return Ok("sent".to_string());
        }

        Ok("sent".to_string())
    }

    async fn send_reply(&self, chat_id: &str, reply: ReplyPayload) -> ProviderResult<String> {
        self.send(OutboundMessage {
            to: format!("signal:{}", chat_id),
            text: reply.text,
            media: reply.media_url.map(|url| {
                vec![MediaAttachment {
                    media_type: MediaType::Document,
                    mime_type: "application/octet-stream".to_string(),
                    url,
                    file_name: None,
                    size: None,
                    caption: None,
                }]
            }),
            reply_to_id: reply.reply_to_id,
            parse_mode: reply.parse_mode,
        })
        .await
    }

    async fn send_typing(&self, _chat_id: &str) -> ProviderResult<()> {
        // Signal doesn't support typing indicators via JSON-RPC
        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<InboundMessage> {
        futures::executor::block_on(async {
            self.message_rx
                .write()
                .await
                .take()
                .expect("subscribe can only be called once")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_e164() {
        assert_eq!(SignalProvider::normalize_e164("+1234567890"), "+1234567890");
        assert_eq!(SignalProvider::normalize_e164("1234567890"), "1234567890");
        assert_eq!(
            SignalProvider::normalize_e164("+1 (234) 567-890"),
            "+1234567890"
        );
    }

    #[test]
    fn test_chunk_text() {
        let config = EdithConfig::default();
        let provider = SignalProvider::new(config);

        let text = "Hello World";
        let chunks = provider.chunk_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }
}
