//! Configuration schema types
//!
//! These types mirror the TypeScript Edith configuration schema.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EdithConfig {
    /// Identity settings for the assistant
    pub identity: Option<IdentityConfig>,

    /// Wizard state tracking
    pub wizard: Option<WizardConfig>,

    /// Logging configuration
    pub logging: Option<LoggingConfig>,

    /// Browser automation settings
    pub browser: Option<BrowserConfig>,

    /// UI customization
    pub ui: Option<UiConfig>,

    /// Skills configuration
    pub skills: Option<SkillsConfig>,

    /// Model providers and definitions
    pub models: Option<ModelsConfig>,

    /// Agent configuration
    pub agent: Option<AgentConfig>,

    /// Message routing configuration
    pub routing: Option<RoutingConfig>,

    /// Message formatting
    pub messages: Option<MessagesConfig>,

    /// Session management
    pub session: Option<SessionConfig>,

    /// WhatsApp Web provider settings
    pub web: Option<WebConfig>,

    /// WhatsApp-specific settings
    pub whatsapp: Option<WhatsAppConfig>,

    /// Telegram provider settings
    pub telegram: Option<TelegramConfig>,

    /// Discord provider settings
    pub discord: Option<DiscordConfig>,

    /// Signal provider settings
    pub signal: Option<SignalConfig>,

    /// iMessage provider settings (macOS only)
    pub imessage: Option<IMessageConfig>,

    /// Cron job configuration
    pub cron: Option<CronConfig>,

    /// HTTP hooks configuration
    pub hooks: Option<HooksConfig>,

    /// Bridge server configuration
    pub bridge: Option<BridgeConfig>,

    /// Discovery configuration
    pub discovery: Option<DiscoveryConfig>,

    /// Canvas host configuration
    #[serde(rename = "canvasHost")]
    pub canvas_host: Option<CanvasHostConfig>,

    /// Talk mode (voice) configuration
    pub talk: Option<TalkConfig>,

    /// Gateway configuration
    pub gateway: Option<GatewayConfig>,

    /// LLM configuration
    pub llm: Option<LlmConfig>,
}

impl EdithConfig {
    /// Load configuration from a file path
    pub fn from_file(path: &str) -> Result<Self, crate::ConfigError> {
        crate::load_config_from_path(std::path::Path::new(path))
    }
}

// ============================================================================
// LLM Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// API key for the LLM provider
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    /// Model to use (e.g., "claude-3-5-sonnet-20241022")
    pub model: Option<String>,
    /// Maximum tokens for response
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    /// Temperature for sampling
    pub temperature: Option<f32>,
    /// Provider name (anthropic, openai, etc.)
    pub provider: Option<String>,
    /// Base URL for API (for proxies)
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
}

// ============================================================================
// Identity Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentityConfig {
    /// Name of the assistant
    pub name: Option<String>,
    /// Theme identifier
    pub theme: Option<String>,
    /// Emoji representation
    pub emoji: Option<String>,
    /// System prompt for the assistant
    #[serde(rename = "systemPrompt")]
    pub system_prompt: Option<String>,
}

// ============================================================================
// Wizard Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WizardConfig {
    #[serde(rename = "lastRunAt")]
    pub last_run_at: Option<String>,
    #[serde(rename = "lastRunVersion")]
    pub last_run_version: Option<String>,
    #[serde(rename = "lastRunCommit")]
    pub last_run_commit: Option<String>,
    #[serde(rename = "lastRunCommand")]
    pub last_run_command: Option<String>,
    #[serde(rename = "lastRunMode")]
    pub last_run_mode: Option<GatewayMode>,
}

// ============================================================================
// Logging Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Silent,
    Fatal,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleStyle {
    #[default]
    Pretty,
    Compact,
    Json,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: Option<LogLevel>,
    pub file: Option<String>,
    #[serde(rename = "consoleLevel")]
    pub console_level: Option<LogLevel>,
    #[serde(rename = "consoleStyle")]
    pub console_style: Option<ConsoleStyle>,
}

// ============================================================================
// Browser Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "controlUrl")]
    pub control_url: Option<String>,
    #[serde(rename = "cdpUrl")]
    pub cdp_url: Option<String>,
    pub color: Option<String>,
    #[serde(rename = "executablePath")]
    pub executable_path: Option<String>,
    pub headless: Option<bool>,
    #[serde(rename = "noSandbox")]
    pub no_sandbox: Option<bool>,
    #[serde(rename = "attachOnly")]
    pub attach_only: Option<bool>,
}

// ============================================================================
// UI Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    #[serde(rename = "seamColor")]
    pub seam_color: Option<String>,
}

// ============================================================================
// Skills Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    pub env: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsLoadConfig {
    #[serde(rename = "extraDirs")]
    pub extra_dirs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeManager {
    #[default]
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsInstallConfig {
    #[serde(rename = "preferBrew")]
    pub prefer_brew: Option<bool>,
    #[serde(rename = "nodeManager")]
    pub node_manager: Option<NodeManager>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsConfig {
    #[serde(rename = "allowBundled")]
    pub allow_bundled: Option<Vec<String>>,
    pub load: Option<SkillsLoadConfig>,
    pub install: Option<SkillsInstallConfig>,
    pub entries: Option<HashMap<String, SkillConfig>>,
}

// ============================================================================
// Models Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelApi {
    OpenaiCompletions,
    OpenaiResponses,
    AnthropicMessages,
    GoogleGenerativeAi,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelCompatConfig {
    #[serde(rename = "supportsStore")]
    pub supports_store: Option<bool>,
    #[serde(rename = "supportsDeveloperRole")]
    pub supports_developer_role: Option<bool>,
    #[serde(rename = "supportsReasoningEffort")]
    pub supports_reasoning_effort: Option<bool>,
    #[serde(rename = "maxTokensField")]
    pub max_tokens_field: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelInput {
    Text,
    Image,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelDefinitionConfig {
    pub id: String,
    pub name: String,
    pub api: Option<ModelApi>,
    pub reasoning: bool,
    pub input: Vec<ModelInput>,
    pub cost: ModelCost,
    #[serde(rename = "contextWindow")]
    pub context_window: u64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u64,
    pub headers: Option<HashMap<String, String>>,
    pub compat: Option<ModelCompatConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelProviderConfig {
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub api: Option<ModelApi>,
    pub headers: Option<HashMap<String, String>>,
    #[serde(rename = "authHeader")]
    pub auth_header: Option<bool>,
    pub models: Vec<ModelDefinitionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelsMode {
    Merge,
    Replace,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub mode: Option<ModelsMode>,
    pub providers: Option<HashMap<String, ModelProviderConfig>>,
}

// ============================================================================
// Agent Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnOff {
    Off,
    On,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockStreamingBreak {
    TextEnd,
    MessageEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BreakPreference {
    Paragraph,
    Newline,
    Sentence,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BlockStreamingChunk {
    #[serde(rename = "minChars")]
    pub min_chars: Option<u64>,
    #[serde(rename = "maxChars")]
    pub max_chars: Option<u64>,
    #[serde(rename = "breakPreference")]
    pub break_preference: Option<BreakPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeartbeatTarget {
    Last,
    Whatsapp,
    Telegram,
    Discord,
    Signal,
    Imessage,
    None,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HeartbeatConfig {
    pub every: Option<String>,
    pub model: Option<String>,
    pub target: Option<HeartbeatTarget>,
    pub to: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BashToolConfig {
    #[serde(rename = "backgroundMs")]
    pub background_ms: Option<u64>,
    #[serde(rename = "timeoutSec")]
    pub timeout_sec: Option<u64>,
    #[serde(rename = "cleanupMs")]
    pub cleanup_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    #[default]
    Off,
    NonMain,
    All,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DockerSandboxConfig {
    pub image: Option<String>,
    #[serde(rename = "containerPrefix")]
    pub container_prefix: Option<String>,
    pub workdir: Option<String>,
    #[serde(rename = "readOnlyRoot")]
    pub read_only_root: Option<bool>,
    pub tmpfs: Option<Vec<String>>,
    pub network: Option<String>,
    pub user: Option<String>,
    #[serde(rename = "capDrop")]
    pub cap_drop: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    #[serde(rename = "setupCommand")]
    pub setup_command: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxBrowserConfig {
    pub enabled: Option<bool>,
    pub image: Option<String>,
    #[serde(rename = "containerPrefix")]
    pub container_prefix: Option<String>,
    #[serde(rename = "cdpPort")]
    pub cdp_port: Option<u16>,
    #[serde(rename = "vncPort")]
    pub vnc_port: Option<u16>,
    #[serde(rename = "noVncPort")]
    pub no_vnc_port: Option<u16>,
    pub headless: Option<bool>,
    #[serde(rename = "enableNoVnc")]
    pub enable_no_vnc: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxToolsConfig {
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxPruneConfig {
    #[serde(rename = "idleHours")]
    pub idle_hours: Option<u64>,
    #[serde(rename = "maxAgeDays")]
    pub max_age_days: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxConfig {
    pub mode: Option<SandboxMode>,
    #[serde(rename = "perSession")]
    pub per_session: Option<bool>,
    #[serde(rename = "workspaceRoot")]
    pub workspace_root: Option<String>,
    pub docker: Option<DockerSandboxConfig>,
    pub browser: Option<SandboxBrowserConfig>,
    pub tools: Option<SandboxToolsConfig>,
    pub prune: Option<SandboxPruneConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub model: Option<String>,
    pub workspace: Option<String>,
    #[serde(rename = "allowedModels")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(rename = "modelAliases")]
    pub model_aliases: Option<HashMap<String, String>>,
    #[serde(rename = "contextTokens")]
    pub context_tokens: Option<u64>,
    #[serde(rename = "thinkingDefault")]
    pub thinking_default: Option<ThinkingLevel>,
    #[serde(rename = "verboseDefault")]
    pub verbose_default: Option<OnOff>,
    #[serde(rename = "blockStreamingDefault")]
    pub block_streaming_default: Option<OnOff>,
    #[serde(rename = "blockStreamingBreak")]
    pub block_streaming_break: Option<BlockStreamingBreak>,
    #[serde(rename = "blockStreamingChunk")]
    pub block_streaming_chunk: Option<BlockStreamingChunk>,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u64>,
    #[serde(rename = "mediaMaxMb")]
    pub media_max_mb: Option<f64>,
    #[serde(rename = "typingIntervalSeconds")]
    pub typing_interval_seconds: Option<u64>,
    pub heartbeat: Option<HeartbeatConfig>,
    #[serde(rename = "maxConcurrent")]
    pub max_concurrent: Option<u64>,
    pub bash: Option<BashToolConfig>,
    pub sandbox: Option<SandboxConfig>,
}

// ============================================================================
// Routing Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueueMode {
    Steer,
    Followup,
    Collect,
    SteerBacklog,
    #[serde(rename = "steer+backlog")]
    SteerPlusBacklog,
    Queue,
    Interrupt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueDropPolicy {
    Old,
    New,
    Summarize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueModeBySurface {
    pub whatsapp: Option<QueueMode>,
    pub telegram: Option<QueueMode>,
    pub discord: Option<QueueMode>,
    pub signal: Option<QueueMode>,
    pub imessage: Option<QueueMode>,
    pub webchat: Option<QueueMode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueConfig {
    pub mode: Option<QueueMode>,
    #[serde(rename = "bySurface")]
    pub by_surface: Option<QueueModeBySurface>,
    #[serde(rename = "debounceMs")]
    pub debounce_ms: Option<u64>,
    pub cap: Option<u64>,
    pub drop: Option<QueueDropPolicy>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TranscribeAudioConfig {
    pub command: Vec<String>,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupChatConfig {
    #[serde(rename = "mentionPatterns")]
    pub mention_patterns: Option<Vec<String>>,
    #[serde(rename = "historyLimit")]
    pub history_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    #[serde(rename = "transcribeAudio")]
    pub transcribe_audio: Option<TranscribeAudioConfig>,
    #[serde(rename = "groupChat")]
    pub group_chat: Option<GroupChatConfig>,
    pub queue: Option<QueueConfig>,
}

// ============================================================================
// Messages Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessagesConfig {
    #[serde(rename = "messagePrefix")]
    pub message_prefix: Option<String>,
    #[serde(rename = "responsePrefix")]
    pub response_prefix: Option<String>,
    #[serde(rename = "timestampPrefix")]
    pub timestamp_prefix: Option<serde_json::Value>, // bool or string
}

// ============================================================================
// Session Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionScope {
    PerSender,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplyToMode {
    Off,
    First,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SendPolicyAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SendPolicyMatch {
    pub surface: Option<String>,
    #[serde(rename = "chatType")]
    pub chat_type: Option<String>,
    #[serde(rename = "keyPrefix")]
    pub key_prefix: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SendPolicyRule {
    pub action: Option<SendPolicyAction>,
    #[serde(rename = "match")]
    pub match_: Option<SendPolicyMatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SendPolicyConfig {
    pub default: Option<SendPolicyAction>,
    pub rules: Option<Vec<SendPolicyRule>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub scope: Option<SessionScope>,
    #[serde(rename = "resetTriggers")]
    pub reset_triggers: Option<Vec<String>>,
    #[serde(rename = "idleMinutes")]
    pub idle_minutes: Option<u64>,
    #[serde(rename = "heartbeatIdleMinutes")]
    pub heartbeat_idle_minutes: Option<u64>,
    pub store: Option<String>,
    #[serde(rename = "typingIntervalSeconds")]
    pub typing_interval_seconds: Option<u64>,
    #[serde(rename = "mainKey")]
    pub main_key: Option<String>,
    #[serde(rename = "sendPolicy")]
    pub send_policy: Option<SendPolicyConfig>,
}

// ============================================================================
// Web (WhatsApp Web) Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WebReconnectConfig {
    #[serde(rename = "initialMs")]
    pub initial_ms: Option<u64>,
    #[serde(rename = "maxMs")]
    pub max_ms: Option<u64>,
    pub factor: Option<f64>,
    pub jitter: Option<f64>,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "heartbeatSeconds")]
    pub heartbeat_seconds: Option<u64>,
    pub reconnect: Option<WebReconnectConfig>,
}

// ============================================================================
// WhatsApp Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WhatsAppGroupConfig {
    #[serde(rename = "requireMention")]
    pub require_mention: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WhatsAppConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "allowFrom")]
    pub allow_from: Option<Vec<String>>,
    #[serde(rename = "textChunkLimit")]
    pub text_chunk_limit: Option<u64>,
    pub groups: Option<HashMap<String, WhatsAppGroupConfig>>,
}

// ============================================================================
// Telegram Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramGroupConfig {
    #[serde(rename = "requireMention")]
    pub require_mention: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "botToken")]
    pub bot_token: Option<String>,
    #[serde(rename = "tokenFile")]
    pub token_file: Option<String>,
    #[serde(rename = "replyToMode")]
    pub reply_to_mode: Option<ReplyToMode>,
    pub groups: Option<HashMap<String, TelegramGroupConfig>>,
    #[serde(rename = "allowFrom")]
    pub allow_from: Option<Vec<serde_json::Value>>, // string | number
    #[serde(rename = "textChunkLimit")]
    pub text_chunk_limit: Option<u64>,
    #[serde(rename = "mediaMaxMb")]
    pub media_max_mb: Option<f64>,
    pub proxy: Option<String>,
    #[serde(rename = "webhookUrl")]
    pub webhook_url: Option<String>,
    #[serde(rename = "webhookSecret")]
    pub webhook_secret: Option<String>,
    #[serde(rename = "webhookPath")]
    pub webhook_path: Option<String>,
}

// ============================================================================
// Discord Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordDmConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "allowFrom")]
    pub allow_from: Option<Vec<serde_json::Value>>,
    #[serde(rename = "groupEnabled")]
    pub group_enabled: Option<bool>,
    #[serde(rename = "groupChannels")]
    pub group_channels: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordChannelConfig {
    pub allow: Option<bool>,
    #[serde(rename = "requireMention")]
    pub require_mention: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscordReactionMode {
    Off,
    Own,
    All,
    Allowlist,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordGuildEntry {
    pub slug: Option<String>,
    #[serde(rename = "requireMention")]
    pub require_mention: Option<bool>,
    #[serde(rename = "reactionNotifications")]
    pub reaction_notifications: Option<DiscordReactionMode>,
    pub users: Option<Vec<serde_json::Value>>,
    pub channels: Option<HashMap<String, DiscordChannelConfig>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordSlashCommandConfig {
    pub enabled: Option<bool>,
    pub name: Option<String>,
    #[serde(rename = "sessionPrefix")]
    pub session_prefix: Option<String>,
    pub ephemeral: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordActionsConfig {
    pub reactions: Option<bool>,
    pub stickers: Option<bool>,
    pub polls: Option<bool>,
    pub permissions: Option<bool>,
    pub messages: Option<bool>,
    pub threads: Option<bool>,
    pub pins: Option<bool>,
    pub search: Option<bool>,
    #[serde(rename = "memberInfo")]
    pub member_info: Option<bool>,
    #[serde(rename = "roleInfo")]
    pub role_info: Option<bool>,
    pub roles: Option<bool>,
    #[serde(rename = "channelInfo")]
    pub channel_info: Option<bool>,
    #[serde(rename = "voiceStatus")]
    pub voice_status: Option<bool>,
    pub events: Option<bool>,
    pub moderation: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    pub enabled: Option<bool>,
    pub token: Option<String>,
    #[serde(rename = "textChunkLimit")]
    pub text_chunk_limit: Option<u64>,
    #[serde(rename = "mediaMaxMb")]
    pub media_max_mb: Option<f64>,
    #[serde(rename = "historyLimit")]
    pub history_limit: Option<u64>,
    pub actions: Option<DiscordActionsConfig>,
    #[serde(rename = "replyToMode")]
    pub reply_to_mode: Option<ReplyToMode>,
    #[serde(rename = "slashCommand")]
    pub slash_command: Option<DiscordSlashCommandConfig>,
    pub dm: Option<DiscordDmConfig>,
    pub guilds: Option<HashMap<String, DiscordGuildEntry>>,
}

// ============================================================================
// Signal Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalReceiveMode {
    OnStart,
    Manual,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SignalConfig {
    pub enabled: Option<bool>,
    pub account: Option<String>,
    #[serde(rename = "httpUrl")]
    pub http_url: Option<String>,
    #[serde(rename = "httpHost")]
    pub http_host: Option<String>,
    #[serde(rename = "httpPort")]
    pub http_port: Option<u16>,
    #[serde(rename = "cliPath")]
    pub cli_path: Option<String>,
    #[serde(rename = "autoStart")]
    pub auto_start: Option<bool>,
    #[serde(rename = "receiveMode")]
    pub receive_mode: Option<SignalReceiveMode>,
    #[serde(rename = "ignoreAttachments")]
    pub ignore_attachments: Option<bool>,
    #[serde(rename = "ignoreStories")]
    pub ignore_stories: Option<bool>,
    #[serde(rename = "sendReadReceipts")]
    pub send_read_receipts: Option<bool>,
    #[serde(rename = "allowFrom")]
    pub allow_from: Option<Vec<serde_json::Value>>,
    #[serde(rename = "textChunkLimit")]
    pub text_chunk_limit: Option<u64>,
    #[serde(rename = "mediaMaxMb")]
    pub media_max_mb: Option<f64>,
}

// ============================================================================
// iMessage Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IMessageService {
    Imessage,
    Sms,
    Auto,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IMessageGroupConfig {
    #[serde(rename = "requireMention")]
    pub require_mention: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IMessageConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "cliPath")]
    pub cli_path: Option<String>,
    #[serde(rename = "dbPath")]
    pub db_path: Option<String>,
    pub service: Option<IMessageService>,
    pub region: Option<String>,
    #[serde(rename = "allowFrom")]
    pub allow_from: Option<Vec<serde_json::Value>>,
    #[serde(rename = "includeAttachments")]
    pub include_attachments: Option<bool>,
    #[serde(rename = "mediaMaxMb")]
    pub media_max_mb: Option<f64>,
    #[serde(rename = "textChunkLimit")]
    pub text_chunk_limit: Option<u64>,
    pub groups: Option<HashMap<String, IMessageGroupConfig>>,
}

// ============================================================================
// Cron Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CronConfig {
    pub enabled: Option<bool>,
    pub store: Option<String>,
    #[serde(rename = "maxConcurrentRuns")]
    pub max_concurrent_runs: Option<u64>,
}

// ============================================================================
// Hooks Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HookMappingMatch {
    pub path: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HookMappingTransform {
    pub module: String,
    pub export: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookAction {
    Wake,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WakeMode {
    Now,
    NextHeartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookChannel {
    Last,
    Whatsapp,
    Telegram,
    Discord,
    Signal,
    Imessage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HookMappingConfig {
    pub id: Option<String>,
    #[serde(rename = "match")]
    pub match_: Option<HookMappingMatch>,
    pub action: Option<HookAction>,
    #[serde(rename = "wakeMode")]
    pub wake_mode: Option<WakeMode>,
    pub name: Option<String>,
    #[serde(rename = "sessionKey")]
    pub session_key: Option<String>,
    #[serde(rename = "messageTemplate")]
    pub message_template: Option<String>,
    #[serde(rename = "textTemplate")]
    pub text_template: Option<String>,
    pub deliver: Option<bool>,
    pub channel: Option<HookChannel>,
    pub to: Option<String>,
    pub thinking: Option<String>,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u64>,
    pub transform: Option<HookMappingTransform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TailscaleMode {
    Off,
    Serve,
    Funnel,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksGmailServeConfig {
    pub bind: Option<String>,
    pub port: Option<u16>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksGmailTailscaleConfig {
    pub mode: Option<TailscaleMode>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksGmailConfig {
    pub account: Option<String>,
    pub label: Option<String>,
    pub topic: Option<String>,
    pub subscription: Option<String>,
    #[serde(rename = "pushToken")]
    pub push_token: Option<String>,
    #[serde(rename = "hookUrl")]
    pub hook_url: Option<String>,
    #[serde(rename = "includeBody")]
    pub include_body: Option<bool>,
    #[serde(rename = "maxBytes")]
    pub max_bytes: Option<u64>,
    #[serde(rename = "renewEveryMinutes")]
    pub renew_every_minutes: Option<u64>,
    pub serve: Option<HooksGmailServeConfig>,
    pub tailscale: Option<HooksGmailTailscaleConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    pub token: Option<String>,
    #[serde(rename = "maxBodyBytes")]
    pub max_body_bytes: Option<u64>,
    pub presets: Option<Vec<String>>,
    #[serde(rename = "transformsDir")]
    pub transforms_dir: Option<String>,
    pub mappings: Option<Vec<HookMappingConfig>>,
    pub gmail: Option<HooksGmailConfig>,
}

// ============================================================================
// Bridge Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BridgeBindMode {
    Auto,
    Lan,
    Tailnet,
    Loopback,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BridgeConfig {
    pub enabled: Option<bool>,
    pub port: Option<u16>,
    pub bind: Option<BridgeBindMode>,
}

// ============================================================================
// Discovery Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WideAreaDiscoveryConfig {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoveryConfig {
    #[serde(rename = "wideArea")]
    pub wide_area: Option<WideAreaDiscoveryConfig>,
}

// ============================================================================
// Canvas Host Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasHostConfig {
    pub enabled: Option<bool>,
    pub root: Option<String>,
    pub port: Option<u16>,
}

// ============================================================================
// Talk Configuration
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TalkConfig {
    #[serde(rename = "voiceId")]
    pub voice_id: Option<String>,
    #[serde(rename = "voiceAliases")]
    pub voice_aliases: Option<HashMap<String, String>>,
    #[serde(rename = "modelId")]
    pub model_id: Option<String>,
    #[serde(rename = "outputFormat")]
    pub output_format: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "interruptOnSpeech")]
    pub interrupt_on_speech: Option<bool>,
}

// ============================================================================
// Gateway Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayMode {
    Local,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayAuthMode {
    Token,
    Password,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayAuthConfig {
    pub mode: Option<GatewayAuthMode>,
    pub token: Option<String>,
    pub password: Option<String>,
    #[serde(rename = "allowTailscale")]
    pub allow_tailscale: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayTailscaleConfig {
    pub mode: Option<TailscaleMode>,
    #[serde(rename = "resetOnExit")]
    pub reset_on_exit: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayRemoteConfig {
    pub url: Option<String>,
    pub token: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayReloadMode {
    Off,
    Restart,
    Hot,
    Hybrid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayReloadConfig {
    pub mode: Option<GatewayReloadMode>,
    #[serde(rename = "debounceMs")]
    pub debounce_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayControlUiConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "basePath")]
    pub base_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayConfig {
    pub enabled: Option<bool>,
    pub port: Option<u16>,
    pub mode: Option<GatewayMode>,
    pub bind: Option<BridgeBindMode>,
    #[serde(rename = "controlUi")]
    pub control_ui: Option<GatewayControlUiConfig>,
    pub auth: Option<GatewayAuthConfig>,
    pub tailscale: Option<GatewayTailscaleConfig>,
    pub remote: Option<GatewayRemoteConfig>,
    pub reload: Option<GatewayReloadConfig>,
}
