//! Configuration loading and management

use crate::{error::ConfigResult, schema::EdithConfig, ConfigError};
use once_cell::sync::Lazy;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::RwLock,
};
use tracing::{debug, info, warn};

/// Default gateway port
pub const DEFAULT_GATEWAY_PORT: u16 = 18789;

/// Environment variable for state directory override
const ENV_STATE_DIR: &str = "EDITH_STATE_DIR";

/// Environment variable for config path override
const ENV_CONFIG_PATH: &str = "EDITH_CONFIG_PATH";

/// Environment variable for Nix mode
const ENV_NIX_MODE: &str = "EDITH_NIX_MODE";

/// Global configuration cache
static CONFIG_CACHE: Lazy<RwLock<Option<EdithConfig>>> = Lazy::new(|| RwLock::new(None));

/// Check if running in Nix mode
pub fn is_nix_mode() -> bool {
    std::env::var(ENV_NIX_MODE).map_or(false, |v| v == "1")
}

/// Get the state directory for Edith data
pub fn state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(ENV_STATE_DIR) {
        return PathBuf::from(dir);
    }

    // Default to ~/.edith
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".edith"))
        .unwrap_or_else(|_| PathBuf::from(".edith"))
}

/// Get the configuration file path
pub fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_CONFIG_PATH) {
        return PathBuf::from(path);
    }

    state_dir().join("edith.json")
}

/// Get the sessions directory
pub fn sessions_dir() -> PathBuf {
    state_dir().join("sessions")
}

/// Get the credentials directory
pub fn credentials_dir() -> PathBuf {
    state_dir().join("credentials")
}

/// Load configuration from the default path
pub fn load_config() -> ConfigResult<EdithConfig> {
    load_config_from_path(&config_path())
}

/// Load configuration from a specific path
pub fn load_config_from_path(path: &Path) -> ConfigResult<EdithConfig> {
    if !path.exists() {
        debug!("Config file not found at {:?}, using defaults", path);
        return Ok(EdithConfig::default());
    }

    let raw = fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
        path: path.display().to_string(),
        source: e,
    })?;

    parse_config(&raw)
}

/// Parse configuration from a JSON5 string
pub fn parse_config(raw: &str) -> ConfigResult<EdithConfig> {
    let config: EdithConfig = json5::from_str(raw)?;
    Ok(apply_defaults(config))
}

/// Apply default values to configuration
fn apply_defaults(mut config: EdithConfig) -> EdithConfig {
    // Apply identity-based defaults for mention patterns
    if let Some(ref identity) = config.identity {
        if let Some(ref name) = identity.name {
            let name = name.trim();
            if !name.is_empty() {
                if config.routing.is_none() {
                    config.routing = Some(crate::schema::RoutingConfig::default());
                }
                if let Some(ref mut routing) = config.routing {
                    if routing.group_chat.is_none() {
                        routing.group_chat = Some(crate::schema::GroupChatConfig::default());
                    }
                    if let Some(ref mut group_chat) = routing.group_chat {
                        if group_chat.mention_patterns.is_none() {
                            // Create mention pattern from name
                            let escaped = regex::escape(name);
                            let pattern = format!(r"\b@?{}\b", escaped.replace(r"\ ", r"\s+"));
                            group_chat.mention_patterns = Some(vec![pattern]);
                        }
                    }
                }
            }
        }
    }

    // Force main session key
    if let Some(ref mut session) = config.session {
        if session.main_key.is_some() {
            let key = session.main_key.as_ref().unwrap().trim();
            if !key.is_empty() && key != "main" {
                warn!("session.mainKey is ignored; main session is always 'main'");
            }
            session.main_key = Some("main".to_string());
        }
    }

    config
}

/// Save configuration to the default path
pub fn save_config(config: &EdithConfig) -> ConfigResult<()> {
    save_config_to_path(config, &config_path())
}

/// Save configuration to a specific path
pub fn save_config_to_path(config: &EdithConfig, path: &Path) -> ConfigResult<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ConfigError::CreateDir {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, format!("{}\n", json)).map_err(|e| ConfigError::WriteFile {
        path: path.display().to_string(),
        source: e,
    })?;

    info!("Configuration saved to {:?}", path);
    Ok(())
}

/// Get cached configuration or load from file
pub fn get_config() -> ConfigResult<EdithConfig> {
    // Try to read from cache first
    {
        let cache = CONFIG_CACHE.read().unwrap();
        if let Some(ref config) = *cache {
            return Ok(config.clone());
        }
    }

    // Load from file and cache
    let config = load_config()?;
    {
        let mut cache = CONFIG_CACHE.write().unwrap();
        *cache = Some(config.clone());
    }

    Ok(config)
}

/// Invalidate the configuration cache
pub fn invalidate_config_cache() {
    let mut cache = CONFIG_CACHE.write().unwrap();
    *cache = None;
    debug!("Configuration cache invalidated");
}

/// Reload configuration from file
pub fn reload_config() -> ConfigResult<EdithConfig> {
    invalidate_config_cache();
    get_config()
}

/// Resolve the gateway port from config and environment
pub fn resolve_gateway_port(config: &EdithConfig) -> u16 {
    // Check environment variable first
    if let Ok(port_str) = std::env::var("EDITH_GATEWAY_PORT") {
        if let Ok(port) = port_str.trim().parse::<u16>() {
            if port > 0 {
                return port;
            }
        }
    }

    // Then check config
    if let Some(ref gateway) = config.gateway {
        if let Some(port) = gateway.port {
            if port > 0 {
                return port;
            }
        }
    }

    DEFAULT_GATEWAY_PORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_config() {
        let config = parse_config("{}").unwrap();
        assert!(config.identity.is_none());
        assert!(config.gateway.is_none());
    }

    #[test]
    fn test_parse_basic_config() {
        let raw = r#"{
            "identity": {
                "name": "TestBot",
                "emoji": "🤖"
            },
            "gateway": {
                "port": 8080
            }
        }"#;

        let config = parse_config(raw).unwrap();
        assert_eq!(
            config.identity.as_ref().unwrap().name.as_ref().unwrap(),
            "TestBot"
        );
        assert_eq!(config.gateway.as_ref().unwrap().port, Some(8080));
    }

    #[test]
    fn test_resolve_gateway_port_default() {
        let config = EdithConfig::default();
        assert_eq!(resolve_gateway_port(&config), DEFAULT_GATEWAY_PORT);
    }

    #[test]
    fn test_resolve_gateway_port_from_config() {
        let config = parse_config(r#"{ "gateway": { "port": 9999 } }"#).unwrap();
        assert_eq!(resolve_gateway_port(&config), 9999);
    }
}
