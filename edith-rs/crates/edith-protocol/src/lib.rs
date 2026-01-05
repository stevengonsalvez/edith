//! Edith Gateway Protocol
//!
//! This crate defines the typed WebSocket protocol for the Edith gateway,
//! including request/response frames, events, and all RPC method definitions.
//!
//! The protocol mirrors the Clawdis TypeScript implementation for full compatibility.

pub mod constants;
pub mod error;
pub mod frames;
pub mod handshake;
pub mod dedupe;

// Method parameter types (from methods.rs)
mod method_params;
pub use method_params::*;

// Event payload types (from events.rs)
mod event_payloads;
pub use event_payloads::*;

// Re-export everything from constants (top-level consts)
pub use constants::*;
// Re-export the methods and events modules for direct access (method::HEALTH, events::TICK)
pub use constants::methods;
pub use constants::events;

pub use error::*;
pub use frames::*;
pub use handshake::*;
pub use dedupe::*;
