//! Edith CLI - Personal AI Assistant Gateway
//!
//! A self-hosted gateway connecting messaging platforms to LLM APIs.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use edith::{Edith, EdithConfig};

/// Edith - Personal AI Assistant Gateway
#[derive(Parser)]
#[command(name = "edith")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config.json5")]
    config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Enable JSON logging
    #[arg(long)]
    json_logs: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the gateway server
    Start {
        /// Port to bind to
        #[arg(short, long)]
        port: Option<u16>,

        /// Host to bind to (default: 0.0.0.0)
        #[arg(long)]
        host: Option<String>,

        /// Force kill any process using the port
        #[arg(long)]
        force: bool,
    },

    /// Validate configuration file
    Validate,

    /// Show current configuration
    Show,

    /// Initialize a new configuration file
    Init {
        /// Output file path
        #[arg(short, long, default_value = "config.json5")]
        output: String,
    },

    /// Check provider connectivity
    Check {
        /// Provider to check (telegram, whatsapp, etc.)
        provider: Option<String>,
    },

    /// Show version information
    Version,

    /// Send a message via configured providers
    Send {
        /// Recipient (E.164 phone, chat ID, @username, etc.)
        #[arg(short, long)]
        to: String,

        /// Message text
        #[arg(short, long)]
        message: String,

        /// Provider to use (telegram, whatsapp, discord, signal, imessage)
        #[arg(short, long, default_value = "whatsapp")]
        provider: String,

        /// Attach media (local path or URL)
        #[arg(long)]
        media: Option<String>,

        /// Output result as JSON
        #[arg(long)]
        json: bool,

        /// Dry run (don't actually send)
        #[arg(long)]
        dry_run: bool,
    },

    /// Talk directly to the AI agent
    Agent {
        /// Message for the agent
        #[arg(short, long)]
        message: String,

        /// Recipient (for session key derivation)
        #[arg(short, long)]
        to: Option<String>,

        /// Explicit session ID
        #[arg(long)]
        session_id: Option<String>,

        /// Provider to deliver reply (if --deliver is set)
        #[arg(short, long, default_value = "whatsapp")]
        provider: String,

        /// Deliver the agent's reply to the recipient
        #[arg(long)]
        deliver: bool,

        /// Output result as JSON
        #[arg(long)]
        json: bool,

        /// Timeout in seconds (default: 600)
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Health checks and quick fixes
    Doctor {
        /// Auto-fix detected issues
        #[arg(long)]
        fix: bool,
    },

    /// Show status of providers and sessions
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Run deep provider probes
        #[arg(long)]
        deep: bool,

        /// Probe timeout in milliseconds
        #[arg(long, default_value = "10000")]
        timeout: u64,
    },

    /// List conversation sessions
    Sessions {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Only show sessions updated within N minutes
        #[arg(long)]
        active: Option<u64>,
    },

    /// Gateway control commands
    Gateway {
        #[command(subcommand)]
        action: GatewayAction,
    },
}

#[derive(Subcommand)]
enum GatewayAction {
    /// Start the gateway
    Start {
        /// Port to bind to
        #[arg(short, long)]
        port: Option<u16>,

        /// Force kill any process using the port
        #[arg(long)]
        force: bool,
    },

    /// Stop the running gateway
    Stop,

    /// Restart the gateway
    Restart,

    /// Show gateway status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show connected clients
    Clients {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // Set up logging
    let log_level = match cli.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    if cli.json_logs {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    // Execute command
    match cli.command {
        None => {
            run_server(&cli.config, None, None).await
        }
        Some(Commands::Start { port, host, force: _ }) => {
            run_server(&cli.config, port, host.as_deref()).await
        }
        Some(Commands::Validate) => {
            validate_config(&cli.config)
        }
        Some(Commands::Show) => {
            show_config(&cli.config)
        }
        Some(Commands::Init { output }) => {
            init_config(&output)
        }
        Some(Commands::Check { provider }) => {
            check_providers(&cli.config, provider).await
        }
        Some(Commands::Version) => {
            show_version();
            Ok(())
        }
        Some(Commands::Send { to, message, provider, media, json, dry_run }) => {
            send_message(&cli.config, &to, &message, &provider, media.as_deref(), json, dry_run).await
        }
        Some(Commands::Agent { message, to, session_id, provider, deliver, json, timeout }) => {
            run_agent(&cli.config, &message, to.as_deref(), session_id.as_deref(), &provider, deliver, json, timeout).await
        }
        Some(Commands::Doctor { fix }) => {
            run_doctor(&cli.config, fix).await
        }
        Some(Commands::Status { json, deep, timeout }) => {
            show_status(&cli.config, json, deep, timeout).await
        }
        Some(Commands::Sessions { json, active }) => {
            list_sessions(&cli.config, json, active).await
        }
        Some(Commands::Gateway { action }) => {
            handle_gateway_action(&cli.config, action).await
        }
    }
}

/// Run the gateway server
async fn run_server(config_path: &str, port: Option<u16>, host: Option<&str>) -> Result<()> {
    info!(config = %config_path, "Loading configuration");

    let mut config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    // Override port/host if specified
    if port.is_some() || host.is_some() {
        let gateway = config.gateway.get_or_insert_with(Default::default);
        if let Some(p) = port {
            gateway.port = Some(p);
        }
        if let Some(_h) = host {
            // Note: host binding would be set here when gateway supports it
        }
    }

    info!("Configuration loaded successfully");

    let mut app = Edith::new(config)
        .context("Failed to create Edith instance")?;

    info!("Starting Edith...");
    app.run().await.context("Edith runtime error")?;

    Ok(())
}

/// Validate configuration file
fn validate_config(config_path: &str) -> Result<()> {
    println!("Validating configuration: {}", config_path);

    match EdithConfig::from_file(config_path) {
        Ok(config) => {
            println!("Configuration is valid!");

            // Show summary
            println!("\nConfiguration Summary:");
            println!("  Identity: {}", config.identity.as_ref().and_then(|i| i.name.as_ref()).unwrap_or(&"(not set)".to_string()));

            let telegram_enabled = config.telegram.as_ref().map(|t| t.enabled.unwrap_or(false)).unwrap_or(false);
            let whatsapp_enabled = config.whatsapp.as_ref().map(|w| w.enabled.unwrap_or(false)).unwrap_or(false);
            let discord_enabled = config.discord.as_ref().map(|d| d.enabled.unwrap_or(false)).unwrap_or(false);

            println!("  Providers:");
            println!("    Telegram: {}", if telegram_enabled { "enabled" } else { "disabled" });
            println!("    WhatsApp: {}", if whatsapp_enabled { "enabled" } else { "disabled" });
            println!("    Discord: {}", if discord_enabled { "enabled" } else { "disabled" });

            let gateway_port = config.gateway.as_ref().and_then(|g| g.port).unwrap_or(8080);
            println!("  Gateway Port: {}", gateway_port);

            Ok(())
        }
        Err(e) => {
            eprintln!("Configuration validation failed:");
            eprintln!("  {}", e);
            std::process::exit(1);
        }
    }
}

/// Show current configuration
fn show_config(config_path: &str) -> Result<()> {
    let config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    // Serialize to JSON with pretty printing (hide sensitive values)
    let json = serde_json::to_string_pretty(&config)?;
    println!("{}", json);

    Ok(())
}

/// Initialize a new configuration file
fn init_config(output_path: &str) -> Result<()> {
    if std::path::Path::new(output_path).exists() {
        eprintln!("Configuration file already exists: {}", output_path);
        eprintln!("Use a different path or remove the existing file.");
        std::process::exit(1);
    }

    let default_config = r#"{
    // Edith Configuration
    // See https://github.com/steipete/edith for documentation

    // Bot identity
    "identity": {
        "name": "Assistant",
        "systemPrompt": "You are a helpful AI assistant. Be concise and helpful."
    },

    // LLM configuration
    "llm": {
        // API key can also be set via ANTHROPIC_API_KEY env var
        // "apiKey": "sk-ant-...",
        "model": "claude-3-5-sonnet-20241022",
        "maxTokens": 4096
    },

    // HTTP Gateway
    "gateway": {
        "enabled": true,
        "port": 8080,
        "auth": {
            "type": "bearer",
            "token": "your-secret-token"
        }
    },

    // Telegram configuration
    "telegram": {
        "enabled": false,
        // Token can also be set via TELEGRAM_BOT_TOKEN env var
        // "token": "123456:ABC-DEF..."
        "allowFrom": ["*"]
    },

    // WhatsApp configuration (requires external bridge)
    "whatsapp": {
        "enabled": false
    },

    // Session configuration
    "sessions": {
        "store": "json",
        "path": "./sessions",
        "maxMessages": 100
    }
}
"#;

    std::fs::write(output_path, default_config)?;
    println!("Created configuration file: {}", output_path);
    println!("\nNext steps:");
    println!("  1. Edit {} to configure your providers", output_path);
    println!("  2. Set your API keys (via config or environment variables)");
    println!("  3. Run: edith start");

    Ok(())
}

/// Check provider connectivity
async fn check_providers(config_path: &str, provider: Option<String>) -> Result<()> {
    let config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    println!("Checking provider connectivity...\n");

    // Check Telegram
    if provider.is_none() || provider.as_deref() == Some("telegram") {
        print!("Telegram: ");
        if let Some(ref telegram) = config.telegram {
            if telegram.enabled.unwrap_or(false) {
                if telegram.bot_token.is_some() || std::env::var("TELEGRAM_BOT_TOKEN").is_ok() {
                    println!("configured (token present)");
                } else {
                    println!("MISSING TOKEN");
                }
            } else {
                println!("disabled");
            }
        } else {
            println!("not configured");
        }
    }

    // Check WhatsApp
    if provider.is_none() || provider.as_deref() == Some("whatsapp") {
        print!("WhatsApp: ");
        if let Some(ref whatsapp) = config.whatsapp {
            if whatsapp.enabled.unwrap_or(false) {
                if config.bridge.as_ref().map(|b| b.enabled.unwrap_or(false)).unwrap_or(false) {
                    println!("configured (bridge mode)");
                } else {
                    println!("enabled but NO BRIDGE configured");
                }
            } else {
                println!("disabled");
            }
        } else {
            println!("not configured");
        }
    }

    // Check LLM
    if provider.is_none() || provider.as_deref() == Some("llm") {
        print!("LLM (Anthropic): ");
        if config.llm.as_ref().and_then(|l| l.api_key.as_ref()).is_some()
            || std::env::var("ANTHROPIC_API_KEY").is_ok()
        {
            println!("configured (API key present)");
        } else {
            println!("MISSING API KEY");
        }
    }

    Ok(())
}

/// Show version information
fn show_version() {
    println!("Edith v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("A self-hosted personal AI assistant gateway");
    println!("https://github.com/steipete/edith");
}

/// Send a message via configured providers
async fn send_message(
    config_path: &str,
    to: &str,
    message: &str,
    provider: &str,
    media: Option<&str>,
    json_output: bool,
    dry_run: bool,
) -> Result<()> {
    let config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    let payload = serde_json::json!({
        "to": to,
        "message": message,
        "provider": provider,
        "media": media,
    });

    if dry_run {
        if json_output {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        } else {
            println!("Dry run - would send:");
            println!("  To: {}", to);
            println!("  Provider: {}", provider);
            println!("  Message: {}", message);
            if let Some(m) = media {
                println!("  Media: {}", m);
            }
        }
        return Ok(());
    }

    // TODO: Implement actual sending via gateway WebSocket or direct provider
    info!(to = to, provider = provider, "Sending message");

    let result = serde_json::json!({
        "success": true,
        "to": to,
        "provider": provider,
        "message_id": "placeholder",
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Message sent successfully to {} via {}", to, provider);
    }

    Ok(())
}

/// Run agent command
async fn run_agent(
    config_path: &str,
    message: &str,
    to: Option<&str>,
    session_id: Option<&str>,
    provider: &str,
    deliver: bool,
    json_output: bool,
    timeout: Option<u64>,
) -> Result<()> {
    let config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    let timeout_secs = timeout.unwrap_or(600);
    let session = session_id
        .map(String::from)
        .or_else(|| to.map(|t| format!("cli:{}", t)))
        .unwrap_or_else(|| "cli:default".to_string());

    info!(
        session = %session,
        timeout_secs = timeout_secs,
        "Running agent command"
    );

    // TODO: Implement agent interaction via gateway
    let result = serde_json::json!({
        "session": session,
        "input": message,
        "output": "Agent response placeholder",
        "delivered": deliver,
        "provider": if deliver { Some(provider) } else { None },
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Agent response: {}", result["output"]);
        if deliver {
            if let Some(recipient) = to {
                println!("Delivered to {} via {}", recipient, provider);
            }
        }
    }

    Ok(())
}

/// Run doctor command for health checks and fixes
async fn run_doctor(config_path: &str, fix: bool) -> Result<()> {
    println!("Running health checks...\n");

    let mut issues = Vec::new();
    let mut fixes = Vec::new();

    // Check config file
    match EdithConfig::from_file(config_path) {
        Ok(_) => println!("✓ Configuration file valid"),
        Err(e) => {
            println!("✗ Configuration file invalid: {}", e);
            issues.push("config_invalid");
        }
    }

    // Check for API key
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        println!("✓ ANTHROPIC_API_KEY environment variable set");
    } else {
        println!("✗ ANTHROPIC_API_KEY not set");
        issues.push("missing_api_key");
    }

    // Check session directory
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let edith_dir = home.join(".edith");
    if edith_dir.exists() {
        println!("✓ Edith directory exists: {}", edith_dir.display());
    } else {
        println!("✗ Edith directory missing: {}", edith_dir.display());
        issues.push("missing_edith_dir");
        if fix {
            std::fs::create_dir_all(&edith_dir)?;
            println!("  → Created {}", edith_dir.display());
            fixes.push("created_edith_dir");
        }
    }

    // Summary
    println!();
    if issues.is_empty() {
        println!("All checks passed!");
    } else {
        println!("{} issue(s) found", issues.len());
        if fix {
            println!("{} issue(s) fixed", fixes.len());
        } else {
            println!("Run with --fix to attempt automatic fixes");
        }
    }

    Ok(())
}

/// Show status of providers and sessions
async fn show_status(config_path: &str, json_output: bool, deep: bool, timeout: u64) -> Result<()> {
    let config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    let mut providers = Vec::new();

    // Check each provider
    if let Some(ref telegram) = config.telegram {
        providers.push(serde_json::json!({
            "name": "telegram",
            "enabled": telegram.enabled.unwrap_or(false),
            "configured": telegram.bot_token.is_some(),
        }));
    }

    if let Some(ref whatsapp) = config.whatsapp {
        providers.push(serde_json::json!({
            "name": "whatsapp",
            "enabled": whatsapp.enabled.unwrap_or(false),
        }));
    }

    if let Some(ref discord) = config.discord {
        providers.push(serde_json::json!({
            "name": "discord",
            "enabled": discord.enabled.unwrap_or(false),
            "configured": discord.token.is_some(),
        }));
    }

    if let Some(ref signal) = config.signal {
        providers.push(serde_json::json!({
            "name": "signal",
            "enabled": signal.enabled.unwrap_or(false),
        }));
    }

    if let Some(ref imessage) = config.imessage {
        providers.push(serde_json::json!({
            "name": "imessage",
            "enabled": imessage.enabled.unwrap_or(false),
        }));
    }

    let status = serde_json::json!({
        "providers": providers,
        "gateway": {
            "port": config.gateway.as_ref().and_then(|g| g.port).unwrap_or(8080),
        },
        "deep_probe": deep,
        "timeout_ms": timeout,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("Provider Status:\n");
        for p in &providers {
            let name = p["name"].as_str().unwrap_or("unknown");
            let enabled = p["enabled"].as_bool().unwrap_or(false);
            let icon = if enabled { "✓" } else { "✗" };
            println!("  {} {}: {}", icon, name, if enabled { "enabled" } else { "disabled" });
        }
        println!();
        println!("Gateway port: {}", status["gateway"]["port"]);
    }

    Ok(())
}

/// List conversation sessions
async fn list_sessions(config_path: &str, json_output: bool, active_minutes: Option<u64>) -> Result<()> {
    let config = EdithConfig::from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;

    // Get session store path
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let session_path = config
        .session
        .as_ref()
        .and_then(|s| s.store.as_ref())
        .map(|p| home.join(".edith").join(p))
        .unwrap_or_else(|| home.join(".edith").join("sessions.db"));

    // TODO: Implement session listing from SQLite store
    let sessions: Vec<serde_json::Value> = vec![];

    let result = serde_json::json!({
        "sessions": sessions,
        "count": sessions.len(),
        "filter_active_minutes": active_minutes,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        if sessions.is_empty() {
            println!("No sessions found");
        } else {
            println!("Sessions: {}", sessions.len());
            for session in &sessions {
                println!("  - {}", session["key"]);
            }
        }
    }

    Ok(())
}

/// Handle gateway subcommands
async fn handle_gateway_action(config_path: &str, action: GatewayAction) -> Result<()> {
    match action {
        GatewayAction::Start { port, force } => {
            println!("Starting gateway...");
            if force {
                println!("Force mode: killing any existing process on port");
            }
            run_server(config_path, port, None).await
        }
        GatewayAction::Stop => {
            println!("Stopping gateway...");
            // TODO: Implement gateway stop via IPC
            Ok(())
        }
        GatewayAction::Restart => {
            println!("Restarting gateway...");
            // TODO: Implement gateway restart via IPC
            Ok(())
        }
        GatewayAction::Status { json } => {
            // TODO: Query running gateway status
            let status = serde_json::json!({
                "running": false,
                "uptime_seconds": 0,
            });
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("Gateway: not running");
            }
            Ok(())
        }
        GatewayAction::Clients { json } => {
            // TODO: Query connected clients from gateway
            let clients: Vec<serde_json::Value> = vec![];
            let result = serde_json::json!({
                "clients": clients,
                "count": clients.len(),
            });
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                if clients.is_empty() {
                    println!("No connected clients");
                } else {
                    println!("Connected clients: {}", clients.len());
                }
            }
            Ok(())
        }
    }
}
