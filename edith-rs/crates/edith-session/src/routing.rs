//! Session routing and key resolution

use crate::ChatType;

/// Message context for routing decisions
#[derive(Debug, Clone, Default)]
pub struct MessageContext {
    /// Sender identifier (E.164 for phone, ID for others)
    pub from: String,
    /// Recipient/conversation identifier
    pub to: String,
    /// Surface (whatsapp, telegram, discord, etc.)
    pub surface: Option<String>,
    /// Chat type (direct, group, room)
    pub chat_type: Option<ChatType>,
    /// Group subject/title
    pub subject: Option<String>,
    /// Room name (for Discord/Slack)
    pub room: Option<String>,
    /// Space/server name
    pub space: Option<String>,
    /// Explicit session key override
    pub session_key: Option<String>,
}

/// Session scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionScope {
    /// One session per sender
    PerSender,
    /// Single global session
    Global,
}

impl Default for SessionScope {
    fn default() -> Self {
        Self::PerSender
    }
}

/// Group key resolution result
#[derive(Debug, Clone)]
pub struct GroupKeyResolution {
    /// The canonical session key
    pub key: String,
    /// Legacy key format (for migration)
    pub legacy_key: Option<String>,
    /// Detected surface
    pub surface: Option<String>,
    /// Group/channel ID
    pub id: Option<String>,
    /// Chat type
    pub chat_type: ChatType,
}

/// Group surfaces that support groups
const GROUP_SURFACES: &[&str] = &[
    "whatsapp",
    "telegram",
    "discord",
    "signal",
    "imessage",
    "webchat",
    "slack",
];

/// Resolve the session key from message context
pub fn resolve_session_key(
    scope: SessionScope,
    ctx: &MessageContext,
    main_key: Option<&str>,
) -> String {
    // If explicit session key is provided, use it
    if let Some(ref key) = ctx.session_key {
        return key.clone();
    }

    // Global scope always returns the same key
    if scope == SessionScope::Global {
        return "global".to_string();
    }

    // Try to resolve as group
    if let Some(group) = resolve_group_session_key(ctx) {
        return group.key;
    }

    // For direct chats, use the main key (default: "main")
    main_key.unwrap_or("main").to_string()
}

/// Resolve group session key from context
pub fn resolve_group_session_key(ctx: &MessageContext) -> Option<GroupKeyResolution> {
    let from = ctx.from.trim();
    if from.is_empty() {
        return None;
    }

    // Check if this is a group chat
    let is_group = matches!(ctx.chat_type, Some(ChatType::Group) | Some(ChatType::Room))
        || from.starts_with("group:")
        || from.contains("@g.us")
        || from.contains(":group:")
        || from.contains(":channel:");

    if !is_group {
        return None;
    }

    let surface_hint = ctx.surface.as_deref().map(|s| s.to_lowercase());
    let has_legacy_prefix = from.starts_with("group:");

    // Extract the raw ID
    let raw = if has_legacy_prefix {
        from.strip_prefix("group:").unwrap_or(from).trim()
    } else {
        from
    };

    // Parse the key parts
    let parts: Vec<&str> = raw.split(':').filter(|s| !s.is_empty()).collect();

    let (surface, kind, id) = parse_group_parts(&parts, surface_hint.as_deref());

    let resolved_surface = surface.or(surface_hint);

    // No surface means we can't create a proper key
    if resolved_surface.is_none() {
        let legacy = if has_legacy_prefix {
            format!("group:{}", raw)
        } else {
            format!("group:{}", from)
        };
        return Some(GroupKeyResolution {
            key: legacy.clone(),
            legacy_key: Some(legacy),
            surface: None,
            id: Some(raw.to_string()),
            chat_type: ChatType::Group,
        });
    }

    let resolved_surface = resolved_surface.unwrap();
    let resolved_kind = kind.unwrap_or_else(|| "group".to_string());
    let resolved_id = id.unwrap_or_else(|| raw.to_string());

    let key = format!("{}:{}:{}", resolved_surface, resolved_kind, resolved_id);

    let legacy_key = if has_legacy_prefix || from.contains("@g.us") {
        Some(format!("group:{}", resolved_id))
    } else {
        None
    };

    let chat_type = if resolved_kind == "channel" {
        ChatType::Room
    } else {
        ChatType::Group
    };

    Some(GroupKeyResolution {
        key,
        legacy_key,
        surface: Some(resolved_surface),
        id: Some(resolved_id),
        chat_type,
    })
}

/// Parse group key parts
/// Returns (surface, kind, id) all as owned strings
fn parse_group_parts(
    parts: &[&str],
    surface_hint: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>) {
    if parts.is_empty() {
        return (None, None, None);
    }

    // Check if first part is a known surface
    if parts.len() >= 2 && GROUP_SURFACES.contains(&parts[0]) {
        let surface = Some(parts[0].to_string());

        // Check if second part is kind (group/channel)
        if parts.len() >= 3 && (parts[1] == "group" || parts[1] == "channel") {
            let kind = Some(parts[1].to_string());
            let id = parts[2..].join(":");
            let id = if id.is_empty() { None } else { Some(id) };
            return (surface, kind, id);
        }

        // No kind, rest is ID
        let id = parts[1..].join(":");
        return (surface, Some("group".to_string()), Some(id));
    }

    // Check if first part is a kind
    if parts.len() >= 2 && (parts[0] == "group" || parts[0] == "channel") {
        let kind = Some(parts[0].to_string());
        let id = parts[1..].join(":");
        return (
            surface_hint.map(String::from),
            kind,
            Some(id),
        );
    }

    // Just an ID
    let id = parts.join(":");
    (
        surface_hint.map(String::from),
        Some("group".to_string()),
        Some(id),
    )
}

/// Normalize an E.164 phone number
pub fn normalize_e164(phone: &str) -> String {
    let cleaned: String = phone.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();

    if cleaned.starts_with('+') {
        cleaned
    } else if cleaned.len() == 10 {
        // US number without country code
        format!("+1{}", cleaned)
    } else if cleaned.len() > 10 && !cleaned.starts_with('0') {
        // Assume it has country code
        format!("+{}", cleaned)
    } else {
        cleaned
    }
}

/// Build a display name for a group session
pub fn build_group_display_name(
    surface: Option<&str>,
    subject: Option<&str>,
    room: Option<&str>,
    space: Option<&str>,
    id: Option<&str>,
    key: &str,
) -> String {
    let surface_key = surface.unwrap_or("group").to_lowercase();

    // Build detail from available info
    let detail = if let (Some(r), Some(s)) = (room, space) {
        let hash = if r.starts_with('#') { "" } else { "#" };
        format!("{}{}{}", s, hash, r)
    } else {
        room.or(subject).or(space).unwrap_or("").to_string()
    };

    let fallback_id = id.unwrap_or_else(|| key.strip_prefix("group:").unwrap_or(key));
    let raw_label = if detail.is_empty() {
        fallback_id.to_string()
    } else {
        detail
    };

    // Normalize the label
    let token = normalize_group_label(&raw_label);
    let token = if token.is_empty() {
        normalize_group_label(&shorten_id(fallback_id))
    } else {
        token
    };

    // Add prefix if needed
    let token = if !token.is_empty()
        && !token.starts_with('@')
        && !token.starts_with('#')
        && !token.starts_with("g-")
        && !token.contains('#')
    {
        format!("g-{}", token)
    } else {
        token
    };

    if token.is_empty() {
        surface_key
    } else {
        format!("{}:{}", surface_key, token)
    }
}

/// Normalize a group label
fn normalize_group_label(raw: &str) -> String {
    let trimmed = raw.trim().to_lowercase();
    if trimmed.is_empty() {
        return String::new();
    }

    // Replace spaces with dashes
    let dashed = trimmed.replace(char::is_whitespace, "-");

    // Keep only allowed chars
    let cleaned: String = dashed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "#@._+-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Collapse multiple dashes
    let mut result = String::new();
    let mut last_was_dash = false;
    for c in cleaned.chars() {
        if c == '-' {
            if !last_was_dash {
                result.push(c);
                last_was_dash = true;
            }
        } else {
            result.push(c);
            last_was_dash = false;
        }
    }

    // Trim dashes and dots from ends
    result.trim_matches(|c| c == '-' || c == '.').to_string()
}

/// Shorten a long ID for display
fn shorten_id(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.len() <= 14 {
        trimmed.to_string()
    } else {
        format!("{}...{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_session_key_direct() {
        let ctx = MessageContext {
            from: "+1234567890".to_string(),
            ..Default::default()
        };

        let key = resolve_session_key(SessionScope::PerSender, &ctx, None);
        assert_eq!(key, "main");
    }

    #[test]
    fn test_resolve_session_key_global() {
        let ctx = MessageContext {
            from: "+1234567890".to_string(),
            ..Default::default()
        };

        let key = resolve_session_key(SessionScope::Global, &ctx, None);
        assert_eq!(key, "global");
    }

    #[test]
    fn test_resolve_group_key() {
        let ctx = MessageContext {
            from: "group:1234@g.us".to_string(),
            surface: Some("whatsapp".to_string()),
            chat_type: Some(ChatType::Group),
            ..Default::default()
        };

        let result = resolve_group_session_key(&ctx);
        assert!(result.is_some());
        let group = result.unwrap();
        assert!(group.key.contains("whatsapp"));
        assert!(group.key.contains("group"));
    }

    #[test]
    fn test_normalize_e164() {
        assert_eq!(normalize_e164("1234567890"), "+11234567890");
        assert_eq!(normalize_e164("+1234567890"), "+1234567890");
        assert_eq!(normalize_e164("441234567890"), "+441234567890");
    }

    #[test]
    fn test_normalize_group_label() {
        assert_eq!(normalize_group_label("Test Group"), "test-group");
        assert_eq!(normalize_group_label("test--group"), "test-group");
        assert_eq!(normalize_group_label("--test--"), "test");
    }
}
