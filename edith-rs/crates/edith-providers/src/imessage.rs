//! iMessage provider using imsg CLI JSON-RPC interface (macOS only)
//!
//! This provider communicates with the `imsg` CLI tool via JSON-RPC over stdin/stdout.
//! The `imsg` binary must be installed and available in PATH or configured via `cliPath`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use edith_config::EdithConfig;

use crate::{
    ChatType, InboundMessage, MediaAttachment, MediaType, OutboundMessage, ProviderError,
    ProviderResult, ReplyPayload,
};

const IMESSAGE_TEXT_LIMIT: usize = 20000;

// ============================================================================
// Target Types
// ============================================================================

/// iMessage service type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IMessageService {
    IMessage,
    Sms,
    Auto,
}

impl Default for IMessageService {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::fmt::Display for IMessageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IMessage => write!(f, "imessage"),
            Self::Sms => write!(f, "sms"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

/// Parsed iMessage target
#[derive(Debug, Clone)]
pub enum IMessageTarget {
    ChatId { chat_id: i64 },
    ChatGuid { chat_guid: String },
    ChatIdentifier { chat_identifier: String },
    Handle { to: String, service: IMessageService },
}

const CHAT_ID_PREFIXES: &[&str] = &["chat_id:", "chatid:", "chat:"];
const CHAT_GUID_PREFIXES: &[&str] = &["chat_guid:", "chatguid:", "guid:"];
const CHAT_IDENTIFIER_PREFIXES: &[&str] = &["chat_identifier:", "chatidentifier:", "chatident:"];

/// Normalize an E.164 phone number
fn normalize_e164(phone: &str) -> String {
    phone
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect()
}

/// Normalize an iMessage handle (email or phone)
pub fn normalize_imessage_handle(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lowered = trimmed.to_lowercase();
    if lowered.starts_with("imessage:") {
        return normalize_imessage_handle(&trimmed[9..]);
    }
    if lowered.starts_with("sms:") {
        return normalize_imessage_handle(&trimmed[4..]);
    }
    if lowered.starts_with("auto:") {
        return normalize_imessage_handle(&trimmed[5..]);
    }

    // Email address
    if trimmed.contains('@') {
        return trimmed.to_lowercase();
    }

    // Phone number
    let normalized = normalize_e164(trimmed);
    if !normalized.is_empty() {
        return normalized;
    }

    trimmed.split_whitespace().collect()
}

/// Parse an iMessage target string
pub fn parse_imessage_target(raw: &str) -> Result<IMessageTarget, ProviderError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::InvalidRecipient(
            "iMessage target is required".to_string(),
        ));
    }
    let lower = trimmed.to_lowercase();

    // Check for service prefix (imessage:, sms:, auto:)
    for (prefix, service) in [
        ("imessage:", IMessageService::IMessage),
        ("sms:", IMessageService::Sms),
        ("auto:", IMessageService::Auto),
    ] {
        if lower.starts_with(prefix) {
            let remainder = trimmed[prefix.len()..].trim();
            if remainder.is_empty() {
                return Err(ProviderError::InvalidRecipient(format!(
                    "{} target is required",
                    prefix
                )));
            }

            // Check if remainder is a chat target
            let remainder_lower = remainder.to_lowercase();
            let is_chat_target = CHAT_ID_PREFIXES
                .iter()
                .any(|p| remainder_lower.starts_with(p))
                || CHAT_GUID_PREFIXES
                    .iter()
                    .any(|p| remainder_lower.starts_with(p))
                || CHAT_IDENTIFIER_PREFIXES
                    .iter()
                    .any(|p| remainder_lower.starts_with(p))
                || remainder_lower.starts_with("group:");

            if is_chat_target {
                return parse_imessage_target(remainder);
            }

            return Ok(IMessageTarget::Handle {
                to: remainder.to_string(),
                service,
            });
        }
    }

    // Check for chat_id
    for prefix in CHAT_ID_PREFIXES {
        if lower.starts_with(prefix) {
            let value = trimmed[prefix.len()..].trim();
            let chat_id: i64 = value.parse().map_err(|_| {
                ProviderError::InvalidRecipient(format!("Invalid chat_id: {}", value))
            })?;
            return Ok(IMessageTarget::ChatId { chat_id });
        }
    }

    // Check for chat_guid
    for prefix in CHAT_GUID_PREFIXES {
        if lower.starts_with(prefix) {
            let value = trimmed[prefix.len()..].trim();
            if value.is_empty() {
                return Err(ProviderError::InvalidRecipient(
                    "chat_guid is required".to_string(),
                ));
            }
            return Ok(IMessageTarget::ChatGuid {
                chat_guid: value.to_string(),
            });
        }
    }

    // Check for chat_identifier
    for prefix in CHAT_IDENTIFIER_PREFIXES {
        if lower.starts_with(prefix) {
            let value = trimmed[prefix.len()..].trim();
            if value.is_empty() {
                return Err(ProviderError::InvalidRecipient(
                    "chat_identifier is required".to_string(),
                ));
            }
            return Ok(IMessageTarget::ChatIdentifier {
                chat_identifier: value.to_string(),
            });
        }
    }

    // Check for group:
    if lower.starts_with("group:") {
        let value = trimmed[6..].trim();
        if let Ok(chat_id) = value.parse::<i64>() {
            return Ok(IMessageTarget::ChatId { chat_id });
        }
        if value.is_empty() {
            return Err(ProviderError::InvalidRecipient(
                "group target is required".to_string(),
            ));
        }
        return Ok(IMessageTarget::ChatGuid {
            chat_guid: value.to_string(),
        });
    }

    // Default to handle with auto service
    Ok(IMessageTarget::Handle {
        to: trimmed.to_string(),
        service: IMessageService::Auto,
    })
}

/// Format a chat_id as target string
pub fn format_chat_target(chat_id: Option<i64>) -> String {
    match chat_id {
        Some(id) if id != 0 => format!("chat_id:{}", id),
        _ => String::new(),
    }
}

// ============================================================================
// JSON-RPC Types
// ============================================================================

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: Serialize> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<u64>,
    result: Option<T>,
    error: Option<JsonRpcError>,
    method: Option<String>,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    #[allow(dead_code)]
    code: Option<i64>,
    message: Option<String>,
}

// ============================================================================
// iMessage Payload Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct IMessageAttachmentPayload {
    original_path: Option<String>,
    mime_type: Option<String>,
    missing: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct IMessagePayload {
    id: Option<i64>,
    chat_id: Option<i64>,
    sender: Option<String>,
    is_from_me: Option<bool>,
    text: Option<String>,
    created_at: Option<String>,
    attachments: Option<Vec<IMessageAttachmentPayload>>,
    chat_identifier: Option<String>,
    chat_guid: Option<String>,
    chat_name: Option<String>,
    participants: Option<Vec<String>>,
    is_group: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageNotification {
    message: Option<IMessagePayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct WatchSubscribeResult {
    subscription: Option<u64>,
}

// ============================================================================
// iMessage RPC Client
// ============================================================================

struct IMessageRpcClient {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    next_id: Arc<Mutex<u64>>,
}

impl IMessageRpcClient {
    async fn new(
        cli_path: &str,
        db_path: Option<&str>,
        on_notification: mpsc::Sender<JsonRpcResponse<serde_json::Value>>,
    ) -> Result<Self, ProviderError> {
        let mut cmd = Command::new(cli_path);
        cmd.arg("rpc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(db) = db_path {
            cmd.args(["--db", db]);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| ProviderError::NotAvailable(format!("Failed to spawn imsg: {}", e)))?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Handle stdout (JSON-RPC responses)
        if let Some(stdout) = stdout {
            let pending_clone = pending.clone();
            let notif_tx = on_notification.clone();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<JsonRpcResponse<serde_json::Value>>(trimmed) {
                        Ok(response) => {
                            // Check if it's a response to a request
                            if let Some(id) = response.id {
                                let mut pending = pending_clone.lock().await;
                                if let Some(tx) = pending.remove(&id) {
                                    let _ = tx.send(
                                        response
                                            .result
                                            .unwrap_or(serde_json::Value::Null),
                                    );
                                    continue;
                                }
                            }

                            // It's a notification
                            if response.method.is_some() {
                                let _ = notif_tx.send(response).await;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, line = %trimmed, "Failed to parse imsg response");
                        }
                    }
                }
            });
        }

        // Handle stderr (log errors)
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if !line.trim().is_empty() {
                        debug!(stderr = %line, "imsg stderr");
                    }
                }
            });
        }

        Ok(Self {
            child: Some(child),
            stdin,
            pending,
            next_id: Arc::new(Mutex::new(1)),
        })
    }

    async fn request<T: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &'static str,
        params: T,
        timeout_ms: u64,
    ) -> Result<R, ProviderError> {
        let id = {
            let mut next_id = self.next_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let line = serde_json::to_string(&request)
            .map_err(|e| ProviderError::Internal(format!("Failed to serialize request: {}", e)))?;

        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| ProviderError::NotAvailable("imsg stdin not available".to_string()))?;

        stdin
            .write_all(format!("{}\n", line).as_bytes())
            .await
            .map_err(|e| ProviderError::SendFailed(format!("Failed to write to imsg: {}", e)))?;

        stdin
            .flush()
            .await
            .map_err(|e| ProviderError::SendFailed(format!("Failed to flush imsg: {}", e)))?;

        // Wait for response
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            rx,
        )
        .await
        .map_err(|_| ProviderError::Timeout(format!("imsg rpc timeout ({})", method)))?
        .map_err(|_| ProviderError::Internal("Response channel closed".to_string()))?;

        serde_json::from_value(result)
            .map_err(|e| ProviderError::Internal(format!("Failed to deserialize response: {}", e)))
    }

    async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Close stdin to signal child to exit
            drop(self.stdin.take());

            // Wait a bit then kill if needed
            tokio::select! {
                _ = child.wait() => {}
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    let _ = child.kill().await;
                }
            }
        }
    }
}

// ============================================================================
// iMessage Provider
// ============================================================================

/// iMessage provider
pub struct IMessageProvider {
    config: EdithConfig,
    cli_path: String,
    db_path: Option<String>,
    connected: Arc<RwLock<bool>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<Option<mpsc::Receiver<InboundMessage>>>>,
    client: Arc<Mutex<Option<IMessageRpcClient>>>,
}

impl IMessageProvider {
    /// Create a new iMessage provider
    pub fn new(config: EdithConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Resolve CLI path from config
        let imessage_config = config.imessage.as_ref();
        let cli_path = imessage_config
            .and_then(|c| c.cli_path.clone())
            .unwrap_or_else(|| "imsg".to_string());
        let db_path = imessage_config.and_then(|c| c.db_path.clone());

        Self {
            config,
            cli_path,
            db_path,
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(Some(rx))),
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Get allow list from config
    fn get_allow_list(&self) -> Vec<String> {
        self.config
            .imessage
            .as_ref()
            .and_then(|c| c.allow_from.as_ref())
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
    fn is_sender_allowed(&self, sender: &str, chat_id: Option<i64>, chat_guid: Option<&str>, chat_identifier: Option<&str>) -> bool {
        let allow_list = self.get_allow_list();
        if allow_list.is_empty() || allow_list.contains(&"*".to_string()) {
            return true;
        }

        let sender_normalized = normalize_imessage_handle(sender);

        for entry in &allow_list {
            if entry.is_empty() {
                continue;
            }

            // Try to parse as a target
            if let Ok(target) = parse_imessage_target(entry) {
                match target {
                    IMessageTarget::ChatId { chat_id: allowed_id } => {
                        if let Some(id) = chat_id {
                            if allowed_id == id {
                                return true;
                            }
                        }
                    }
                    IMessageTarget::ChatGuid { chat_guid: allowed_guid } => {
                        if let Some(guid) = chat_guid {
                            if allowed_guid == guid {
                                return true;
                            }
                        }
                    }
                    IMessageTarget::ChatIdentifier { chat_identifier: allowed_ident } => {
                        if let Some(ident) = chat_identifier {
                            if allowed_ident == ident {
                                return true;
                            }
                        }
                    }
                    IMessageTarget::Handle { to, .. } => {
                        let handle_normalized = normalize_imessage_handle(&to);
                        if handle_normalized == sender_normalized {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Get mention patterns from config
    fn get_mention_patterns(&self) -> Vec<regex::Regex> {
        self.config
            .routing
            .as_ref()
            .and_then(|r| r.group_chat.as_ref())
            .and_then(|g| g.mention_patterns.as_ref())
            .map(|patterns| {
                patterns
                    .iter()
                    .filter_map(|p| regex::Regex::new(p).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if message text contains a mention
    fn is_mentioned(&self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        let patterns = self.get_mention_patterns();
        if patterns.is_empty() {
            return false;
        }

        // Clean text (remove zero-width chars)
        let cleaned: String = text
            .chars()
            .filter(|c| !matches!(*c as u32, 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f))
            .collect();
        let lower = cleaned.to_lowercase();

        patterns.iter().any(|re| re.is_match(&lower))
    }

    /// Check if group requires mention
    fn requires_mention(&self, chat_id: Option<i64>) -> bool {
        let imessage_config = self.config.imessage.as_ref();

        // Check specific group config
        if let Some(id) = chat_id {
            if let Some(groups) = imessage_config.and_then(|c| c.groups.as_ref()) {
                let id_str = id.to_string();
                if let Some(group) = groups.get(&id_str) {
                    if let Some(require) = group.require_mention {
                        return require;
                    }
                }
                // Check wildcard
                if let Some(default_group) = groups.get("*") {
                    if let Some(require) = default_group.require_mention {
                        return require;
                    }
                }
            }
        }

        // Default: require mention for groups
        true
    }

    /// Get text chunk limit from config
    fn text_chunk_limit(&self) -> usize {
        self.config
            .imessage
            .as_ref()
            .and_then(|c| c.text_chunk_limit)
            .unwrap_or(IMESSAGE_TEXT_LIMIT as u64) as usize
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

    /// Convert iMessage payload to InboundMessage
    fn convert_payload(&self, payload: IMessagePayload) -> Option<InboundMessage> {
        let sender = payload.sender.as_deref()?.trim();
        if sender.is_empty() {
            return None;
        }

        // Skip messages from self
        if payload.is_from_me.unwrap_or(false) {
            return None;
        }

        let chat_id = payload.chat_id;
        let is_group = payload.is_group.unwrap_or(false);

        // Groups require chat_id
        if is_group && chat_id.is_none() {
            return None;
        }

        // Check allow list
        if !self.is_sender_allowed(
            sender,
            chat_id,
            payload.chat_guid.as_deref(),
            payload.chat_identifier.as_deref(),
        ) {
            debug!(sender = sender, "Blocked unauthorized iMessage sender");
            return None;
        }

        let message_text = payload.text.as_deref().unwrap_or("").trim();

        // Check mention for groups
        let mentioned = if is_group {
            self.is_mentioned(message_text)
        } else {
            true
        };

        if is_group && self.requires_mention(chat_id) && !mentioned {
            debug!("imessage: skipping group message (no mention)");
            return None;
        }

        // Extract attachment info
        let include_attachments = self
            .config
            .imessage
            .as_ref()
            .and_then(|c| c.include_attachments)
            .unwrap_or(false);

        let attachments = if include_attachments {
            payload.attachments.unwrap_or_default()
        } else {
            vec![]
        };

        let first_attachment = attachments.iter().find(|a| {
            a.original_path.is_some() && !a.missing.unwrap_or(false)
        });

        let media_path = first_attachment.and_then(|a| a.original_path.clone());
        let media_type_str = first_attachment.and_then(|a| a.mime_type.clone());

        // Determine media placeholder
        let kind = media_type_str.as_ref().and_then(|m| {
            if m.starts_with("image/") {
                Some("image")
            } else if m.starts_with("video/") {
                Some("video")
            } else if m.starts_with("audio/") {
                Some("audio")
            } else {
                Some("attachment")
            }
        });

        let placeholder = match kind {
            Some(k) => format!("<media:{}>", k),
            None if !attachments.is_empty() => "<media:attachment>".to_string(),
            None => String::new(),
        };

        let body_text = if message_text.is_empty() {
            placeholder
        } else {
            message_text.to_string()
        };

        if body_text.is_empty() {
            return None;
        }

        // Build chat target
        let chat_target = format_chat_target(chat_id);
        let conversation_id = if is_group {
            chat_id.map(|id| id.to_string()).unwrap_or_default()
        } else {
            sender.to_string()
        };

        // Parse timestamp
        let timestamp = payload
            .created_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        // Build media attachment if present
        let media = first_attachment.map(|a| {
            let mime = a.mime_type.as_deref().unwrap_or("application/octet-stream");
            let media_type = if mime.starts_with("image/") {
                MediaType::Image
            } else if mime.starts_with("video/") {
                MediaType::Video
            } else if mime.starts_with("audio/") {
                MediaType::Audio
            } else {
                MediaType::Document
            };

            MediaAttachment {
                media_type,
                mime_type: mime.to_string(),
                url: media_path.clone().unwrap_or_default(),
                file_name: None,
                size: None,
                caption: None,
            }
        });

        Some(InboundMessage {
            id: payload.id.map(|id| id.to_string()),
            from: if is_group {
                format!("group:{}", chat_id.unwrap_or(0))
            } else {
                format!("imessage:{}", sender)
            },
            conversation_id: conversation_id.clone(),
            to: if chat_target.is_empty() {
                format!("imessage:{}", sender)
            } else {
                chat_target
            },
            body: body_text,
            sender_name: Some(sender.to_string()),
            timestamp: Some(timestamp),
            chat_type: if is_group {
                ChatType::Group
            } else {
                ChatType::Direct
            },
            chat_id: conversation_id,
            sender_jid: None,
            sender_e164: Some(sender.to_string()),
            reply_context: None,
            group_subject: payload.chat_name.clone(),
            group_participants: payload.participants.clone(),
            mentioned_ids: None,
            was_mentioned: Some(mentioned),
            media,
            surface: "imessage".to_string(),
            raw: None,
        })
    }
}

#[async_trait]
impl crate::MessagingProvider for IMessageProvider {
    fn name(&self) -> &'static str {
        "imessage"
    }

    async fn start(&self) -> ProviderResult<()> {
        info!("Starting iMessage provider");

        // Create notification channel
        let (notif_tx, mut notif_rx) = mpsc::channel::<JsonRpcResponse<serde_json::Value>>(100);

        // Create RPC client
        let mut client = IMessageRpcClient::new(
            &self.cli_path,
            self.db_path.as_deref(),
            notif_tx,
        )
        .await?;

        // Include attachments setting
        let include_attachments = self
            .config
            .imessage
            .as_ref()
            .and_then(|c| c.include_attachments)
            .unwrap_or(false);

        // Subscribe to messages
        let result: WatchSubscribeResult = client
            .request(
                "watch.subscribe",
                serde_json::json!({ "attachments": include_attachments }),
                10_000,
            )
            .await?;

        info!(
            subscription = ?result.subscription,
            "iMessage watch subscription started"
        );

        *self.connected.write().await = true;

        // Store client
        {
            let mut client_lock = self.client.lock().await;
            *client_lock = Some(client);
        }

        // Handle notifications
        let message_tx = self.message_tx.clone();
        let config = self.config.clone();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            let provider = IMessageProvider::new(config);

            while let Some(notification) = notif_rx.recv().await {
                if !*connected.read().await {
                    break;
                }

                if notification.method.as_deref() == Some("message") {
                    if let Some(params) = notification.params {
                        match serde_json::from_value::<MessageNotification>(params) {
                            Ok(msg_notif) => {
                                if let Some(payload) = msg_notif.message {
                                    if let Some(inbound) = provider.convert_payload(payload) {
                                        if message_tx.send(inbound).await.is_err() {
                                            warn!("Failed to send iMessage message");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to parse iMessage notification");
                            }
                        }
                    }
                } else if notification.method.as_deref() == Some("error") {
                    error!(params = ?notification.params, "iMessage watch error");
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> ProviderResult<()> {
        info!("Stopping iMessage provider");
        *self.connected.write().await = false;

        let mut client_lock = self.client.lock().await;
        if let Some(mut client) = client_lock.take() {
            client.stop().await;
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.try_read().map(|g| *g).unwrap_or(false)
    }

    async fn send(&self, message: OutboundMessage) -> ProviderResult<String> {
        let mut client_lock = self.client.lock().await;
        let client = client_lock
            .as_mut()
            .ok_or_else(|| ProviderError::NotAvailable("iMessage not connected".to_string()))?;

        // Parse target
        let target = parse_imessage_target(&message.to)?;

        // Get service and region from config
        let imessage_config = self.config.imessage.as_ref();
        let service = imessage_config
            .and_then(|c| c.service.as_ref())
            .map(|s| match s {
                edith_config::IMessageService::Imessage => "imessage",
                edith_config::IMessageService::Sms => "sms",
                edith_config::IMessageService::Auto => "auto",
            })
            .unwrap_or("auto");
        let region = imessage_config
            .and_then(|c| c.region.as_ref())
            .cloned()
            .unwrap_or_else(|| "US".to_string());

        // Send text messages
        if let Some(text) = message.text {
            let chunks = self.chunk_text(&text);

            for chunk in chunks {
                let mut params = serde_json::json!({
                    "text": chunk,
                    "service": service,
                    "region": region,
                });

                // Add target info
                match &target {
                    IMessageTarget::ChatId { chat_id } => {
                        params["chat_id"] = serde_json::json!(chat_id);
                    }
                    IMessageTarget::ChatGuid { chat_guid } => {
                        params["chat_guid"] = serde_json::json!(chat_guid);
                    }
                    IMessageTarget::ChatIdentifier { chat_identifier } => {
                        params["chat_identifier"] = serde_json::json!(chat_identifier);
                    }
                    IMessageTarget::Handle { to, service: svc } => {
                        params["to"] = serde_json::json!(to);
                        params["service"] = serde_json::json!(svc.to_string());
                    }
                }

                let _result: serde_json::Value = client.request("send", params, 30_000).await?;
            }
        }

        Ok("sent".to_string())
    }

    async fn send_reply(&self, chat_id: &str, reply: ReplyPayload) -> ProviderResult<String> {
        self.send(OutboundMessage {
            to: chat_id.to_string(),
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
        // iMessage doesn't support typing indicators via CLI
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
        assert_eq!(normalize_e164("+1234567890"), "+1234567890");
        assert_eq!(normalize_e164("1234567890"), "1234567890");
        assert_eq!(normalize_e164("+1 (234) 567-890"), "+1234567890");
    }

    #[test]
    fn test_normalize_handle() {
        assert_eq!(
            normalize_imessage_handle("imessage:+1234567890"),
            "+1234567890"
        );
        assert_eq!(
            normalize_imessage_handle("sms:+1234567890"),
            "+1234567890"
        );
        assert_eq!(
            normalize_imessage_handle("Test@Example.com"),
            "test@example.com"
        );
    }

    #[test]
    fn test_parse_target_handle() {
        let target = parse_imessage_target("+1234567890").unwrap();
        match target {
            IMessageTarget::Handle { to, service } => {
                assert_eq!(to, "+1234567890");
                assert_eq!(service, IMessageService::Auto);
            }
            _ => panic!("Expected Handle target"),
        }
    }

    #[test]
    fn test_parse_target_chat_id() {
        let target = parse_imessage_target("chat_id:123").unwrap();
        match target {
            IMessageTarget::ChatId { chat_id } => {
                assert_eq!(chat_id, 123);
            }
            _ => panic!("Expected ChatId target"),
        }
    }

    #[test]
    fn test_parse_target_chat_guid() {
        let target = parse_imessage_target("chat_guid:iMessage;-;+1234567890").unwrap();
        match target {
            IMessageTarget::ChatGuid { chat_guid } => {
                assert_eq!(chat_guid, "iMessage;-;+1234567890");
            }
            _ => panic!("Expected ChatGuid target"),
        }
    }

    #[test]
    fn test_parse_target_with_service() {
        let target = parse_imessage_target("imessage:+1234567890").unwrap();
        match target {
            IMessageTarget::Handle { to, service } => {
                assert_eq!(to, "+1234567890");
                assert_eq!(service, IMessageService::IMessage);
            }
            _ => panic!("Expected Handle target"),
        }
    }

    #[test]
    fn test_format_chat_target() {
        assert_eq!(format_chat_target(Some(123)), "chat_id:123");
        assert_eq!(format_chat_target(None), "");
        assert_eq!(format_chat_target(Some(0)), "");
    }

    #[test]
    fn test_chunk_text() {
        let config = EdithConfig::default();
        let provider = IMessageProvider::new(config);

        let text = "Hello World";
        let chunks = provider.chunk_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }
}
