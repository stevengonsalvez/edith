//! Configuration validation

use crate::{error::ConfigResult, schema::EdithConfig, ConfigError};

/// Validation result with collected issues
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn add_error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            path: path.into(),
            message: message.into(),
            severity: Severity::Error,
        });
    }

    pub fn add_warning(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(ValidationIssue {
            path: path.into(),
            message: message.into(),
            severity: Severity::Warning,
        });
    }
}

/// A single validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
    pub severity: Severity,
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.path, self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN"),
        }
    }
}

/// Validate a configuration
pub fn validate_config(config: &EdithConfig) -> ValidationResult {
    let mut result = ValidationResult::default();

    // Validate gateway configuration
    if let Some(ref gateway) = config.gateway {
        if let Some(port) = gateway.port {
            if port == 0 {
                result.add_error("gateway.port", "port must be > 0");
            }
        }

        // Check auth configuration
        if let Some(ref auth) = gateway.auth {
            if auth.mode.is_some() && auth.token.is_none() && auth.password.is_none() {
                result.add_warning(
                    "gateway.auth",
                    "auth mode set but no token or password configured",
                );
            }
        }
    }

    // Validate Telegram configuration
    if let Some(ref telegram) = config.telegram {
        if telegram.enabled.unwrap_or(true) {
            if telegram.bot_token.is_none() && telegram.token_file.is_none() {
                result.add_warning(
                    "telegram",
                    "Telegram enabled but no botToken or tokenFile configured",
                );
            }

            if let Some(ref proxy) = telegram.proxy {
                if !proxy.starts_with("http://")
                    && !proxy.starts_with("https://")
                    && !proxy.starts_with("socks5://")
                {
                    result.add_error(
                        "telegram.proxy",
                        "proxy must be http://, https://, or socks5:// URL",
                    );
                }
            }
        }
    }

    // Validate Discord configuration
    if let Some(ref discord) = config.discord {
        if discord.enabled.unwrap_or(true) && discord.token.is_none() {
            result.add_warning("discord", "Discord enabled but no token configured");
        }

        if let Some(limit) = discord.text_chunk_limit {
            if limit > 2000 {
                result.add_warning(
                    "discord.textChunkLimit",
                    "Discord has a 2000 character message limit",
                );
            }
        }
    }

    // Validate Signal configuration
    if let Some(ref signal) = config.signal {
        if signal.enabled.unwrap_or(true) {
            if signal.http_url.is_none() && signal.http_host.is_none() {
                result.add_warning(
                    "signal",
                    "Signal enabled but no httpUrl or httpHost configured",
                );
            }
        }
    }

    // Validate hooks configuration
    if let Some(ref hooks) = config.hooks {
        if hooks.enabled.unwrap_or(false) && hooks.token.is_none() {
            result.add_warning(
                "hooks",
                "hooks enabled but no token configured - endpoints will be unprotected",
            );
        }

        if let Some(ref mappings) = hooks.mappings {
            for (i, mapping) in mappings.iter().enumerate() {
                if mapping.action.is_none()
                    && mapping.message_template.is_none()
                    && mapping.text_template.is_none()
                {
                    result.add_warning(
                        format!("hooks.mappings[{}]", i),
                        "mapping has no action or template",
                    );
                }
            }
        }
    }

    // Validate agent configuration
    if let Some(ref agent) = config.agent {
        if let Some(timeout) = agent.timeout_seconds {
            if timeout == 0 {
                result.add_error("agent.timeoutSeconds", "timeout must be > 0");
            }
        }

        if let Some(max_concurrent) = agent.max_concurrent {
            if max_concurrent == 0 {
                result.add_error("agent.maxConcurrent", "maxConcurrent must be > 0");
            }
        }

        // Validate sandbox configuration
        if let Some(ref sandbox) = agent.sandbox {
            if let Some(ref docker) = sandbox.docker {
                if docker.image.is_none() {
                    result.add_warning(
                        "agent.sandbox.docker.image",
                        "no Docker image specified for sandbox",
                    );
                }
            }
        }
    }

    // Validate session configuration
    if let Some(ref session) = config.session {
        if let Some(idle) = session.idle_minutes {
            if idle == 0 {
                result.add_warning(
                    "session.idleMinutes",
                    "idleMinutes is 0, sessions will never expire",
                );
            }
        }
    }

    // Validate routing configuration
    if let Some(ref routing) = config.routing {
        if let Some(ref queue) = routing.queue {
            if let Some(cap) = queue.cap {
                if cap == 0 {
                    result.add_error("routing.queue.cap", "queue cap must be > 0");
                }
            }
        }
    }

    result
}

/// Validate and return errors as ConfigError
pub fn validate_config_strict(config: &EdithConfig) -> ConfigResult<()> {
    let result = validate_config(config);
    if !result.is_valid() {
        let messages: Vec<_> = result.issues.iter().map(|i| i.to_string()).collect();
        return Err(ConfigError::validation(messages.join("; ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_config() {
        let config = EdithConfig::default();
        let result = validate_config(&config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_invalid_port() {
        let config = EdithConfig {
            gateway: Some(crate::schema::GatewayConfig {
                port: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = validate_config(&config);
        assert!(!result.is_valid());
        assert!(result.issues[0].message.contains("port must be > 0"));
    }
}
