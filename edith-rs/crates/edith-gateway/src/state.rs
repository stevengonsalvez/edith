//! Gateway shared state

use edith_config::EdithConfig;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::dispatch::EventDispatcher;

/// Shared gateway state
#[derive(Clone)]
pub struct GatewayState {
    inner: Arc<GatewayStateInner>,
}

struct GatewayStateInner {
    /// Current configuration
    config: RwLock<EdithConfig>,

    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<()>,

    /// Connected WebSocket clients
    connections: RwLock<Vec<ConnectionInfo>>,

    /// Hook token for authentication
    hook_token: RwLock<Option<String>>,

    /// Gateway auth token
    auth_token: RwLock<Option<String>>,

    /// Event dispatcher for hooks and agent runs
    dispatcher: EventDispatcher,
}

/// Information about a connected WebSocket client
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub remote_addr: Option<String>,
    pub user_agent: Option<String>,
}

use crate::dispatch::EventReceivers;

impl GatewayState {
    /// Create a new gateway state with event receivers
    pub fn new(config: EdithConfig) -> (Self, EventReceivers) {
        let (shutdown_tx, _) = broadcast::channel(1);

        let hook_token = config
            .hooks
            .as_ref()
            .and_then(|h| h.token.clone());

        let auth_token = config
            .gateway
            .as_ref()
            .and_then(|g| g.auth.as_ref())
            .and_then(|a| a.token.clone());

        // Create event dispatcher with buffer of 100 events
        let (dispatcher, receivers) = EventDispatcher::new(100);

        let state = Self {
            inner: Arc::new(GatewayStateInner {
                config: RwLock::new(config),
                shutdown_tx,
                connections: RwLock::new(Vec::new()),
                hook_token: RwLock::new(hook_token),
                auth_token: RwLock::new(auth_token),
                dispatcher,
            }),
        };

        (state, receivers)
    }

    /// Get the event dispatcher
    pub fn dispatcher(&self) -> &EventDispatcher {
        &self.inner.dispatcher
    }

    /// Get the current configuration
    pub fn config(&self) -> EdithConfig {
        self.inner.config.read().clone()
    }

    /// Update the configuration
    pub fn update_config(&self, config: EdithConfig) {
        let hook_token = config
            .hooks
            .as_ref()
            .and_then(|h| h.token.clone());

        let auth_token = config
            .gateway
            .as_ref()
            .and_then(|g| g.auth.as_ref())
            .and_then(|a| a.token.clone());

        *self.inner.config.write() = config;
        *self.inner.hook_token.write() = hook_token;
        *self.inner.auth_token.write() = auth_token;
    }

    /// Subscribe to shutdown signals
    pub fn subscribe_shutdown(&self) -> broadcast::Receiver<()> {
        self.inner.shutdown_tx.subscribe()
    }

    /// Signal shutdown
    pub fn shutdown(&self) {
        let _ = self.inner.shutdown_tx.send(());
    }

    /// Register a new connection
    pub fn register_connection(&self, info: ConnectionInfo) {
        self.inner.connections.write().push(info);
    }

    /// Remove a connection
    pub fn remove_connection(&self, id: &str) {
        self.inner.connections.write().retain(|c| c.id != id);
    }

    /// Get all connections
    pub fn connections(&self) -> Vec<ConnectionInfo> {
        self.inner.connections.read().clone()
    }

    /// Check if a hook token is valid
    pub fn check_hook_token(&self, token: &str) -> bool {
        self.inner
            .hook_token
            .read()
            .as_ref()
            .map_or(false, |t| t == token)
    }

    /// Check if an auth token is valid
    pub fn check_auth_token(&self, token: &str) -> bool {
        self.inner
            .auth_token
            .read()
            .as_ref()
            .map_or(false, |t| t == token)
    }

    /// Get the hooks base path
    pub fn hooks_base_path(&self) -> String {
        self.inner
            .config
            .read()
            .hooks
            .as_ref()
            .and_then(|h| h.path.clone())
            .unwrap_or_else(|| "/hooks".to_string())
    }

    /// Check if hooks are enabled
    pub fn hooks_enabled(&self) -> bool {
        self.inner
            .config
            .read()
            .hooks
            .as_ref()
            .and_then(|h| h.enabled)
            .unwrap_or(false)
    }

    /// Get max body bytes for hooks
    pub fn hooks_max_body_bytes(&self) -> usize {
        self.inner
            .config
            .read()
            .hooks
            .as_ref()
            .and_then(|h| h.max_body_bytes)
            .unwrap_or(1024 * 1024) as usize // 1MB default
    }
}

impl GatewayState {
    /// Create a new gateway state without returning receivers (for testing)
    pub fn new_without_receivers(config: EdithConfig) -> Self {
        let (state, _receivers) = Self::new(config);
        state
    }
}

impl Default for GatewayState {
    fn default() -> Self {
        Self::new_without_receivers(EdithConfig::default())
    }
}
