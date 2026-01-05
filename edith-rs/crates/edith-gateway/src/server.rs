//! Gateway server startup and lifecycle

use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

use edith_config::{resolve_gateway_port, EdithConfig};

use crate::dispatch::EventReceivers;
use crate::{create_router, GatewayError, GatewayResult, GatewayState};

/// Gateway server configuration
#[derive(Debug, Clone, Copy)]
pub struct ServerConfig {
    /// Port to listen on
    pub port: u16,
    /// Bind address
    pub bind_addr: SocketAddr,
}

impl ServerConfig {
    /// Create server config from Edith config
    pub fn from_config(config: &EdithConfig) -> Self {
        let port = resolve_gateway_port(config);
        let bind_mode = config
            .gateway
            .as_ref()
            .and_then(|g| g.bind.as_ref())
            .map(|b| format!("{:?}", b).to_lowercase())
            .unwrap_or_else(|| "loopback".to_string());

        let bind_addr = match bind_mode.as_str() {
            "loopback" => SocketAddr::from(([127, 0, 0, 1], port)),
            "lan" => SocketAddr::from(([0, 0, 0, 0], port)),
            _ => SocketAddr::from(([127, 0, 0, 1], port)),
        };

        Self { port, bind_addr }
    }
}

/// Gateway server handle
#[derive(Clone)]
pub struct GatewayServer {
    state: GatewayState,
    config: ServerConfig,
}

/// Result of creating a gateway server, includes event receivers
pub struct GatewayServerInit {
    /// The gateway server
    pub server: GatewayServer,
    /// Event receivers for processing dispatched events
    pub receivers: EventReceivers,
}

impl GatewayServer {
    /// Create a new gateway server with event receivers
    pub fn new(edith_config: EdithConfig) -> GatewayServerInit {
        let server_config = ServerConfig::from_config(&edith_config);
        let (state, receivers) = GatewayState::new(edith_config);

        let server = Self {
            state,
            config: server_config,
        };

        GatewayServerInit { server, receivers }
    }

    /// Create a new gateway server without event receivers (for testing)
    pub fn new_without_receivers(edith_config: EdithConfig) -> Self {
        let server_config = ServerConfig::from_config(&edith_config);
        let state = GatewayState::new_without_receivers(edith_config);

        Self {
            state,
            config: server_config,
        }
    }

    /// Get the gateway state
    pub fn state(&self) -> &GatewayState {
        &self.state
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> SocketAddr {
        self.config.bind_addr
    }

    /// Run the gateway server
    pub async fn run(self) -> GatewayResult<()> {
        let router = create_router(self.state.clone());

        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .map_err(|e| GatewayError::Internal(format!("failed to bind: {}", e)))?;

        info!(
            addr = %self.config.bind_addr,
            "Gateway server listening"
        );

        // Get shutdown signal
        let mut shutdown_rx = self.state.subscribe_shutdown();

        // Run server with graceful shutdown
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
                info!("Gateway server shutting down");
            })
            .await
            .map_err(|e| GatewayError::Internal(format!("server error: {}", e)))?;

        Ok(())
    }

    /// Run the gateway server with a shutdown signal
    pub async fn run_with_shutdown<F>(self, shutdown: F) -> GatewayResult<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let router = create_router(self.state.clone());

        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .map_err(|e| GatewayError::Internal(format!("failed to bind: {}", e)))?;

        info!(
            addr = %self.config.bind_addr,
            "Gateway server listening"
        );

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|e| GatewayError::Internal(format!("server error: {}", e)))?;

        Ok(())
    }

    /// Shutdown the gateway server
    pub async fn shutdown(&self) {
        info!("Signaling gateway shutdown");
        self.state.shutdown();
    }
}

/// Start the gateway server
pub async fn start_server(config: EdithConfig) -> GatewayResult<()> {
    let init = GatewayServer::new(config);
    // Note: In a full implementation, receivers would be passed to an event processor
    // For now, we drop them (events will be lost)
    let _receivers = init.receivers;
    init.server.run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = EdithConfig::default();
        let server_config = ServerConfig::from_config(&config);

        assert_eq!(server_config.port, edith_config::DEFAULT_GATEWAY_PORT);
        assert_eq!(server_config.bind_addr.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn test_server_config_custom_port() {
        let config = edith_config::parse_config(r#"{ "gateway": { "port": 9999 } }"#).unwrap();
        let server_config = ServerConfig::from_config(&config);

        assert_eq!(server_config.port, 9999);
    }
}
