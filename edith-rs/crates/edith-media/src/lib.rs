//! Edith Media Handling
//!
//! This crate provides media processing capabilities:
//! - Audio transcription via external commands
//! - Media type detection
//! - Media storage and retrieval
//! - File format conversions

mod error;
mod transcription;
mod types;

pub use error::*;
pub use transcription::*;
pub use types::*;
