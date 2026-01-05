//! Audio transcription module
//!
//! This module provides audio transcription via external commands.
//! Common backends include:
//! - OpenAI Whisper
//! - faster-whisper
//! - insanely-fast-whisper
//! - Local whisper.cpp

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use regex::Regex;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::{is_audio_mime, MediaContext, MediaError, MediaResult, TranscriptionResult};

/// Default timeout for transcription commands (45 seconds)
const DEFAULT_TIMEOUT_SECONDS: u64 = 45;

/// Maximum output buffer size (5MB)
const MAX_OUTPUT_BUFFER: usize = 5 * 1024 * 1024;

/// Transcription configuration
#[derive(Debug, Clone)]
pub struct TranscriptionConfig {
    /// Command template with placeholders
    /// Example: ["whisper", "--model", "base", "--output-format", "txt", "{MediaPath}"]
    pub command: Vec<String>,
    /// Timeout in seconds
    pub timeout_seconds: u64,
    /// Working directory for the command
    pub working_dir: Option<PathBuf>,
    /// Environment variables
    pub env: HashMap<String, String>,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            command: Vec::new(),
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            working_dir: None,
            env: HashMap::new(),
        }
    }
}

impl TranscriptionConfig {
    /// Create config from a command line
    pub fn from_command(command: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            command: command.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    /// Set timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Set working directory
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Audio transcriber
pub struct Transcriber {
    config: TranscriptionConfig,
    http_client: reqwest::Client,
}

impl Transcriber {
    /// Create a new transcriber with the given configuration
    pub fn new(config: TranscriptionConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// Check if transcription is configured
    pub fn is_configured(&self) -> bool {
        !self.config.command.is_empty()
    }

    /// Transcribe audio from a media context
    ///
    /// This handles:
    /// - Downloading remote audio files
    /// - Running the transcription command
    /// - Cleaning up temporary files
    pub async fn transcribe(&self, ctx: &MediaContext) -> MediaResult<TranscriptionResult> {
        if !self.is_configured() {
            return Err(MediaError::TranscriptionFailed(
                "Transcription not configured".to_string(),
            ));
        }

        // Check if it's audio
        if let Some(ref mime) = ctx.media_type {
            if !is_audio_mime(mime) {
                return Err(MediaError::InvalidMediaType(format!(
                    "Not an audio type: {}",
                    mime
                )));
            }
        }

        // Get or download the audio file
        let (media_path, temp_path) = self.resolve_media_path(ctx).await?;

        // Run transcription
        let result = self.run_transcription(&media_path, ctx).await;

        // Cleanup temp file if we created one
        if let Some(temp) = temp_path {
            if let Err(e) = fs::remove_file(&temp).await {
                warn!(path = %temp.display(), error = %e, "Failed to cleanup temp audio file");
            }
        }

        result
    }

    /// Resolve media path, downloading if necessary
    async fn resolve_media_path(
        &self,
        ctx: &MediaContext,
    ) -> MediaResult<(PathBuf, Option<PathBuf>)> {
        // Use local path if available
        if let Some(ref path) = ctx.media_path {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return Ok((path_buf, None));
            }
        }

        // Download from URL
        if let Some(ref url) = ctx.media_url {
            let temp_path = self.download_audio(url).await?;
            return Ok((temp_path.clone(), Some(temp_path)));
        }

        Err(MediaError::FileNotFound(
            "No media path or URL provided".to_string(),
        ))
    }

    /// Download audio file to a temporary location
    async fn download_audio(&self, url: &str) -> MediaResult<PathBuf> {
        debug!(url = url, "Downloading audio for transcription");

        let response = self.http_client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(MediaError::DownloadFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await?;

        // Create temp file with appropriate extension
        let temp_dir = std::env::temp_dir();
        let file_name = format!("edith-audio-{}.ogg", uuid::Uuid::new_v4());
        let temp_path = temp_dir.join(file_name);

        fs::write(&temp_path, &bytes).await?;

        let size_mb = bytes.len() as f64 / (1024.0 * 1024.0);
        debug!(
            path = %temp_path.display(),
            size_mb = format!("{:.2}", size_mb),
            "Downloaded audio for transcription"
        );

        Ok(temp_path)
    }

    /// Run the transcription command
    async fn run_transcription(
        &self,
        media_path: &Path,
        ctx: &MediaContext,
    ) -> MediaResult<TranscriptionResult> {
        // Apply template to command
        let argv = self.apply_templates(&self.config.command, media_path, ctx);

        if argv.is_empty() {
            return Err(MediaError::CommandFailed("Empty command".to_string()));
        }

        info!(
            command = argv.join(" "),
            "Running audio transcription command"
        );

        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let timeout = Duration::from_secs(self.config.timeout_seconds);

        let result = tokio::time::timeout(timeout, async {
            let child = cmd.spawn().map_err(|e| {
                MediaError::CommandFailed(format!("Failed to spawn command: {}", e))
            })?;

            let output = child.wait_with_output().await.map_err(|e| {
                MediaError::CommandFailed(format!("Command execution failed: {}", e))
            })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(MediaError::CommandFailed(format!(
                    "Command exited with {}: {}",
                    output.status,
                    stderr.trim()
                )));
            }

            // Limit output size
            let stdout = if output.stdout.len() > MAX_OUTPUT_BUFFER {
                &output.stdout[..MAX_OUTPUT_BUFFER]
            } else {
                &output.stdout
            };

            let text = String::from_utf8_lossy(stdout).trim().to_string();
            Ok::<_, MediaError>(text)
        })
        .await;

        match result {
            Ok(Ok(text)) => {
                if text.is_empty() {
                    Err(MediaError::TranscriptionFailed(
                        "Empty transcription result".to_string(),
                    ))
                } else {
                    info!(
                        length = text.len(),
                        "Audio transcription completed successfully"
                    );
                    Ok(TranscriptionResult::new(text))
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(MediaError::Timeout),
        }
    }

    /// Apply template substitutions to command arguments
    fn apply_templates(
        &self,
        templates: &[String],
        media_path: &Path,
        ctx: &MediaContext,
    ) -> Vec<String> {
        let media_path_str = media_path.to_string_lossy();

        templates
            .iter()
            .map(|template| {
                let mut result = template.clone();

                // Replace placeholders
                result = result.replace("{MediaPath}", &media_path_str);
                result = result.replace("{MEDIA_PATH}", &media_path_str);

                if let Some(ref url) = ctx.media_url {
                    result = result.replace("{MediaUrl}", url);
                    result = result.replace("{MEDIA_URL}", url);
                }

                if let Some(ref mime) = ctx.media_type {
                    result = result.replace("{MediaType}", mime);
                    result = result.replace("{MEDIA_TYPE}", mime);
                }

                if let Some(ref from) = ctx.from {
                    result = result.replace("{From}", from);
                    result = result.replace("{FROM}", from);
                }

                if let Some(ref body) = ctx.body {
                    result = result.replace("{Body}", body);
                    result = result.replace("{BODY}", body);
                }

                if let Some(ref surface) = ctx.surface {
                    result = result.replace("{Surface}", surface);
                    result = result.replace("{SURFACE}", surface);
                }

                result
            })
            .collect()
    }
}

/// Create a transcriber from Edith config
pub fn create_transcriber_from_config(config: &edith_config::EdithConfig) -> Option<Transcriber> {
    let transcribe_config = config.routing.as_ref()?.transcribe_audio.as_ref()?;

    let command = &transcribe_config.command;
    if command.is_empty() {
        return None;
    }

    let timeout = transcribe_config.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);

    let config = TranscriptionConfig {
        command: command.clone(),
        timeout_seconds: timeout,
        working_dir: None,
        env: HashMap::new(),
    };

    Some(Transcriber::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcription_config_default() {
        let config = TranscriptionConfig::default();
        assert!(config.command.is_empty());
        assert_eq!(config.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn test_transcription_config_builder() {
        let config = TranscriptionConfig::from_command(["whisper", "{MediaPath}"])
            .with_timeout(60)
            .with_working_dir("/tmp")
            .with_env("OPENAI_API_KEY", "test");

        assert_eq!(config.command.len(), 2);
        assert_eq!(config.timeout_seconds, 60);
        assert!(config.working_dir.is_some());
        assert!(config.env.contains_key("OPENAI_API_KEY"));
    }

    #[test]
    fn test_transcriber_is_configured() {
        let empty = Transcriber::new(TranscriptionConfig::default());
        assert!(!empty.is_configured());

        let configured =
            Transcriber::new(TranscriptionConfig::from_command(["whisper", "{MediaPath}"]));
        assert!(configured.is_configured());
    }

    #[test]
    fn test_apply_templates() {
        let transcriber =
            Transcriber::new(TranscriptionConfig::from_command(["echo", "{MediaPath}"]));

        let ctx = MediaContext {
            media_url: Some("https://example.com/audio.ogg".to_string()),
            media_path: Some("/tmp/test.ogg".to_string()),
            media_type: Some("audio/ogg".to_string()),
            from: Some("user@example.com".to_string()),
            body: Some("Hello".to_string()),
            surface: Some("whatsapp".to_string()),
        };

        let templates = vec![
            "{MediaPath}".to_string(),
            "--url={MediaUrl}".to_string(),
            "--from={From}".to_string(),
        ];

        let result = transcriber.apply_templates(&templates, Path::new("/tmp/audio.ogg"), &ctx);

        assert_eq!(result[0], "/tmp/audio.ogg");
        assert_eq!(result[1], "--url=https://example.com/audio.ogg");
        assert_eq!(result[2], "--from=user@example.com");
    }
}
