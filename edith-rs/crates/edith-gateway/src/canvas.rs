//! Canvas host server with live-reload support
//!
//! This module provides a static file server for the Canvas UI with:
//! - WebSocket-based live-reload on file changes
//! - Script injection for reload functionality
//! - Security-conscious file path resolution

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, Request, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

pub const CANVAS_HOST_PATH: &str = "/__edith__/canvas";
pub const CANVAS_WS_PATH: &str = "/__edith/ws";

/// Canvas host configuration
#[derive(Debug, Clone)]
pub struct CanvasHostConfig {
    pub root_dir: PathBuf,
    pub base_path: String,
    pub enabled: bool,
}

impl Default for CanvasHostConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            root_dir: home.join("edith").join("canvas"),
            base_path: CANVAS_HOST_PATH.to_string(),
            enabled: true,
        }
    }
}

/// Shared state for the Canvas host
pub struct CanvasHostState {
    config: CanvasHostConfig,
    reload_tx: broadcast::Sender<()>,
    connected_clients: Arc<RwLock<HashSet<u64>>>,
    next_client_id: Arc<RwLock<u64>>,
    #[allow(dead_code)]
    watcher: Option<RecommendedWatcher>,
}

impl CanvasHostState {
    /// Create a new Canvas host state
    pub async fn new(config: CanvasHostConfig) -> Result<Self, std::io::Error> {
        // Ensure root directory exists
        if !config.root_dir.exists() {
            fs::create_dir_all(&config.root_dir).await?;
            // Create default index.html
            let index_path = config.root_dir.join("index.html");
            fs::write(&index_path, default_index_html()).await?;
        }

        let (reload_tx, _) = broadcast::channel(16);
        let reload_tx_clone = reload_tx.clone();

        // Set up file watcher
        let root_dir_clone = config.root_dir.clone();
        let watcher = match notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        debug!("Canvas file change detected: {:?}", event.paths);
                        let _ = reload_tx_clone.send(());
                    }
                }
                Err(e) => {
                    warn!("Canvas file watch error: {}", e);
                }
            },
        ) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&root_dir_clone, RecursiveMode::Recursive) {
                    warn!("Failed to watch canvas directory: {}", e);
                } else {
                    info!(root_dir = %root_dir_clone.display(), "Canvas file watcher started");
                }
                Some(watcher)
            }
            Err(e) => {
                warn!("Failed to create canvas file watcher: {}", e);
                None
            }
        };

        Ok(Self {
            config,
            reload_tx,
            connected_clients: Arc::new(RwLock::new(HashSet::new())),
            next_client_id: Arc::new(RwLock::new(1)),
            watcher,
        })
    }

    /// Get the number of connected WebSocket clients
    pub async fn connected_count(&self) -> usize {
        self.connected_clients.read().await.len()
    }

    /// Trigger a reload for all connected clients
    pub fn trigger_reload(&self) {
        let _ = self.reload_tx.send(());
    }
}

/// Create the Canvas host router
pub fn canvas_router(state: Arc<CanvasHostState>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/", get(index_handler))
        .route("/*path", get(file_handler))
        .with_state(state)
}

/// WebSocket handler for live-reload
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<CanvasHostState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<CanvasHostState>) {
    // Assign client ID
    let client_id = {
        let mut id = state.next_client_id.write().await;
        let current = *id;
        *id += 1;
        current
    };

    // Register client
    {
        let mut clients = state.connected_clients.write().await;
        clients.insert(client_id);
    }
    debug!(client_id = client_id, "Canvas WebSocket client connected");

    // Subscribe to reload broadcasts
    let mut reload_rx = state.reload_tx.subscribe();

    loop {
        tokio::select! {
            // Handle incoming messages
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            // Handle reload broadcasts
            _ = reload_rx.recv() => {
                if socket.send(Message::Text("reload".to_string().into())).await.is_err() {
                    break;
                }
                debug!(client_id = client_id, "Sent reload to Canvas client");
            }
        }
    }

    // Unregister client
    {
        let mut clients = state.connected_clients.write().await;
        clients.remove(&client_id);
    }
    debug!(client_id = client_id, "Canvas WebSocket client disconnected");
}

/// Index handler - serves index.html
async fn index_handler(State(state): State<Arc<CanvasHostState>>) -> impl IntoResponse {
    let index_path = state.config.root_dir.join("index.html");
    serve_file(&index_path, true).await
}

/// File handler - serves static files
async fn file_handler(
    State(state): State<Arc<CanvasHostState>>,
    req: Request<Body>,
) -> impl IntoResponse {
    let path = req.uri().path();
    let file_path = resolve_file_path(&state.config.root_dir, path).await;

    match file_path {
        Some(path) => serve_file(&path, path.extension().map_or(false, |e| e == "html")).await,
        None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

/// Resolve a URL path to a file path, ensuring security
async fn resolve_file_path(root: &Path, url_path: &str) -> Option<PathBuf> {
    // Decode and normalize the path
    let decoded = urlencoding::decode(url_path).ok()?;
    let normalized = decoded.trim_start_matches('/');

    // Check for path traversal attempts
    if normalized.split('/').any(|p| p == "..") {
        return None;
    }

    let mut candidate = root.join(normalized);

    // Check if it's a directory, and look for index.html
    if candidate.is_dir() {
        candidate = candidate.join("index.html");
    }

    // Verify the resolved path is within the root
    let canonical = fs::canonicalize(&candidate).await.ok()?;
    let root_canonical = fs::canonicalize(root).await.ok()?;

    if !canonical.starts_with(&root_canonical) {
        return None;
    }

    // Check if file exists and is not a symlink
    let metadata = fs::symlink_metadata(&candidate).await.ok()?;
    if metadata.file_type().is_symlink() {
        return None;
    }

    if metadata.is_file() {
        Some(candidate)
    } else {
        None
    }
}

/// Serve a file with appropriate content type
async fn serve_file(path: &Path, inject_reload: bool) -> Response<Body> {
    let content = match fs::read(path).await {
        Ok(c) => c,
        Err(e) => {
            error!(path = %path.display(), error = %e, "Failed to read file");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading file").into_response();
        }
    };

    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    // Inject live-reload script into HTML files
    let content = if inject_reload && mime_type.starts_with("text/html") {
        let html = String::from_utf8_lossy(&content);
        inject_live_reload(&html).into_bytes()
    } else {
        content
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(content))
        .unwrap()
}

/// Inject live-reload script into HTML
fn inject_live_reload(html: &str) -> String {
    let snippet = format!(
        r#"
<script>
(() => {{
  // Edith Canvas live-reload
  const actionHandlerName = "edithCanvasAction";

  function postToNode(payload) {{
    try {{
      const raw = typeof payload === "string" ? payload : JSON.stringify(payload);
      const iosHandler = globalThis.webkit?.messageHandlers?.[actionHandlerName];
      if (iosHandler && typeof iosHandler.postMessage === "function") {{
        iosHandler.postMessage(raw);
        return true;
      }}
      const androidHandler = globalThis[actionHandlerName];
      if (androidHandler && typeof androidHandler.postMessage === "function") {{
        androidHandler.postMessage(raw);
        return true;
      }}
    }} catch {{}}
    return false;
  }}

  function sendUserAction(userAction) {{
    const id = (userAction && typeof userAction.id === "string" && userAction.id.trim()) ||
      (globalThis.crypto?.randomUUID?.() ?? String(Date.now()));
    const action = {{ ...userAction, id }};
    return postToNode({{ userAction: action }});
  }}

  globalThis.Edith = globalThis.Edith ?? {{}};
  globalThis.Edith.postMessage = postToNode;
  globalThis.Edith.sendUserAction = sendUserAction;
  globalThis.edithPostMessage = postToNode;
  globalThis.edithSendUserAction = sendUserAction;

  try {{
    const proto = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(proto + "://" + location.host + "{CANVAS_WS_PATH}");
    ws.onmessage = (ev) => {{
      if (String(ev.data || "") === "reload") location.reload();
    }};
    ws.onerror = (e) => console.warn("Edith live-reload WebSocket error:", e);
  }} catch (e) {{
    console.warn("Edith live-reload WebSocket failed:", e);
  }}
}})();
</script>
"#
    );

    // Find </body> and inject before it
    let lower = html.to_lowercase();
    if let Some(idx) = lower.rfind("</body>") {
        format!("{}\n{}\n{}", &html[..idx], snippet.trim(), &html[idx..])
    } else {
        format!("{}\n{}\n", html, snippet.trim())
    }
}

/// Default index.html content
fn default_index_html() -> &'static str {
    r#"<!doctype html>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Edith Canvas</title>
<style>
  html, body { height: 100%; margin: 0; background: #000; color: #fff; font: 16px/1.4 -apple-system, BlinkMacSystemFont, system-ui, Segoe UI, Roboto, Helvetica, Arial, sans-serif; }
  .wrap { min-height: 100%; display: grid; place-items: center; padding: 24px; }
  .card { width: min(720px, 100%); background: rgba(255,255,255,0.06); border: 1px solid rgba(255,255,255,0.10); border-radius: 16px; padding: 18px 18px 14px; }
  .title { display: flex; align-items: baseline; gap: 10px; }
  h1 { margin: 0; font-size: 22px; letter-spacing: 0.2px; }
  .sub { opacity: 0.75; font-size: 13px; }
  .row { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 14px; }
  button { appearance: none; border: 1px solid rgba(255,255,255,0.14); background: rgba(255,255,255,0.10); color: #fff; padding: 10px 12px; border-radius: 12px; font-weight: 600; cursor: pointer; }
  button:active { transform: translateY(1px); }
  .ok { color: #24e08a; }
  .bad { color: #ff5c5c; }
  .log { margin-top: 14px; opacity: 0.85; font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace; white-space: pre-wrap; background: rgba(0,0,0,0.35); border: 1px solid rgba(255,255,255,0.08); padding: 10px; border-radius: 12px; }
</style>
<div class="wrap">
  <div class="card">
    <div class="title">
      <h1>Edith Canvas</h1>
      <div class="sub">Interactive canvas (auto-reload enabled)</div>
    </div>

    <div class="row">
      <button id="btn-hello">Hello</button>
      <button id="btn-time">Time</button>
      <button id="btn-photo">Photo</button>
    </div>

    <div id="status" class="sub" style="margin-top: 10px;"></div>
    <div id="log" class="log">Ready.</div>
  </div>
</div>
<script>
(() => {
  const logEl = document.getElementById("log");
  const statusEl = document.getElementById("status");
  const log = (msg) => { logEl.textContent = String(msg); };

  const hasIOS = () => !!(window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.edithCanvasAction);
  const hasAndroid = () => !!(window.edithCanvasAction && typeof window.edithCanvasAction.postMessage === "function");
  const hasHelper = () => typeof window.edithSendUserAction === "function";
  statusEl.innerHTML =
    "Bridge: " +
    (hasHelper() ? "<span class='ok'>ready</span>" : "<span class='bad'>missing</span>") +
    " · iOS=" + (hasIOS() ? "yes" : "no") +
    " · Android=" + (hasAndroid() ? "yes" : "no");

  window.addEventListener("edith:action-status", (ev) => {
    const d = ev && ev.detail || {};
    log("Action status: id=" + (d.id || "?") + " ok=" + String(!!d.ok) + (d.error ? (" error=" + d.error) : ""));
  });

  function send(name, sourceComponentId) {
    if (!hasHelper()) {
      log("No action bridge found. Ensure you're viewing this on an iOS/Android Edith node canvas.");
      return;
    }
    const ok = window.edithSendUserAction({
      name,
      surfaceId: "main",
      sourceComponentId,
      context: { t: Date.now() },
    });
    log(ok ? ("Sent action: " + name) : ("Failed to send action: " + name));
  }

  document.getElementById("btn-hello").onclick = () => send("hello", "demo.hello");
  document.getElementById("btn-time").onclick = () => send("time", "demo.time");
  document.getElementById("btn-photo").onclick = () => send("photo", "demo.photo");
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_live_reload() {
        let html = "<html><head></head><body><h1>Hello</h1></body></html>";
        let injected = inject_live_reload(html);
        assert!(injected.contains("Edith Canvas live-reload"));
        assert!(injected.contains("WebSocket"));
    }

    #[test]
    fn test_inject_live_reload_no_body() {
        let html = "<html><head></head><h1>Hello</h1></html>";
        let injected = inject_live_reload(html);
        assert!(injected.contains("Edith Canvas live-reload"));
    }

    #[test]
    fn test_default_config() {
        let config = CanvasHostConfig::default();
        assert!(config.root_dir.to_string_lossy().contains("edith"));
        assert!(config.enabled);
    }
}
