//! Skills system for Edith
//!
//! Skills are configurable capabilities that can be enabled/disabled based on:
//! - OS platform
//! - Required binaries (bins)
//! - Required environment variables
//! - Config paths
//!
//! Skills are loaded from multiple sources with precedence:
//! extra < bundled < managed < workspace

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Skill install specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallSpec {
    pub id: Option<String>,
    #[serde(rename = "kind")]
    pub install_kind: InstallKind,
    pub label: Option<String>,
    pub bins: Option<Vec<String>>,
    pub formula: Option<String>,
    pub package: Option<String>,
    pub module: Option<String>,
}

/// Install method kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind {
    Brew,
    Node,
    Go,
    Uv,
}

/// Requirements for a skill to be available
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRequirements {
    /// All these binaries must exist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bins: Option<Vec<String>>,
    /// At least one of these binaries must exist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any_bins: Option<Vec<String>>,
    /// All these environment variables must be set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,
    /// All these config paths must be truthy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Vec<String>>,
}

/// Edith-specific skill metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    /// Always include this skill
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always: Option<bool>,
    /// Config key for this skill
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_key: Option<String>,
    /// Primary environment variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_env: Option<String>,
    /// Emoji for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// Homepage URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Supported operating systems
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<Vec<String>>,
    /// Requirements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<SkillRequirements>,
    /// Install specifications
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install: Option<Vec<SkillInstallSpec>>,
}

/// Skill source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSource {
    /// Bundled with edith
    EdithBundled,
    /// Managed by edith (in config dir)
    EdithManaged,
    /// Workspace-specific
    EdithWorkspace,
    /// Extra directories
    EdithExtra,
}

/// A loaded skill
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Skill name (from filename)
    pub name: String,
    /// Source of the skill
    pub source: SkillSource,
    /// Path to the skill file
    pub file_path: PathBuf,
    /// Raw content
    pub content: String,
    /// Parsed frontmatter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<HashMap<String, String>>,
    /// Edith metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SkillMetadata>,
}

/// Skill configuration entry (from skills.yaml or config)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntryConfig {
    /// Whether skill is enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// API key for the skill
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Environment variable overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

/// Skills install preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsInstallPreferences {
    /// Prefer Homebrew for installation
    pub prefer_brew: bool,
    /// Node package manager
    pub node_manager: String,
}

impl Default for SkillsInstallPreferences {
    fn default() -> Self {
        Self {
            prefer_brew: true,
            node_manager: "npm".to_string(),
        }
    }
}

/// Skill snapshot for prompt generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSnapshot {
    /// Formatted prompt
    pub prompt: String,
    /// List of active skills
    pub skills: Vec<SkillInfo>,
    /// Resolved skill objects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_skills: Option<Vec<Skill>>,
}

/// Skill info for snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_env: Option<String>,
}

/// Parse YAML frontmatter from skill content
pub fn parse_frontmatter(content: &str) -> Option<HashMap<String, String>> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.starts_with("---") {
        return None;
    }

    let end_index = normalized[3..].find("\n---")?;
    let block = &normalized[4..end_index + 3];

    let mut frontmatter = HashMap::new();
    for line in block.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = strip_quotes(value.trim());
            if !key.is_empty() && !value.is_empty() {
                frontmatter.insert(key.to_string(), value.to_string());
            }
        }
    }

    Some(frontmatter)
}

/// Strip quotes from a string value
fn strip_quotes(value: &str) -> String {
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// Parse skill metadata from frontmatter
pub fn parse_skill_metadata(frontmatter: &HashMap<String, String>) -> Option<SkillMetadata> {
    let raw = frontmatter.get("metadata")?;

    // The metadata field contains JSON like: {"edith": {...}}
    #[derive(Deserialize)]
    struct MetadataWrapper {
        edith: Option<SkillMetadata>,
        clawdis: Option<SkillMetadata>,
    }

    let parsed: MetadataWrapper = serde_json::from_str(raw).ok()?;
    // Support both "edith" and "clawdis" keys for compatibility
    parsed.edith.or(parsed.clawdis)
}

/// Check if a binary exists in PATH
pub fn has_binary(bin: &str) -> bool {
    if let Some(path_env) = env::var_os("PATH") {
        for dir in env::split_paths(&path_env) {
            let candidate = dir.join(bin);
            if candidate.is_file() {
                // Check if executable
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = fs::metadata(&candidate) {
                        if meta.permissions().mode() & 0o111 != 0 {
                            return true;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Get current runtime platform
pub fn get_runtime_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "darwin"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "windows")]
    {
        "win32"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "unknown"
    }
}

/// Load skills from a directory
pub fn load_skills_from_dir(dir: &Path, source: SkillSource) -> Vec<Skill> {
    let mut skills = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return skills;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            warn!(dir = %dir.display(), error = %err, "Failed to read skills directory");
            return skills;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "txt" {
                    if let Some(skill) = load_skill(&path, source) {
                        skills.push(skill);
                    }
                }
            }
        }
    }

    debug!(dir = %dir.display(), count = skills.len(), "Loaded skills from directory");
    skills
}

/// Load a single skill file
fn load_skill(path: &Path, source: SkillSource) -> Option<Skill> {
    let name = path.file_stem()?.to_str()?.to_string();
    let content = fs::read_to_string(path).ok()?;

    let frontmatter = parse_frontmatter(&content);
    let metadata = frontmatter
        .as_ref()
        .and_then(|fm| parse_skill_metadata(fm));

    Some(Skill {
        name,
        source,
        file_path: path.to_path_buf(),
        content,
        frontmatter,
        metadata,
    })
}

/// Skill manager for loading and filtering skills
pub struct SkillManager {
    /// Config directory
    config_dir: PathBuf,
    /// Bundled skills directory
    bundled_dir: Option<PathBuf>,
    /// Extra skill directories
    extra_dirs: Vec<PathBuf>,
    /// Skill configurations
    skill_configs: HashMap<String, SkillEntryConfig>,
    /// Allow list for bundled skills
    allow_bundled: Option<Vec<String>>,
}

impl SkillManager {
    /// Create a new skill manager
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            bundled_dir: None,
            extra_dirs: Vec::new(),
            skill_configs: HashMap::new(),
            allow_bundled: None,
        }
    }

    /// Set bundled skills directory
    pub fn with_bundled_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.bundled_dir = Some(dir.into());
        self
    }

    /// Add extra skills directory
    pub fn with_extra_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.extra_dirs.push(dir.into());
        self
    }

    /// Set skill configurations
    pub fn with_skill_configs(mut self, configs: HashMap<String, SkillEntryConfig>) -> Self {
        self.skill_configs = configs;
        self
    }

    /// Set allow list for bundled skills
    pub fn with_allow_bundled(mut self, allow: Vec<String>) -> Self {
        self.allow_bundled = Some(allow);
        self
    }

    /// Load all skills from configured directories
    pub fn load_skills(&self, workspace_dir: &Path) -> Vec<Skill> {
        let mut skills_map: HashMap<String, Skill> = HashMap::new();

        // Load extra skills (lowest precedence)
        for dir in &self.extra_dirs {
            for skill in load_skills_from_dir(dir, SkillSource::EdithExtra) {
                skills_map.insert(skill.name.clone(), skill);
            }
        }

        // Load bundled skills
        if let Some(ref bundled) = self.bundled_dir {
            for skill in load_skills_from_dir(bundled, SkillSource::EdithBundled) {
                skills_map.insert(skill.name.clone(), skill);
            }
        }

        // Load managed skills
        let managed_dir = self.config_dir.join("skills");
        for skill in load_skills_from_dir(&managed_dir, SkillSource::EdithManaged) {
            skills_map.insert(skill.name.clone(), skill);
        }

        // Load workspace skills (highest precedence)
        let workspace_skills = workspace_dir.join("skills");
        for skill in load_skills_from_dir(&workspace_skills, SkillSource::EdithWorkspace) {
            skills_map.insert(skill.name.clone(), skill);
        }

        skills_map.into_values().collect()
    }

    /// Filter skills based on requirements
    pub fn filter_skills(&self, skills: Vec<Skill>, edith_config: Option<&serde_json::Value>) -> Vec<Skill> {
        skills
            .into_iter()
            .filter(|skill| self.should_include_skill(skill, edith_config))
            .collect()
    }

    /// Check if a skill should be included
    fn should_include_skill(&self, skill: &Skill, edith_config: Option<&serde_json::Value>) -> bool {
        let skill_key = skill
            .metadata
            .as_ref()
            .and_then(|m| m.skill_key.clone())
            .unwrap_or_else(|| skill.name.clone());

        // Check if explicitly disabled
        if let Some(config) = self.skill_configs.get(&skill_key) {
            if config.enabled == Some(false) {
                return false;
            }
        }

        // Check bundled allowlist
        if skill.source == SkillSource::EdithBundled {
            if let Some(ref allow) = self.allow_bundled {
                if !allow.contains(&skill_key) && !allow.contains(&skill.name) {
                    return false;
                }
            }
        }

        // Check metadata requirements
        if let Some(ref meta) = skill.metadata {
            // Check OS
            if let Some(ref os_list) = meta.os {
                if !os_list.is_empty() {
                    let current = get_runtime_platform();
                    if !os_list.iter().any(|os| os == current) {
                        return false;
                    }
                }
            }

            // Always include if always=true
            if meta.always == Some(true) {
                return true;
            }

            // Check requirements
            if let Some(ref requires) = meta.requires {
                // Check required binaries
                if let Some(ref bins) = requires.bins {
                    for bin in bins {
                        if !has_binary(bin) {
                            return false;
                        }
                    }
                }

                // Check any required binaries
                if let Some(ref any_bins) = requires.any_bins {
                    if !any_bins.is_empty() && !any_bins.iter().any(|bin| has_binary(bin)) {
                        return false;
                    }
                }

                // Check required env vars
                if let Some(ref env_vars) = requires.env {
                    let skill_config = self.skill_configs.get(&skill_key);
                    for env_name in env_vars {
                        // Check environment
                        if env::var(env_name).is_ok() {
                            continue;
                        }
                        // Check skill config env
                        if let Some(cfg) = skill_config {
                            if let Some(ref env_map) = cfg.env {
                                if env_map.contains_key(env_name) {
                                    continue;
                                }
                            }
                            // Check apiKey for primaryEnv
                            if cfg.api_key.is_some() {
                                if let Some(ref primary) = meta.primary_env {
                                    if primary == env_name {
                                        continue;
                                    }
                                }
                            }
                        }
                        return false;
                    }
                }

                // Check required config paths
                if let Some(ref config_paths) = requires.config {
                    for path in config_paths {
                        if !is_config_path_truthy(edith_config, path) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// Generate skill snapshot
    pub fn build_snapshot(&self, workspace_dir: &Path, edith_config: Option<&serde_json::Value>) -> SkillSnapshot {
        let all_skills = self.load_skills(workspace_dir);
        let filtered = self.filter_skills(all_skills, edith_config);

        let skill_infos: Vec<SkillInfo> = filtered
            .iter()
            .map(|s| SkillInfo {
                name: s.name.clone(),
                primary_env: s.metadata.as_ref().and_then(|m| m.primary_env.clone()),
            })
            .collect();

        let prompt = format_skills_for_prompt(&filtered);

        SkillSnapshot {
            prompt,
            skills: skill_infos,
            resolved_skills: Some(filtered),
        }
    }
}

/// Check if a config path is truthy
fn is_config_path_truthy(config: Option<&serde_json::Value>, path: &str) -> bool {
    // Default values for config paths
    let defaults: &[(&str, bool)] = &[("browser.enabled", true)];

    if let Some(cfg) = config {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = cfg;
        for part in &parts {
            match current.get(*part) {
                Some(v) => current = v,
                None => {
                    // Check default
                    for (p, v) in defaults {
                        if *p == path {
                            return *v;
                        }
                    }
                    return false;
                }
            }
        }
        is_truthy(current)
    } else {
        // Check default
        for (p, v) in defaults {
            if *p == path {
                return *v;
            }
        }
        false
    }
}

/// Check if a JSON value is truthy
fn is_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        serde_json::Value::String(s) => !s.trim().is_empty(),
        serde_json::Value::Array(_) => true,
        serde_json::Value::Object(_) => true,
    }
}

/// Format skills for LLM prompt
pub fn format_skills_for_prompt(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut prompt = String::from("## Available Skills\n\n");

    for skill in skills {
        // Skip frontmatter and get content
        let content = if skill.content.starts_with("---") {
            // Find end of frontmatter
            if let Some(end) = skill.content[3..].find("\n---") {
                skill.content[end + 7..].trim()
            } else {
                skill.content.trim()
            }
        } else {
            skill.content.trim()
        };

        prompt.push_str(&format!("### {}\n\n{}\n\n", skill.name, content));
    }

    prompt
}

/// Apply environment variable overrides from skill configs
pub fn apply_skill_env_overrides(
    skills: &[Skill],
    skill_configs: &HashMap<String, SkillEntryConfig>,
) -> Vec<(String, Option<String>)> {
    let mut updates = Vec::new();

    for skill in skills {
        let skill_key = skill
            .metadata
            .as_ref()
            .and_then(|m| m.skill_key.clone())
            .unwrap_or_else(|| skill.name.clone());

        if let Some(config) = skill_configs.get(&skill_key) {
            // Apply env overrides
            if let Some(ref env_map) = config.env {
                for (key, value) in env_map {
                    if env::var(key).is_err() {
                        let prev = env::var(key).ok();
                        env::set_var(key, value);
                        updates.push((key.clone(), prev));
                    }
                }
            }

            // Apply apiKey to primaryEnv
            if let Some(ref api_key) = config.api_key {
                if let Some(ref meta) = skill.metadata {
                    if let Some(ref primary) = meta.primary_env {
                        if env::var(primary).is_err() {
                            let prev = env::var(primary).ok();
                            env::set_var(primary, api_key);
                            updates.push((primary.clone(), prev));
                        }
                    }
                }
            }
        }
    }

    updates
}

/// Restore environment variables after skill execution
pub fn restore_env(updates: Vec<(String, Option<String>)>) {
    for (key, prev) in updates {
        match prev {
            Some(value) => env::set_var(&key, value),
            None => env::remove_var(&key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test-skill
version: "1.0"
---

# Test Skill Content
"#;
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.get("name"), Some(&"test-skill".to_string()));
        assert_eq!(fm.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just Content\n\nNo frontmatter here";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn test_get_runtime_platform() {
        let platform = get_runtime_platform();
        #[cfg(target_os = "macos")]
        assert_eq!(platform, "darwin");
        #[cfg(target_os = "linux")]
        assert_eq!(platform, "linux");
        #[cfg(target_os = "windows")]
        assert_eq!(platform, "win32");
    }

    #[test]
    fn test_has_binary() {
        // These should exist on most systems
        #[cfg(unix)]
        {
            assert!(has_binary("sh"));
        }
        #[cfg(windows)]
        {
            assert!(has_binary("cmd.exe"));
        }
        // This shouldn't exist
        assert!(!has_binary("definitely_not_a_real_binary_12345"));
    }

    #[test]
    fn test_is_truthy() {
        assert!(!is_truthy(&serde_json::Value::Null));
        assert!(is_truthy(&serde_json::Value::Bool(true)));
        assert!(!is_truthy(&serde_json::Value::Bool(false)));
        assert!(is_truthy(&serde_json::json!(1)));
        assert!(!is_truthy(&serde_json::json!(0)));
        assert!(is_truthy(&serde_json::json!("hello")));
        assert!(!is_truthy(&serde_json::json!("")));
    }

    #[test]
    fn test_format_skills_empty() {
        let skills: Vec<Skill> = vec![];
        assert!(format_skills_for_prompt(&skills).is_empty());
    }

    #[test]
    fn test_skill_manager_new() {
        let manager = SkillManager::new(PathBuf::from("/tmp/.edith"));
        assert!(manager.bundled_dir.is_none());
        assert!(manager.extra_dirs.is_empty());
    }
}
