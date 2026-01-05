//! Protocol constants matching Clawdis implementation

/// Current protocol version
pub const PROTOCOL_VERSION: u32 = 2;

/// Method name constants for RPC dispatch
pub mod methods {
    // Health & System
    pub const HEALTH: &str = "health";
    pub const STATUS: &str = "status";
    pub const PROVIDERS_STATUS: &str = "providers.status";

    // Configuration
    pub const CONFIG_GET: &str = "config.get";
    pub const CONFIG_SET: &str = "config.set";
    pub const CONFIG_SCHEMA: &str = "config.schema";

    // Sessions
    pub const SESSIONS_LIST: &str = "sessions.list";
    pub const SESSIONS_PATCH: &str = "sessions.patch";
    pub const SESSIONS_RESET: &str = "sessions.reset";
    pub const SESSIONS_DELETE: &str = "sessions.delete";
    pub const SESSIONS_COMPACT: &str = "sessions.compact";

    // Messaging
    pub const SEND: &str = "send";
    pub const AGENT: &str = "agent";
    pub const AGENT_WAIT: &str = "agent.wait";
    pub const CHAT_SEND: &str = "chat.send";
    pub const CHAT_ABORT: &str = "chat.abort";
    pub const CHAT_HISTORY: &str = "chat.history";

    // Cron
    pub const CRON_LIST: &str = "cron.list";
    pub const CRON_STATUS: &str = "cron.status";
    pub const CRON_ADD: &str = "cron.add";
    pub const CRON_UPDATE: &str = "cron.update";
    pub const CRON_REMOVE: &str = "cron.remove";
    pub const CRON_RUN: &str = "cron.run";
    pub const CRON_RUNS: &str = "cron.runs";

    // Node Bridge
    pub const NODE_PAIR_REQUEST: &str = "node.pair.request";
    pub const NODE_PAIR_LIST: &str = "node.pair.list";
    pub const NODE_PAIR_APPROVE: &str = "node.pair.approve";
    pub const NODE_PAIR_REJECT: &str = "node.pair.reject";
    pub const NODE_PAIR_VERIFY: &str = "node.pair.verify";
    pub const NODE_RENAME: &str = "node.rename";
    pub const NODE_LIST: &str = "node.list";
    pub const NODE_DESCRIBE: &str = "node.describe";
    pub const NODE_INVOKE: &str = "node.invoke";

    // Models & Skills
    pub const MODELS_LIST: &str = "models.list";
    pub const SKILLS_LIST: &str = "skills.list";
    pub const SKILLS_REGISTRY: &str = "skills.registry";
    pub const SKILLS_STATUS: &str = "skills.status";
    pub const SKILLS_INSTALL: &str = "skills.install";
    pub const SKILLS_UNINSTALL: &str = "skills.uninstall";
    pub const SKILLS_UPDATE: &str = "skills.update";

    // Wizard
    pub const WIZARD_INIT: &str = "wizard.init";
    pub const WIZARD_STEP: &str = "wizard.step";
    pub const WIZARD_STATE: &str = "wizard.state";
    pub const WIZARD_ABORT: &str = "wizard.abort";
    pub const WIZARD_START: &str = "wizard.start";
    pub const WIZARD_NEXT: &str = "wizard.next";
    pub const WIZARD_CANCEL: &str = "wizard.cancel";
    pub const WIZARD_STATUS: &str = "wizard.status";

    // Talk/Voice
    pub const TALK_MODE: &str = "talk.mode";
    pub const VOICEWAKE_GET: &str = "voicewake.get";
    pub const VOICEWAKE_SET: &str = "voicewake.set";

    // Web Login
    pub const WEB_LOGIN_REQUEST: &str = "web.login.request";
    pub const WEB_LOGIN_CHECK: &str = "web.login.check";
    pub const WEB_LOGIN_START: &str = "web.login.start";
    pub const WEB_LOGIN_STATUS: &str = "web.login.status";
    pub const WEB_LOGIN_CANCEL: &str = "web.login.cancel";

    // System
    pub const SYSTEM_PRESENCE: &str = "system-presence";
    pub const SYSTEM_EVENT: &str = "system-event";
    pub const WAKE: &str = "wake";
}

/// Event name constants
pub mod events {
    pub const AGENT: &str = "agent";
    pub const CHAT: &str = "chat";
    pub const PRESENCE: &str = "presence";
    pub const TICK: &str = "tick";
    pub const TALK_MODE: &str = "talk.mode";
    pub const SHUTDOWN: &str = "shutdown";
    pub const HEALTH: &str = "health";
    pub const HEARTBEAT: &str = "heartbeat";
    pub const CRON: &str = "cron";
    pub const NODE_PAIR_REQUESTED: &str = "node.pair.requested";
    pub const NODE_PAIR_RESOLVED: &str = "node.pair.resolved";
    pub const VOICEWAKE_CHANGED: &str = "voicewake.changed";
}

/// Minimum supported protocol version
pub const MIN_PROTOCOL_VERSION: u32 = 1;

/// Handshake timeout in milliseconds
pub const HANDSHAKE_TIMEOUT_MS: u64 = 5000;

/// Health refresh interval in milliseconds
pub const HEALTH_REFRESH_INTERVAL_MS: u64 = 10000;

/// Tick (keepalive) interval in milliseconds
pub const TICK_INTERVAL_MS: u64 = 30000;

/// Deduplication cache TTL in milliseconds
pub const DEDUPE_TTL_MS: u64 = 60000;

/// Maximum deduplication cache entries
pub const DEDUPE_MAX: usize = 1000;

/// Maximum payload size in bytes (1 MB)
pub const MAX_PAYLOAD_BYTES: usize = 1_048_576;

/// Maximum buffered bytes per connection (16 MB)
pub const MAX_BUFFERED_BYTES: usize = 16_777_216;

/// Maximum chat history message bytes (5 MB)
pub const MAX_CHAT_HISTORY_BYTES: usize = 5_000_000;

/// Default gateway port
pub const DEFAULT_GATEWAY_PORT: u16 = 18789;

/// Default bridge port (for iOS/Android nodes)
pub const DEFAULT_BRIDGE_PORT: u16 = 18790;

/// Default canvas host port
pub const DEFAULT_CANVAS_PORT: u16 = 18793;

/// All supported RPC methods
pub const METHODS: &[&str] = &[
    // Health & System
    "health",
    "status",
    "providers.status",
    // Configuration
    "config.get",
    "config.set",
    "config.schema",
    // Sessions
    "sessions.list",
    "sessions.patch",
    "sessions.reset",
    "sessions.delete",
    "sessions.compact",
    // Messaging
    "send",
    "agent",
    "agent.wait",
    "chat.send",
    "chat.abort",
    "chat.history",
    // Cron
    "cron.list",
    "cron.status",
    "cron.add",
    "cron.update",
    "cron.remove",
    "cron.run",
    "cron.runs",
    // Node Bridge
    "node.pair.request",
    "node.pair.list",
    "node.pair.approve",
    "node.pair.reject",
    "node.pair.verify",
    "node.rename",
    "node.list",
    "node.describe",
    "node.invoke",
    // Models & Skills
    "models.list",
    "skills.status",
    "skills.install",
    "skills.update",
    // Wizard
    "wizard.start",
    "wizard.next",
    "wizard.cancel",
    "wizard.status",
    // Talk/Voice
    "talk.mode",
    "voicewake.get",
    "voicewake.set",
    // Web Login
    "web.login.start",
    "web.login.status",
    "web.login.cancel",
    // System
    "system-presence",
    "system-event",
    "wake",
];

/// All supported event types
pub const EVENTS: &[&str] = &[
    "agent",
    "chat",
    "presence",
    "tick",
    "talk.mode",
    "shutdown",
    "health",
    "heartbeat",
    "cron",
    "node.pair.requested",
    "node.pair.resolved",
    "voicewake.changed",
];
