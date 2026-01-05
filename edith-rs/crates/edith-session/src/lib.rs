//! Edith Session Management
//!
//! This crate handles session state, persistence, and lifecycle management
//! for conversations across different messaging surfaces.

mod error;
mod manager;
mod routing;
mod session;
mod store;

pub use error::*;
pub use manager::{ConversationMessage, MessageRole, Session, SessionManager};
pub use routing::*;
pub use session::*;
pub use store::*;
