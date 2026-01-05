//! Media type definitions

use serde::{Deserialize, Serialize};

/// Media kind classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Document,
    Sticker,
    Voice,
}

impl MediaKind {
    /// Detect media kind from MIME type
    pub fn from_mime(mime: &str) -> Option<Self> {
        let lower = mime.to_lowercase();
        if lower.starts_with("image/") {
            Some(Self::Image)
        } else if lower.starts_with("video/") {
            Some(Self::Video)
        } else if lower.starts_with("audio/") {
            // Voice notes often have ogg/opus codec
            if lower.contains("ogg") || lower.contains("opus") {
                Some(Self::Voice)
            } else {
                Some(Self::Audio)
            }
        } else if lower.starts_with("application/pdf")
            || lower.starts_with("text/")
            || lower.contains("document")
            || lower.contains("spreadsheet")
            || lower.contains("presentation")
        {
            Some(Self::Document)
        } else if lower.contains("sticker") || lower.contains("webp") {
            Some(Self::Sticker)
        } else {
            Some(Self::Document)
        }
    }

    /// Check if this is an audio type (including voice)
    pub fn is_audio(&self) -> bool {
        matches!(self, Self::Audio | Self::Voice)
    }
}

/// Stored media metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    /// Unique media ID
    pub id: String,
    /// Original file name
    pub file_name: Option<String>,
    /// MIME type
    pub mime_type: String,
    /// Media kind
    pub kind: MediaKind,
    /// File size in bytes
    pub size: u64,
    /// Storage path (local file path or URL)
    pub path: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Optional caption
    pub caption: Option<String>,
    /// Transcription (for audio)
    pub transcription: Option<String>,
}

/// Transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Language detected (if any)
    pub language: Option<String>,
    /// Duration of the audio in seconds
    pub duration_seconds: Option<f64>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: Option<f64>,
}

impl TranscriptionResult {
    /// Create a simple transcription result with just text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
            duration_seconds: None,
            confidence: None,
        }
    }
}

/// Message context for templating
#[derive(Debug, Clone, Default)]
pub struct MediaContext {
    /// Media URL (remote)
    pub media_url: Option<String>,
    /// Local media path
    pub media_path: Option<String>,
    /// Media MIME type
    pub media_type: Option<String>,
    /// Sender identifier
    pub from: Option<String>,
    /// Message body
    pub body: Option<String>,
    /// Surface (platform)
    pub surface: Option<String>,
}

/// Check if a MIME type represents audio
pub fn is_audio_mime(mime: &str) -> bool {
    mime.to_lowercase().starts_with("audio")
}

/// Check if a MIME type represents video
pub fn is_video_mime(mime: &str) -> bool {
    mime.to_lowercase().starts_with("video")
}

/// Check if a MIME type represents an image
pub fn is_image_mime(mime: &str) -> bool {
    mime.to_lowercase().starts_with("image")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_kind_from_mime() {
        assert_eq!(MediaKind::from_mime("image/png"), Some(MediaKind::Image));
        assert_eq!(MediaKind::from_mime("video/mp4"), Some(MediaKind::Video));
        assert_eq!(MediaKind::from_mime("audio/mpeg"), Some(MediaKind::Audio));
        assert_eq!(
            MediaKind::from_mime("audio/ogg; codecs=opus"),
            Some(MediaKind::Voice)
        );
        assert_eq!(
            MediaKind::from_mime("application/pdf"),
            Some(MediaKind::Document)
        );
    }

    #[test]
    fn test_is_audio() {
        assert!(is_audio_mime("audio/mpeg"));
        assert!(is_audio_mime("audio/ogg"));
        assert!(!is_audio_mime("video/mp4"));
        assert!(!is_audio_mime("image/png"));
    }

    #[test]
    fn test_transcription_result() {
        let result = TranscriptionResult::new("Hello world");
        assert_eq!(result.text, "Hello world");
        assert!(result.language.is_none());
    }
}
