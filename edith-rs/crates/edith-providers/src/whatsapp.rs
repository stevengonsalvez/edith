//! WhatsApp provider via Baileys bridge
//!
//! WhatsApp Web protocol is complex and requires either:
//! 1. A Baileys-like implementation (Signal protocol + WhatsApp specifics)
//! 2. An external bridge/daemon
//!
//! This module provides a bridge-based implementation that connects
//! to a Node.js Baileys bridge via WebSocket.

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;

use crate::{ChatType, InboundMessage, MediaAttachment, MediaType, OutboundMessage, ProviderError, ProviderResult, ReplyContext, ReplyPayload};

/// Bridge frame types for communication with Baileys bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeFrame {
    /// Hello handshake
    Hello {
        client_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// Hello acknowledgment
    HelloOk {
        server_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        self_jid: Option<String>,
    },
    /// Error response
    Error { code: String, message: String },
    /// Ping for keepalive
    Ping { id: String },
    /// Pong response
    Pong { id: String },
    /// RPC request
    Req {
        id: String,
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<serde_json::Value>,
    },
    /// RPC response
    Res {
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<BridgeError>,
    },
    /// Event from bridge
    Event {
        event: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeError {
    pub code: String,
    pub message: String,
}

/// Inbound message from WhatsApp bridge
#[derive(Debug, Clone, Deserialize)]
pub struct BridgeInboundMessage {
    pub id: Option<String>,
    pub from: String,
    pub to: String,
    pub body: String,
    #[serde(rename = "pushName")]
    pub push_name: Option<String>,
    pub timestamp: Option<u64>,
    #[serde(rename = "chatType")]
    pub chat_type: Option<String>,
    #[serde(rename = "chatId")]
    pub chat_id: Option<String>,
    #[serde(rename = "senderJid")]
    pub sender_jid: Option<String>,
    #[serde(rename = "senderE164")]
    pub sender_e164: Option<String>,
    #[serde(rename = "senderName")]
    pub sender_name: Option<String>,
    #[serde(rename = "replyToId")]
    pub reply_to_id: Option<String>,
    #[serde(rename = "replyToBody")]
    pub reply_to_body: Option<String>,
    #[serde(rename = "groupSubject")]
    pub group_subject: Option<String>,
    #[serde(rename = "mediaPath")]
    pub media_path: Option<String>,
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(rename = "mediaUrl")]
    pub media_url: Option<String>,
}

/// WhatsApp provider connecting via WebSocket bridge
pub struct WhatsAppProvider {
    config: EdithConfig,
    connected: Arc<RwLock<bool>>,
    self_jid: Arc<RwLock<Option<String>>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<Option<mpsc::Receiver<InboundMessage>>>>,
    /// Bridge connection URL (WebSocket)
    bridge_url: Option<String>,
    /// Pending RPC requests
    pending_requests: Arc<RwLock<HashMap<String, oneshot::Sender<Result<serde_json::Value, BridgeError>>>>>,
    /// WebSocket sender for outbound messages
    ws_tx: Arc<RwLock<Option<mpsc::Sender<String>>>>,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

impl WhatsAppProvider {
    /// Create a new WhatsApp provider
    pub fn new(config: EdithConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Get bridge URL from config
        let bridge_url = config
            .bridge
            .as_ref()
            .filter(|b| b.enabled.unwrap_or(false))
            .map(|b| {
                let port = b.port.unwrap_or(18790);
                // Derive host from bind mode - default to loopback for local bridge
                let host = match b.bind {
                    Some(edith_config::BridgeBindMode::Lan) => "0.0.0.0",
                    Some(edith_config::BridgeBindMode::Tailnet) => "0.0.0.0",
                    _ => "127.0.0.1", // Auto or Loopback
                };
                format!("ws://{}:{}/ws", host, port)
            });

        Self {
            config,
            connected: Arc::new(RwLock::new(false)),
            self_jid: Arc::new(RwLock::new(None)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(Some(rx))),
            bridge_url,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            ws_tx: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if the bridge is configured
    pub fn has_bridge(&self) -> bool {
        self.bridge_url.is_some()
    }

    /// Get the self JID (own phone number in WhatsApp format)
    pub fn self_jid(&self) -> Option<String> {
        self.self_jid.read().clone()
    }

    /// Send a frame to the bridge
    async fn send_frame(&self, frame: &BridgeFrame) -> ProviderResult<()> {
        let json = serde_json::to_string(frame)
            .map_err(|e| ProviderError::Internal(format!("failed to serialize frame: {}", e)))?;

        let tx = self.ws_tx.read().clone();
        if let Some(tx) = tx {
            tx.send(json).await.map_err(|e| {
                ProviderError::NotAvailable(format!("bridge connection closed: {}", e))
            })?;
            Ok(())
        } else {
            Err(ProviderError::NotAvailable("bridge not connected".to_string()))
        }
    }

    /// Send an RPC request and wait for response
    async fn rpc_call(&self, method: &str, params: Option<serde_json::Value>) -> ProviderResult<serde_json::Value> {
        let id = uuid::Uuid::new_v4().to_string();

        // Create response channel
        let (tx, rx) = oneshot::channel();
        self.pending_requests.write().insert(id.clone(), tx);

        // Send request
        let frame = BridgeFrame::Req {
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        self.send_frame(&frame).await?;

        // Wait for response with timeout
        match timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(payload))) => Ok(payload),
            Ok(Ok(Err(e))) => Err(ProviderError::Internal(format!("RPC error: {} - {}", e.code, e.message))),
            Ok(Err(_)) => Err(ProviderError::Internal("RPC response channel closed".to_string())),
            Err(_) => {
                self.pending_requests.write().remove(&id);
                Err(ProviderError::Timeout("RPC call timed out".to_string()))
            }
        }
    }

    /// Handle incoming bridge frame
    fn handle_frame(&self, frame: BridgeFrame) {
        match frame {
            BridgeFrame::HelloOk { server_name, self_jid } => {
                info!(server = %server_name, jid = ?self_jid, "WhatsApp bridge connected");
                *self.self_jid.write() = self_jid;
                *self.connected.write() = true;
            }
            BridgeFrame::Error { code, message } => {
                error!(code = %code, message = %message, "Bridge error");
            }
            BridgeFrame::Pong { id } => {
                debug!(id = %id, "Received pong");
            }
            BridgeFrame::Res { id, ok, payload, error } => {
                if let Some(tx) = self.pending_requests.write().remove(&id) {
                    let result = if ok {
                        Ok(payload.unwrap_or(serde_json::Value::Null))
                    } else {
                        Err(error.unwrap_or(BridgeError {
                            code: "UNKNOWN".to_string(),
                            message: "Unknown error".to_string(),
                        }))
                    };
                    let _ = tx.send(result);
                }
            }
            BridgeFrame::Event { event, payload } => {
                match event.as_str() {
                    "message" => {
                        if let Some(payload) = payload {
                            match serde_json::from_value::<BridgeInboundMessage>(payload) {
                                Ok(msg) => {
                                    let inbound = self.convert_inbound_message(msg);
                                    let tx = self.message_tx.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = tx.send(inbound).await {
                                            error!("Failed to forward message: {}", e);
                                        }
                                    });
                                }
                                Err(e) => {
                                    warn!("Failed to parse inbound message: {}", e);
                                }
                            }
                        }
                    }
                    "connection.update" => {
                        debug!(payload = ?payload, "Connection update");
                    }
                    _ => {
                        debug!(event = %event, "Unhandled bridge event");
                    }
                }
            }
            _ => {
                debug!(?frame, "Unhandled bridge frame");
            }
        }
    }

    /// Convert bridge message to internal format
    fn convert_inbound_message(&self, msg: BridgeInboundMessage) -> InboundMessage {
        let chat_type = match msg.chat_type.as_deref() {
            Some("group") => ChatType::Group,
            _ => ChatType::Direct,
        };

        let media = msg.media_url.as_ref().map(|url| {
            let mime_type = msg.media_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());
            let media_type = MediaType::from_mime(&mime_type).unwrap_or(MediaType::Document);
            MediaAttachment {
                media_type,
                mime_type,
                url: url.clone(),
                file_name: None,
                size: None,
                caption: None,
            }
        });

        let reply_context = msg.reply_to_id.as_ref().map(|id| ReplyContext {
            message_id: id.clone(),
            body: msg.reply_to_body.clone(),
            sender: None,
        });

        let chat_id = msg.chat_id.clone().unwrap_or_else(|| msg.from.clone());

        InboundMessage {
            id: msg.id,
            from: msg.from.clone(),
            conversation_id: chat_id.clone(),
            to: msg.to,
            body: msg.body,
            sender_name: msg.sender_name.or(msg.push_name),
            timestamp: msg.timestamp.map(|t| {
                chrono::DateTime::from_timestamp_millis(t as i64)
                    .unwrap_or_else(chrono::Utc::now)
            }),
            chat_type,
            chat_id,
            sender_jid: msg.sender_jid,
            sender_e164: msg.sender_e164,
            reply_context,
            group_subject: msg.group_subject,
            group_participants: None,
            mentioned_ids: None,
            was_mentioned: None,
            media,
            surface: "whatsapp".to_string(),
            raw: None,
        }
    }
}

#[async_trait]
impl crate::MessagingProvider for WhatsAppProvider {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn start(&self) -> ProviderResult<()> {
        let bridge_url = match &self.bridge_url {
            Some(url) => url.clone(),
            None => {
                warn!("WhatsApp provider started without bridge configuration");
                warn!("Connect to an external WhatsApp gateway or use the TypeScript implementation");
                return Ok(());
            }
        };

        info!(url = %bridge_url, "Connecting to WhatsApp bridge");

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&bridge_url).await.map_err(|e| {
            ProviderError::NotAvailable(format!("failed to connect to bridge: {}", e))
        })?;

        let (mut write, mut read) = ws_stream.split();

        // Create channels for communication
        let (ws_tx, mut ws_rx) = mpsc::channel::<String>(100);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        *self.ws_tx.write() = Some(ws_tx);
        *self.shutdown_tx.write() = Some(shutdown_tx);

        // Send hello
        let client_id = uuid::Uuid::new_v4().to_string();
        let hello = BridgeFrame::Hello {
            client_id,
            // Bridge authentication token - not currently in BridgeConfig
            // Could be added later if bridge requires auth
            token: None,
        };
        let hello_json = serde_json::to_string(&hello)
            .map_err(|e| ProviderError::Internal(format!("failed to serialize hello: {}", e)))?;

        write.send(WsMessage::Text(hello_json.into())).await.map_err(|e| {
            ProviderError::NotAvailable(format!("failed to send hello: {}", e))
        })?;

        // Clone references for the read task
        let pending_requests = self.pending_requests.clone();
        let message_tx = self.message_tx.clone();
        let connected = self.connected.clone();
        let self_jid = self.self_jid.clone();

        // Spawn read task
        let read_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(WsMessage::Text(text))) => {
                                match serde_json::from_str::<BridgeFrame>(&text) {
                                    Ok(frame) => {
                                        match &frame {
                                            BridgeFrame::HelloOk { server_name, self_jid: jid } => {
                                                info!(server = %server_name, jid = ?jid, "WhatsApp bridge connected");
                                                *self_jid.write() = jid.clone();
                                                *connected.write() = true;
                                            }
                                            BridgeFrame::Res { id, ok, payload, error } => {
                                                if let Some(tx) = pending_requests.write().remove(id) {
                                                    let result = if *ok {
                                                        Ok(payload.clone().unwrap_or(serde_json::Value::Null))
                                                    } else {
                                                        Err(error.clone().unwrap_or(BridgeError {
                                                            code: "UNKNOWN".to_string(),
                                                            message: "Unknown error".to_string(),
                                                        }))
                                                    };
                                                    let _ = tx.send(result);
                                                }
                                            }
                                            BridgeFrame::Event { event, payload } => {
                                                if event == "message" {
                                                    if let Some(payload) = payload {
                                                        if let Ok(msg) = serde_json::from_value::<BridgeInboundMessage>(payload.clone()) {
                                                            let chat_type = match msg.chat_type.as_deref() {
                                                                Some("group") => ChatType::Group,
                                                                _ => ChatType::Direct,
                                                            };
                                                            let media = msg.media_url.as_ref().map(|url| {
                                                                let mime_type = msg.media_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());
                                                                let media_type = MediaType::from_mime(&mime_type).unwrap_or(MediaType::Document);
                                                                MediaAttachment {
                                                                    media_type,
                                                                    mime_type,
                                                                    url: url.clone(),
                                                                    file_name: None,
                                                                    size: None,
                                                                    caption: None,
                                                                }
                                                            });
                                                            let reply_context = msg.reply_to_id.as_ref().map(|id| ReplyContext {
                                                                message_id: id.clone(),
                                                                body: msg.reply_to_body.clone(),
                                                                sender: None,
                                                            });
                                                            let chat_id = msg.chat_id.clone().unwrap_or_else(|| msg.from.clone());
                                                            let inbound = InboundMessage {
                                                                id: msg.id,
                                                                from: msg.from.clone(),
                                                                conversation_id: chat_id.clone(),
                                                                to: msg.to,
                                                                body: msg.body,
                                                                sender_name: msg.sender_name.or(msg.push_name),
                                                                timestamp: msg.timestamp.map(|t| {
                                                                    chrono::DateTime::from_timestamp_millis(t as i64)
                                                                        .unwrap_or_else(chrono::Utc::now)
                                                                }),
                                                                chat_type,
                                                                chat_id,
                                                                sender_jid: msg.sender_jid,
                                                                sender_e164: msg.sender_e164,
                                                                reply_context,
                                                                group_subject: msg.group_subject,
                                                                group_participants: None,
                                                                mentioned_ids: None,
                                                                was_mentioned: None,
                                                                media,
                                                                surface: "whatsapp".to_string(),
                                                                raw: None,
                                                            };
                                                            let _ = message_tx.send(inbound).await;
                                                        }
                                                    }
                                                }
                                            }
                                            BridgeFrame::Error { code, message } => {
                                                error!(code = %code, message = %message, "Bridge error");
                                            }
                                            _ => {}
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse bridge frame: {}", e);
                                    }
                                }
                            }
                            Some(Ok(WsMessage::Close(_))) => {
                                info!("Bridge connection closed");
                                *connected.write() = false;
                                break;
                            }
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                *connected.write() = false;
                                break;
                            }
                            None => {
                                info!("Bridge connection ended");
                                *connected.write() = false;
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = &mut shutdown_rx => {
                        info!("Shutdown signal received");
                        break;
                    }
                }
            }
        });

        // Spawn write task
        tokio::spawn(async move {
            while let Some(msg) = ws_rx.recv().await {
                if let Err(e) = write.send(WsMessage::Text(msg.into())).await {
                    error!("Failed to send message: {}", e);
                    break;
                }
            }
        });

        // Wait briefly for connection to establish
        tokio::time::sleep(Duration::from_millis(500)).await;

        if !*self.connected.read() {
            return Err(ProviderError::NotAvailable(
                "Failed to establish bridge connection".to_string(),
            ));
        }

        info!("WhatsApp bridge connection established");
        Ok(())
    }

    async fn stop(&self) -> ProviderResult<()> {
        info!("Stopping WhatsApp provider");

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.write().take() {
            let _ = tx.send(());
        }

        *self.connected.write() = false;
        *self.ws_tx.write() = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }

    async fn send(&self, message: OutboundMessage) -> ProviderResult<String> {
        if !self.has_bridge() {
            return Err(ProviderError::NotConfigured(
                "WhatsApp bridge not configured".to_string(),
            ));
        }

        if !self.is_connected() {
            return Err(ProviderError::NotAvailable(
                "WhatsApp bridge not connected".to_string(),
            ));
        }

        // Convert to WhatsApp JID
        let jid = to_whatsapp_jid(&message.to);

        // Build send params
        let mut params = serde_json::json!({
            "jid": jid,
            "text": message.text,
        });

        if let Some(reply_to) = &message.reply_to_id {
            params["quotedMessageId"] = serde_json::Value::String(reply_to.clone());
        }

        // Send via RPC
        let result = self.rpc_call("send", Some(params)).await?;

        // Extract message ID from response
        let message_id = result
            .get("messageId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(message_id)
    }

    async fn send_reply(&self, chat_id: &str, reply: ReplyPayload) -> ProviderResult<String> {
        self.send(OutboundMessage {
            to: chat_id.to_string(),
            text: reply.text,
            media: None,
            reply_to_id: reply.reply_to_id,
            parse_mode: None,
        })
        .await
    }

    async fn send_typing(&self, chat_id: &str) -> ProviderResult<()> {
        if !self.is_connected() {
            return Ok(()); // Silently ignore
        }

        let jid = to_whatsapp_jid(chat_id);
        let params = serde_json::json!({
            "jid": jid,
            "presence": "composing",
        });

        let _ = self.rpc_call("presence", Some(params)).await;
        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<InboundMessage> {
        self.message_rx
            .write()
            .take()
            .expect("subscribe can only be called once")
    }
}

/// Normalize a phone number to E.164 format
pub fn normalize_e164(phone: &str) -> String {
    let cleaned: String = phone
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();

    if cleaned.starts_with('+') {
        cleaned
    } else if cleaned.len() == 10 {
        // US number
        format!("+1{}", cleaned)
    } else if cleaned.len() > 10 && !cleaned.starts_with('0') {
        format!("+{}", cleaned)
    } else {
        cleaned
    }
}

/// Convert E.164 to WhatsApp JID
pub fn to_whatsapp_jid(e164: &str) -> String {
    // If already a JID, return as-is
    if e164.contains('@') {
        return e164.to_string();
    }

    let normalized = normalize_e164(e164);
    let digits: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
    format!("{}@s.whatsapp.net", digits)
}

/// Extract E.164 from WhatsApp JID
pub fn jid_to_e164(jid: &str) -> Option<String> {
    let parts: Vec<&str> = jid.split('@').collect();
    if parts.len() != 2 {
        return None;
    }

    let number = parts[0];
    if number.chars().all(|c| c.is_ascii_digit()) {
        Some(format!("+{}", number))
    } else {
        None
    }
}

/// Check if a JID is a group
pub fn is_jid_group(jid: &str) -> bool {
    jid.ends_with("@g.us")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_e164() {
        assert_eq!(normalize_e164("1234567890"), "+11234567890");
        assert_eq!(normalize_e164("+1234567890"), "+1234567890");
        assert_eq!(normalize_e164("441234567890"), "+441234567890");
    }

    #[test]
    fn test_to_whatsapp_jid() {
        assert_eq!(
            to_whatsapp_jid("+14155551234"),
            "14155551234@s.whatsapp.net"
        );
        // Already a JID
        assert_eq!(
            to_whatsapp_jid("14155551234@s.whatsapp.net"),
            "14155551234@s.whatsapp.net"
        );
    }

    #[test]
    fn test_jid_to_e164() {
        assert_eq!(
            jid_to_e164("14155551234@s.whatsapp.net"),
            Some("+14155551234".to_string())
        );
        assert_eq!(jid_to_e164("invalid"), None);
    }

    #[test]
    fn test_is_jid_group() {
        assert!(is_jid_group("123456789@g.us"));
        assert!(!is_jid_group("14155551234@s.whatsapp.net"));
    }
}
