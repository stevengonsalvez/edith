//! Provider-specific configuration helpers

use crate::schema::*;

/// Messaging surface types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    WhatsApp,
    Telegram,
    Discord,
    Signal,
    IMessage,
    WebChat,
}

impl Surface {
    pub fn as_str(&self) -> &'static str {
        match self {
            Surface::WhatsApp => "whatsapp",
            Surface::Telegram => "telegram",
            Surface::Discord => "discord",
            Surface::Signal => "signal",
            Surface::IMessage => "imessage",
            Surface::WebChat => "webchat",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "whatsapp" => Some(Surface::WhatsApp),
            "telegram" => Some(Surface::Telegram),
            "discord" => Some(Surface::Discord),
            "signal" => Some(Surface::Signal),
            "imessage" => Some(Surface::IMessage),
            "webchat" => Some(Surface::WebChat),
            _ => None,
        }
    }
}

impl std::fmt::Display for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Check if a provider is enabled in config
pub fn is_provider_enabled(config: &EdithConfig, surface: Surface) -> bool {
    match surface {
        Surface::WhatsApp => config
            .web
            .as_ref()
            .and_then(|w| w.enabled)
            .unwrap_or(true),
        Surface::Telegram => config
            .telegram
            .as_ref()
            .and_then(|t| t.enabled)
            .unwrap_or(true)
            && config
                .telegram
                .as_ref()
                .and_then(|t| t.bot_token.as_ref())
                .is_some(),
        Surface::Discord => config
            .discord
            .as_ref()
            .and_then(|d| d.enabled)
            .unwrap_or(true)
            && config
                .discord
                .as_ref()
                .and_then(|d| d.token.as_ref())
                .is_some(),
        Surface::Signal => config
            .signal
            .as_ref()
            .and_then(|s| s.enabled)
            .unwrap_or(true),
        Surface::IMessage => {
            #[cfg(target_os = "macos")]
            {
                config
                    .imessage
                    .as_ref()
                    .and_then(|i| i.enabled)
                    .unwrap_or(true)
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        }
        Surface::WebChat => true, // WebChat is always enabled if the gateway is running
    }
}

/// Get text chunk limit for a surface
pub fn get_text_chunk_limit(config: &EdithConfig, surface: Surface) -> u64 {
    match surface {
        Surface::WhatsApp => config
            .whatsapp
            .as_ref()
            .and_then(|w| w.text_chunk_limit)
            .unwrap_or(crate::defaults::DEFAULT_WHATSAPP_TEXT_CHUNK_LIMIT),
        Surface::Telegram => config
            .telegram
            .as_ref()
            .and_then(|t| t.text_chunk_limit)
            .unwrap_or(crate::defaults::DEFAULT_TELEGRAM_TEXT_CHUNK_LIMIT),
        Surface::Discord => config
            .discord
            .as_ref()
            .and_then(|d| d.text_chunk_limit)
            .unwrap_or(crate::defaults::DEFAULT_DISCORD_TEXT_CHUNK_LIMIT),
        Surface::Signal => config
            .signal
            .as_ref()
            .and_then(|s| s.text_chunk_limit)
            .unwrap_or(crate::defaults::DEFAULT_WHATSAPP_TEXT_CHUNK_LIMIT),
        Surface::IMessage => config
            .imessage
            .as_ref()
            .and_then(|i| i.text_chunk_limit)
            .unwrap_or(crate::defaults::DEFAULT_WHATSAPP_TEXT_CHUNK_LIMIT),
        Surface::WebChat => crate::defaults::DEFAULT_WHATSAPP_TEXT_CHUNK_LIMIT,
    }
}

/// Get media max size in MB for a surface
pub fn get_media_max_mb(config: &EdithConfig, surface: Surface) -> f64 {
    match surface {
        Surface::WhatsApp => crate::defaults::DEFAULT_MEDIA_MAX_MB,
        Surface::Telegram => config
            .telegram
            .as_ref()
            .and_then(|t| t.media_max_mb)
            .unwrap_or(crate::defaults::DEFAULT_MEDIA_MAX_MB),
        Surface::Discord => config
            .discord
            .as_ref()
            .and_then(|d| d.media_max_mb)
            .unwrap_or(crate::defaults::DEFAULT_MEDIA_MAX_MB),
        Surface::Signal => config
            .signal
            .as_ref()
            .and_then(|s| s.media_max_mb)
            .unwrap_or(crate::defaults::DEFAULT_MEDIA_MAX_MB),
        Surface::IMessage => config
            .imessage
            .as_ref()
            .and_then(|i| i.media_max_mb)
            .unwrap_or(crate::defaults::DEFAULT_MEDIA_MAX_MB),
        Surface::WebChat => crate::defaults::DEFAULT_MEDIA_MAX_MB,
    }
}

/// Get queue mode for a surface
pub fn get_queue_mode(config: &EdithConfig, surface: Surface) -> Option<QueueMode> {
    // First check surface-specific override
    if let Some(ref routing) = config.routing {
        if let Some(ref queue) = routing.queue {
            if let Some(ref by_surface) = queue.by_surface {
                let surface_mode = match surface {
                    Surface::WhatsApp => by_surface.whatsapp.clone(),
                    Surface::Telegram => by_surface.telegram.clone(),
                    Surface::Discord => by_surface.discord.clone(),
                    Surface::Signal => by_surface.signal.clone(),
                    Surface::IMessage => by_surface.imessage.clone(),
                    Surface::WebChat => by_surface.webchat.clone(),
                };
                if surface_mode.is_some() {
                    return surface_mode;
                }
            }
            // Fall back to global mode
            return queue.mode.clone();
        }
    }
    None
}

/// Check if a group requires mention for a given surface and group id
pub fn requires_group_mention(
    config: &EdithConfig,
    surface: Surface,
    group_id: &str,
) -> bool {
    match surface {
        Surface::WhatsApp => config
            .whatsapp
            .as_ref()
            .and_then(|w| {
                w.groups.as_ref().and_then(|g| {
                    g.get(group_id)
                        .or_else(|| g.get("*"))
                        .and_then(|gc| gc.require_mention)
                })
            })
            .unwrap_or(true),
        Surface::Telegram => config
            .telegram
            .as_ref()
            .and_then(|t| {
                t.groups.as_ref().and_then(|g| {
                    g.get(group_id)
                        .or_else(|| g.get("*"))
                        .and_then(|gc| gc.require_mention)
                })
            })
            .unwrap_or(true),
        Surface::IMessage => config
            .imessage
            .as_ref()
            .and_then(|i| {
                i.groups.as_ref().and_then(|g| {
                    g.get(group_id)
                        .or_else(|| g.get("*"))
                        .and_then(|gc| gc.require_mention)
                })
            })
            .unwrap_or(true),
        _ => true,
    }
}

/// Get the Telegram bot token from config
pub fn get_telegram_token(config: &EdithConfig) -> Option<String> {
    let telegram = config.telegram.as_ref()?;

    // First try direct token
    if let Some(ref token) = telegram.bot_token {
        return Some(token.clone());
    }

    // Then try token file
    if let Some(ref token_file) = telegram.token_file {
        if let Ok(token) = std::fs::read_to_string(token_file) {
            return Some(token.trim().to_string());
        }
    }

    // Finally try environment variable
    std::env::var("TELEGRAM_BOT_TOKEN").ok()
}

/// Get the Discord bot token from config
pub fn get_discord_token(config: &EdithConfig) -> Option<String> {
    config
        .discord
        .as_ref()
        .and_then(|d| d.token.clone())
        .or_else(|| std::env::var("DISCORD_BOT_TOKEN").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_from_str() {
        assert_eq!(Surface::from_str("whatsapp"), Some(Surface::WhatsApp));
        assert_eq!(Surface::from_str("TELEGRAM"), Some(Surface::Telegram));
        assert_eq!(Surface::from_str("Discord"), Some(Surface::Discord));
        assert_eq!(Surface::from_str("unknown"), None);
    }

    #[test]
    fn test_is_provider_enabled_defaults() {
        let config = EdithConfig::default();
        assert!(is_provider_enabled(&config, Surface::WhatsApp));
        assert!(!is_provider_enabled(&config, Surface::Telegram)); // No token
        assert!(!is_provider_enabled(&config, Surface::Discord)); // No token
    }
}
