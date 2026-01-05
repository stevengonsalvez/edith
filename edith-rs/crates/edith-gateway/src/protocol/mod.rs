//! Gateway protocol handling
//!
//! Implements the Clawdis-compatible WebSocket protocol with:
//! - Connect handshake (ConnectParams → HelloOk)
//! - Request/Response/Event frames
//! - Method dispatch for 51 RPC methods
//! - Idempotency deduplication

mod connection;
mod handler;
mod methods;

pub use connection::*;
pub use handler::*;
pub use methods::*;
