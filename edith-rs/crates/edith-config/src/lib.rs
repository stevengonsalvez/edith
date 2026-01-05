//! Edith Configuration Management
//!
//! This crate handles all configuration loading, validation, and management
//! for the Edith gateway. It supports JSON5 configuration files and
//! environment variable overrides.

mod config;
mod defaults;
mod error;
mod providers;
pub mod sandbox;
mod schema;
pub mod skills;
mod validation;

pub use config::*;
pub use error::*;
pub use providers::*;
pub use sandbox::*;
pub use schema::*;
pub use skills::*;
