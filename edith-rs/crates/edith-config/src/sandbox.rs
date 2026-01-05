//! Sandbox/Container execution for Edith
//!
//! Provides isolated Docker container execution for agent sessions.
//! Features:
//! - Per-session or shared containers
//! - Tool allow/deny policies
//! - Container registry and lifecycle management
//! - Automatic pruning of idle containers

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};

// Re-use schema types where appropriate
pub use crate::schema::SandboxMode;

/// Default sandbox container image
const DEFAULT_SANDBOX_IMAGE: &str = "edith-sandbox:bookworm-slim";

/// Default container prefix
const DEFAULT_SANDBOX_CONTAINER_PREFIX: &str = "edith-sbx-";

/// Default workdir inside container
const DEFAULT_SANDBOX_WORKDIR: &str = "/workspace";

/// Default idle hours before pruning
const DEFAULT_SANDBOX_IDLE_HOURS: u64 = 24;

/// Default max age in days
const DEFAULT_SANDBOX_MAX_AGE_DAYS: u64 = 7;

/// Default allowed tools
const DEFAULT_TOOL_ALLOW: &[&str] = &["bash", "process", "read", "write", "edit"];

/// Default denied tools
const DEFAULT_TOOL_DENY: &[&str] = &["browser", "canvas", "nodes", "cron", "discord", "gateway"];

/// Tool policy for sandbox
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxToolPolicy {
    /// Allowed tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    /// Denied tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
}

impl SandboxToolPolicy {
    /// Check if a tool is allowed
    pub fn is_tool_allowed(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();

        // Check deny list first
        if let Some(ref deny) = self.deny {
            if deny.iter().any(|d| d.to_lowercase() == name_lower) {
                return false;
            }
        }

        // If allow list exists, check it
        if let Some(ref allow) = self.allow {
            if allow.is_empty() {
                return true;
            }
            return allow.iter().any(|a| a.to_lowercase() == name_lower);
        }

        true
    }
}

/// Docker container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxDockerConfig {
    /// Docker image
    pub image: String,
    /// Container name prefix
    pub container_prefix: String,
    /// Working directory inside container
    pub workdir: String,
    /// Mount root filesystem read-only
    pub read_only_root: bool,
    /// tmpfs mounts
    pub tmpfs: Vec<String>,
    /// Network mode
    pub network: String,
    /// User to run as
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Capabilities to drop
    pub cap_drop: Vec<String>,
    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Setup command to run after container starts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_command: Option<String>,
}

impl Default for SandboxDockerConfig {
    fn default() -> Self {
        let mut env = HashMap::new();
        env.insert("LANG".to_string(), "C.UTF-8".to_string());

        Self {
            image: DEFAULT_SANDBOX_IMAGE.to_string(),
            container_prefix: DEFAULT_SANDBOX_CONTAINER_PREFIX.to_string(),
            workdir: DEFAULT_SANDBOX_WORKDIR.to_string(),
            read_only_root: true,
            tmpfs: vec!["/tmp".to_string(), "/var/tmp".to_string(), "/run".to_string()],
            network: "bridge".to_string(),
            user: None,
            cap_drop: vec!["ALL".to_string()],
            env: Some(env),
            setup_command: None,
        }
    }
}

/// Runtime prune policy (resolved from config)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrunePolicy {
    /// Hours of inactivity before pruning
    pub idle_hours: u64,
    /// Maximum age in days
    pub max_age_days: u64,
}

impl Default for PrunePolicy {
    fn default() -> Self {
        Self {
            idle_hours: DEFAULT_SANDBOX_IDLE_HOURS,
            max_age_days: DEFAULT_SANDBOX_MAX_AGE_DAYS,
        }
    }
}

/// Full runtime sandbox configuration (resolved from schema config)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxRuntimeConfig {
    /// Sandbox mode
    pub mode: SandboxMode,
    /// Create per-session containers
    pub per_session: bool,
    /// Root directory for sandbox workspaces
    pub workspace_root: PathBuf,
    /// Docker configuration
    pub docker: SandboxDockerConfig,
    /// Tool policy
    pub tools: SandboxToolPolicy,
    /// Prune configuration
    pub prune: PrunePolicy,
}

impl Default for SandboxRuntimeConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            mode: SandboxMode::Off,
            per_session: true,
            workspace_root: home.join(".edith").join("sandboxes"),
            docker: SandboxDockerConfig::default(),
            tools: SandboxToolPolicy {
                allow: Some(DEFAULT_TOOL_ALLOW.iter().map(|s| s.to_string()).collect()),
                deny: Some(DEFAULT_TOOL_DENY.iter().map(|s| s.to_string()).collect()),
            },
            prune: PrunePolicy::default(),
        }
    }
}

/// Sandbox context for an active session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxContext {
    /// Whether sandbox is enabled
    pub enabled: bool,
    /// Session key
    pub session_key: String,
    /// Workspace directory on host
    pub workspace_dir: PathBuf,
    /// Container name
    pub container_name: String,
    /// Working directory inside container
    pub container_workdir: String,
    /// Docker configuration
    pub docker: SandboxDockerConfig,
    /// Tool policy
    pub tools: SandboxToolPolicy,
}

/// Registry entry for tracking containers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxRegistryEntry {
    /// Container name
    pub container_name: String,
    /// Session key
    pub session_key: String,
    /// Creation timestamp (ms since epoch)
    pub created_at_ms: u64,
    /// Last used timestamp (ms since epoch)
    pub last_used_at_ms: u64,
    /// Docker image
    pub image: String,
}

/// Container registry
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxRegistry {
    /// Registry entries
    pub entries: Vec<SandboxRegistryEntry>,
}

/// Sandbox manager for container lifecycle
pub struct SandboxManager {
    config: SandboxRuntimeConfig,
    state_dir: PathBuf,
    last_prune: Instant,
}

impl SandboxManager {
    /// Create a new sandbox manager
    pub fn new(config: SandboxRuntimeConfig, state_dir: PathBuf) -> Self {
        Self {
            config,
            state_dir,
            last_prune: Instant::now(),
        }
    }

    /// Get registry file path
    fn registry_path(&self) -> PathBuf {
        self.state_dir.join("sandbox").join("containers.json")
    }

    /// Check if session should be sandboxed
    pub fn should_sandbox(&self, session_key: &str, main_key: &str) -> bool {
        match self.config.mode {
            SandboxMode::Off => false,
            SandboxMode::All => true,
            SandboxMode::NonMain => session_key.trim() != main_key.trim(),
        }
    }

    /// Slugify session key for container naming
    fn slugify_session_key(&self, value: &str) -> String {
        let trimmed = if value.trim().is_empty() {
            "session"
        } else {
            value.trim()
        };

        // Create hash
        let mut hasher = Sha256::new();
        hasher.update(trimmed.as_bytes());
        let hash = format!("{:x}", hasher.finalize())[..8].to_string();

        // Sanitize
        let safe: String = trimmed
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '-' })
            .collect();
        let safe = safe.trim_matches('-');
        let base = if safe.len() > 32 { &safe[..32] } else { safe };
        let base = if base.is_empty() { "session" } else { base };

        format!("{}-{}", base, hash)
    }

    /// Get workspace directory for a session
    pub fn workspace_dir(&self, session_key: &str) -> PathBuf {
        let slug = self.slugify_session_key(session_key);
        self.config.workspace_root.join(slug)
    }

    /// Get container name for a session
    fn container_name(&self, session_key: &str) -> String {
        let slug = if self.config.per_session {
            self.slugify_session_key(session_key)
        } else {
            "shared".to_string()
        };

        let name = format!("{}{}", self.config.docker.container_prefix, slug);
        // Docker container names are limited to 63 chars
        if name.len() > 63 {
            name[..63].to_string()
        } else {
            name
        }
    }

    /// Read registry
    async fn read_registry(&self) -> SandboxRegistry {
        let path = self.registry_path();
        match fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => SandboxRegistry::default(),
        }
    }

    /// Write registry
    async fn write_registry(&self, registry: &SandboxRegistry) -> std::io::Result<()> {
        let path = self.registry_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let content = serde_json::to_string_pretty(registry)?;
        fs::write(&path, content).await
    }

    /// Update registry entry
    async fn update_registry(&self, entry: SandboxRegistryEntry) -> std::io::Result<()> {
        let mut registry = self.read_registry().await;

        // Find existing or add new
        if let Some(existing) = registry.entries.iter_mut().find(|e| e.container_name == entry.container_name) {
            existing.last_used_at_ms = entry.last_used_at_ms;
            existing.session_key = entry.session_key;
        } else {
            registry.entries.push(entry);
        }

        self.write_registry(&registry).await
    }

    /// Remove entry from registry
    async fn remove_registry_entry(&self, container_name: &str) -> std::io::Result<()> {
        let mut registry = self.read_registry().await;
        registry.entries.retain(|e| e.container_name != container_name);
        self.write_registry(&registry).await
    }

    /// Execute docker command
    async fn exec_docker(&self, args: &[&str]) -> Result<(String, String, i32), std::io::Error> {
        let output = Command::new("docker")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, code))
    }

    /// Check if container exists and is running
    async fn container_state(&self, name: &str) -> (bool, bool) {
        let args = ["inspect", "-f", "{{.State.Running}}", name];
        match self.exec_docker(&args).await {
            Ok((stdout, _, code)) => {
                if code != 0 {
                    (false, false)
                } else {
                    (true, stdout.trim() == "true")
                }
            }
            Err(_) => (false, false),
        }
    }

    /// Check if image exists
    async fn image_exists(&self, image: &str) -> bool {
        let args = ["image", "inspect", image];
        match self.exec_docker(&args).await {
            Ok((_, _, code)) => code == 0,
            Err(_) => false,
        }
    }

    /// Create sandbox container
    async fn create_container(
        &self,
        name: &str,
        workspace_dir: &Path,
        session_key: &str,
    ) -> Result<(), SandboxError> {
        // Check if image exists
        if !self.image_exists(&self.config.docker.image).await {
            return Err(SandboxError::ImageNotFound(self.config.docker.image.clone()));
        }

        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            name.to_string(),
        ];

        // Labels
        args.push("--label".to_string());
        args.push("edith.sandbox=1".to_string());
        args.push("--label".to_string());
        args.push(format!("edith.sessionKey={}", session_key));

        // Security options
        if self.config.docker.read_only_root {
            args.push("--read-only".to_string());
        }

        for tmpfs in &self.config.docker.tmpfs {
            args.push("--tmpfs".to_string());
            args.push(tmpfs.clone());
        }

        args.push("--network".to_string());
        args.push(self.config.docker.network.clone());

        if let Some(ref user) = self.config.docker.user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        for cap in &self.config.docker.cap_drop {
            args.push("--cap-drop".to_string());
            args.push(cap.clone());
        }

        args.push("--security-opt".to_string());
        args.push("no-new-privileges".to_string());

        args.push("--workdir".to_string());
        args.push(self.config.docker.workdir.clone());

        // Mount workspace
        args.push("-v".to_string());
        args.push(format!(
            "{}:{}",
            workspace_dir.display(),
            self.config.docker.workdir
        ));

        // Environment variables
        if let Some(ref env) = self.config.docker.env {
            for (key, value) in env {
                args.push("-e".to_string());
                args.push(format!("{}={}", key, value));
            }
        }

        args.push(self.config.docker.image.clone());
        args.push("sleep".to_string());
        args.push("infinity".to_string());

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (_, stderr, code) = self.exec_docker(&args_ref).await?;

        if code != 0 {
            return Err(SandboxError::DockerError(stderr));
        }

        // Start container
        let (_, stderr, code) = self.exec_docker(&["start", name]).await?;
        if code != 0 {
            return Err(SandboxError::DockerError(stderr));
        }

        // Run setup command if configured
        if let Some(ref setup) = self.config.docker.setup_command {
            let (_, stderr, code) = self
                .exec_docker(&["exec", "-i", name, "sh", "-lc", setup])
                .await?;
            if code != 0 {
                warn!(container = name, error = %stderr, "Setup command failed");
            }
        }

        info!(container = name, session = session_key, "Created sandbox container");
        Ok(())
    }

    /// Ensure container exists and is running
    pub async fn ensure_container(&mut self, session_key: &str) -> Result<SandboxContext, SandboxError> {
        // Maybe prune old containers
        self.maybe_prune().await;

        let container_name = self.container_name(session_key);
        let workspace_dir = self.workspace_dir(session_key);

        // Ensure workspace exists
        fs::create_dir_all(&workspace_dir).await?;

        let (exists, running) = self.container_state(&container_name).await;

        if !exists {
            self.create_container(&container_name, &workspace_dir, session_key).await?;
        } else if !running {
            let (_, stderr, code) = self.exec_docker(&["start", &container_name]).await?;
            if code != 0 {
                return Err(SandboxError::DockerError(stderr));
            }
        }

        // Update registry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.update_registry(SandboxRegistryEntry {
            container_name: container_name.clone(),
            session_key: session_key.to_string(),
            created_at_ms: now,
            last_used_at_ms: now,
            image: self.config.docker.image.clone(),
        })
        .await?;

        Ok(SandboxContext {
            enabled: true,
            session_key: session_key.to_string(),
            workspace_dir,
            container_name,
            container_workdir: self.config.docker.workdir.clone(),
            docker: self.config.docker.clone(),
            tools: self.config.tools.clone(),
        })
    }

    /// Execute command in container
    pub async fn exec_in_container(
        &self,
        container_name: &str,
        command: &str,
        workdir: Option<&str>,
    ) -> Result<(String, String, i32), SandboxError> {
        let mut args = vec!["exec", "-i"];

        if let Some(wd) = workdir {
            args.push("-w");
            args.push(wd);
        }

        args.push(container_name);
        args.push("sh");
        args.push("-c");
        args.push(command);

        let (stdout, stderr, code) = self.exec_docker(&args).await?;
        Ok((stdout, stderr, code))
    }

    /// Maybe prune old containers (rate limited)
    async fn maybe_prune(&mut self) {
        // Only prune every 5 minutes
        if self.last_prune.elapsed() < Duration::from_secs(5 * 60) {
            return;
        }
        self.last_prune = Instant::now();

        if let Err(e) = self.prune_containers().await {
            warn!(error = %e, "Failed to prune sandbox containers");
        }
    }

    /// Prune old containers
    pub async fn prune_containers(&self) -> Result<(), SandboxError> {
        let registry = self.read_registry().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let idle_ms = self.config.prune.idle_hours * 60 * 60 * 1000;
        let max_age_ms = self.config.prune.max_age_days * 24 * 60 * 60 * 1000;

        for entry in &registry.entries {
            let idle_time = now.saturating_sub(entry.last_used_at_ms);
            let age = now.saturating_sub(entry.created_at_ms);

            let should_prune = (self.config.prune.idle_hours > 0 && idle_time > idle_ms)
                || (self.config.prune.max_age_days > 0 && age > max_age_ms);

            if should_prune {
                debug!(container = %entry.container_name, "Pruning sandbox container");

                // Remove container (ignore errors)
                let _ = self.exec_docker(&["rm", "-f", &entry.container_name]).await;

                // Remove from registry
                let _ = self.remove_registry_entry(&entry.container_name).await;
            }
        }

        Ok(())
    }

    /// Stop and remove a container
    pub async fn remove_container(&self, container_name: &str) -> Result<(), SandboxError> {
        let (_, stderr, code) = self.exec_docker(&["rm", "-f", container_name]).await?;

        if code != 0 {
            return Err(SandboxError::DockerError(stderr));
        }

        self.remove_registry_entry(container_name).await?;
        Ok(())
    }

    /// List all sandbox containers
    pub async fn list_containers(&self) -> Result<Vec<SandboxRegistryEntry>, SandboxError> {
        let registry = self.read_registry().await;
        Ok(registry.entries)
    }
}

/// Sandbox errors
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("Docker image not found: {0}")]
    ImageNotFound(String),

    #[error("Docker error: {0}")]
    DockerError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_mode_default() {
        let mode = SandboxMode::default();
        assert_eq!(mode, SandboxMode::Off);
    }

    #[test]
    fn test_tool_policy_allow() {
        let policy = SandboxToolPolicy {
            allow: Some(vec!["bash".to_string(), "read".to_string()]),
            deny: None,
        };

        assert!(policy.is_tool_allowed("bash"));
        assert!(policy.is_tool_allowed("BASH")); // case insensitive
        assert!(policy.is_tool_allowed("read"));
        assert!(!policy.is_tool_allowed("write"));
    }

    #[test]
    fn test_tool_policy_deny() {
        let policy = SandboxToolPolicy {
            allow: None,
            deny: Some(vec!["browser".to_string()]),
        };

        assert!(policy.is_tool_allowed("bash"));
        assert!(!policy.is_tool_allowed("browser"));
        assert!(!policy.is_tool_allowed("BROWSER")); // case insensitive
    }

    #[test]
    fn test_tool_policy_deny_over_allow() {
        let policy = SandboxToolPolicy {
            allow: Some(vec!["browser".to_string()]),
            deny: Some(vec!["browser".to_string()]),
        };

        // Deny takes precedence
        assert!(!policy.is_tool_allowed("browser"));
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxRuntimeConfig::default();
        assert_eq!(config.mode, SandboxMode::Off);
        assert!(config.per_session);
        assert_eq!(config.docker.workdir, DEFAULT_SANDBOX_WORKDIR);
    }

    #[test]
    fn test_should_sandbox() {
        let config = SandboxRuntimeConfig {
            mode: SandboxMode::NonMain,
            ..Default::default()
        };
        let manager = SandboxManager::new(config, PathBuf::from("/tmp"));

        assert!(!manager.should_sandbox("main", "main"));
        assert!(manager.should_sandbox("user:123", "main"));

        let config_all = SandboxRuntimeConfig {
            mode: SandboxMode::All,
            ..Default::default()
        };
        let manager_all = SandboxManager::new(config_all, PathBuf::from("/tmp"));
        assert!(manager_all.should_sandbox("main", "main"));
    }
}
