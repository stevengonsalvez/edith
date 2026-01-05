//! Default values for configuration

/// Default session idle timeout in minutes
pub const DEFAULT_IDLE_MINUTES: u64 = 60;

/// Default session reset triggers
pub const DEFAULT_RESET_TRIGGERS: &[&str] = &["/new", "/reset"];

/// Default text chunk limit for WhatsApp
pub const DEFAULT_WHATSAPP_TEXT_CHUNK_LIMIT: u64 = 4000;

/// Default text chunk limit for Telegram
pub const DEFAULT_TELEGRAM_TEXT_CHUNK_LIMIT: u64 = 4000;

/// Default text chunk limit for Discord
pub const DEFAULT_DISCORD_TEXT_CHUNK_LIMIT: u64 = 2000;

/// Default media max size in MB
pub const DEFAULT_MEDIA_MAX_MB: f64 = 5.0;

/// Default webhook path for Telegram
pub const DEFAULT_TELEGRAM_WEBHOOK_PATH: &str = "/telegram-webhook";

/// Default gateway bind mode
pub const DEFAULT_GATEWAY_BIND: &str = "loopback";

/// Default bridge port
pub const DEFAULT_BRIDGE_PORT: u16 = 18790;

/// Default canvas host port
pub const DEFAULT_CANVAS_HOST_PORT: u16 = 18793;

/// Default hooks base path
pub const DEFAULT_HOOKS_PATH: &str = "/hooks";

/// Default hooks max body bytes
pub const DEFAULT_HOOKS_MAX_BODY_BYTES: u64 = 1024 * 1024; // 1MB

/// Default queue debounce ms
pub const DEFAULT_QUEUE_DEBOUNCE_MS: u64 = 100;

/// Default queue cap
pub const DEFAULT_QUEUE_CAP: u64 = 10;

/// Default bash background timeout ms
pub const DEFAULT_BASH_BACKGROUND_MS: u64 = 5000;

/// Default bash command timeout seconds
pub const DEFAULT_BASH_TIMEOUT_SEC: u64 = 120;

/// Default bash cleanup ms
pub const DEFAULT_BASH_CLEANUP_MS: u64 = 60000;

/// Default agent timeout seconds
pub const DEFAULT_AGENT_TIMEOUT_SECONDS: u64 = 300;

/// Default max concurrent agent runs
pub const DEFAULT_AGENT_MAX_CONCURRENT: u64 = 1;

/// Default typing interval seconds
pub const DEFAULT_TYPING_INTERVAL_SECONDS: u64 = 3;

/// Default heartbeat interval
pub const DEFAULT_HEARTBEAT_EVERY: &str = "5m";

/// Default heartbeat prompt
pub const DEFAULT_HEARTBEAT_PROMPT: &str = "HEARTBEAT";

/// Default WebSocket reconnect initial delay ms
pub const DEFAULT_WS_RECONNECT_INITIAL_MS: u64 = 1000;

/// Default WebSocket reconnect max delay ms
pub const DEFAULT_WS_RECONNECT_MAX_MS: u64 = 30000;

/// Default WebSocket reconnect backoff factor
pub const DEFAULT_WS_RECONNECT_FACTOR: f64 = 2.0;

/// Default WebSocket reconnect jitter
pub const DEFAULT_WS_RECONNECT_JITTER: f64 = 0.1;

/// Default Signal HTTP port
pub const DEFAULT_SIGNAL_HTTP_PORT: u16 = 8080;

/// Default Signal HTTP host
pub const DEFAULT_SIGNAL_HTTP_HOST: &str = "127.0.0.1";

/// Default group history limit
pub const DEFAULT_GROUP_HISTORY_LIMIT: u64 = 10;

/// Default ElevenLabs voice model
pub const DEFAULT_ELEVENLABS_MODEL_ID: &str = "eleven_turbo_v2";

/// Default ElevenLabs output format
pub const DEFAULT_ELEVENLABS_OUTPUT_FORMAT: &str = "mp3_44100_128";

/// Default sandbox container prefix
pub const DEFAULT_SANDBOX_CONTAINER_PREFIX: &str = "edith-sandbox-";

/// Default sandbox workdir
pub const DEFAULT_SANDBOX_WORKDIR: &str = "/workspace";

/// Default config reload debounce ms
pub const DEFAULT_RELOAD_DEBOUNCE_MS: u64 = 300;
