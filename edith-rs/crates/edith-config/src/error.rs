//! Configuration error types

use thiserror::Error;

/// Errors that can occur during configuration operations
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read configuration file
    #[error("failed to read config file at {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse JSON5 configuration
    #[error("failed to parse config: {0}")]
    ParseJson5(#[from] json5::Error),

    /// Failed to parse JSON configuration
    #[error("failed to parse JSON config: {0}")]
    ParseJson(#[from] serde_json::Error),

    /// Configuration validation failed
    #[error("config validation failed: {0}")]
    Validation(String),

    /// Missing required configuration field
    #[error("missing required config field: {field}")]
    MissingField { field: String },

    /// Invalid configuration value
    #[error("invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    /// Environment variable error
    #[error("environment variable error for {name}: {message}")]
    EnvVar { name: String, message: String },

    /// Failed to write configuration file
    #[error("failed to write config file at {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Failed to create config directory
    #[error("failed to create config directory at {path}: {source}")]
    CreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Legacy configuration detected
    #[error("legacy config detected at {path}: {message}. Run migration to update.")]
    LegacyConfig { path: String, message: String },

    /// Configuration file not found (not an error in all contexts)
    #[error("config file not found at {path}")]
    NotFound { path: String },
}

impl ConfigError {
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    pub fn invalid_value(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidValue {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}

pub type ConfigResult<T> = Result<T, ConfigError>;
