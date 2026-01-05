//! Edith Messaging Providers
//!
//! This crate provides integrations with various messaging platforms:
//! - Telegram (via teloxide)
//! - WhatsApp (via Baileys-like protocol - bridge mode)
//! - Discord (via serenity)
//! - Signal (via signal-cli JSON-RPC daemon)
//! - iMessage (macOS only, via imsg CLI JSON-RPC)

pub mod discord;
pub mod error;
pub mod imessage;
pub mod message;
pub mod signal;
pub mod telegram;
pub mod traits;
pub mod whatsapp;

pub use discord::DiscordProvider;
pub use error::*;
pub use imessage::IMessageProvider;
pub use message::*;
pub use signal::SignalProvider;
pub use telegram::TelegramProvider;
pub use traits::*;
pub use whatsapp::WhatsAppProvider;
