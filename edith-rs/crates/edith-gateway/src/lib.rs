//! Edith Gateway Server
//!
//! This crate implements the HTTP/WebSocket gateway that handles:
//! - WebSocket connections for real-time communication
//! - HTTP hooks for external integrations
//! - Control UI serving
//! - Canvas host with live-reload
//! - Browser automation via CDP
//! - Authentication and authorization
//!
//! ## Protocol
//!
//! The gateway uses the Clawdis-compatible WebSocket protocol with:
//! - Connect handshake (ConnectParams → HelloOk/HelloError)
//! - Request/Response/Event frame processing
//! - 51 RPC methods for all gateway operations
//! - Idempotency deduplication for side-effecting requests

mod auth;
pub mod browser;
pub mod canvas;
mod dispatch;
mod error;
mod hooks;
pub mod protocol;
mod router;
mod server;
mod state;
mod websocket;

pub use browser::*;
pub use canvas::*;
pub use dispatch::*;
pub use error::*;
pub use router::*;
pub use server::*;
pub use state::*;
pub use websocket::{ws_handler, ProtocolState, WebSocketState, LegacyWsMessage};
