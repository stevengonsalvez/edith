//! Browser automation client
//!
//! HTTP client for controlling Chrome/Chromium via a browser control server.
//! The server exposes CDP (Chrome DevTools Protocol) functionality through a REST API.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Default browser control URL
pub const DEFAULT_BROWSER_CONTROL_URL: &str = "http://localhost:9222";

/// Default timeout for browser operations
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Browser client errors
#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Browser control server unreachable: {0}")]
    Unreachable(String),

    #[error("Browser operation failed: {0}")]
    OperationFailed(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Timeout after {0}ms")]
    Timeout(u64),
}

pub type BrowserResult<T> = Result<T, BrowserError>;

/// Browser status response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub enabled: bool,
    pub control_url: String,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdp_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdp_http: Option<bool>,
    pub pid: Option<i64>,
    pub cdp_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdp_url: Option<String>,
    pub chosen_browser: Option<String>,
    pub user_data_dir: Option<String>,
    pub color: String,
    pub headless: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_sandbox: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    pub attach_only: bool,
}

/// Browser tab information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub target_id: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tab_type: Option<String>,
}

/// Accessibility tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotAriaNode {
    #[serde(rename = "ref")]
    pub node_ref: String,
    pub role: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_dom_node_id: Option<i64>,
    pub depth: i32,
}

/// Snapshot format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotFormat {
    Aria,
    Ai,
}

/// Snapshot result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotResult {
    pub ok: bool,
    pub format: SnapshotFormat,
    pub target_id: String,
    pub url: String,
    /// For aria format, contains nodes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<SnapshotAriaNode>>,
    /// For ai format, contains text snapshot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

/// Browser action request types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BrowserAction {
    Click {
        #[serde(rename = "ref")]
        node_ref: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        double_click: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        button: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        modifiers: Option<Vec<String>>,
    },
    Type {
        #[serde(rename = "ref")]
        node_ref: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        submit: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        slowly: Option<bool>,
    },
    Press {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Hover {
        #[serde(rename = "ref")]
        node_ref: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Drag {
        start_ref: String,
        end_ref: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Select {
        #[serde(rename = "ref")]
        node_ref: String,
        values: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Fill {
        fields: Vec<FormField>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Resize {
        width: u32,
        height: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Wait {
        #[serde(skip_serializing_if = "Option::is_none")]
        time_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text_gone: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Evaluate {
        #[serde(rename = "fn")]
        function: String,
        #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
        node_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
    Close {
        #[serde(skip_serializing_if = "Option::is_none")]
        target_id: Option<String>,
    },
}

/// Form field for fill action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    #[serde(rename = "ref")]
    pub node_ref: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// Browser action response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserActionResponse {
    pub ok: bool,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Screenshot options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_page: Option<bool>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub node_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub image_type: Option<String>,
}

/// Screenshot result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotResult {
    pub ok: bool,
    pub path: String,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Dialog arm options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogOptions {
    pub accept: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// File chooser options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChooserOptions {
    pub paths: Vec<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub node_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Reset profile result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetProfileResult {
    pub ok: bool,
    pub moved: bool,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// Browser configuration
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Control server URL
    pub control_url: String,
    /// Default timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            control_url: DEFAULT_BROWSER_CONTROL_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

/// Browser control client
pub struct BrowserClient {
    config: BrowserConfig,
    http: Client,
}

impl BrowserClient {
    /// Create a new browser client
    pub fn new(config: BrowserConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, http }
    }

    /// Create with default configuration
    pub fn with_url(url: impl Into<String>) -> Self {
        Self::new(BrowserConfig {
            control_url: url.into(),
            ..Default::default()
        })
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.config.control_url.trim_end_matches('/')
    }

    /// Make a GET request
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> BrowserResult<T> {
        let url = format!("{}{}", self.base_url(), path);
        debug!(url = %url, "Browser GET request");

        let response = self.http.get(&url).send().await.map_err(|e| {
            if e.is_connect() {
                BrowserError::Unreachable(format!("{} ({})", url, e))
            } else if e.is_timeout() {
                BrowserError::Timeout(self.config.timeout_ms)
            } else {
                BrowserError::Http(e)
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(BrowserError::OperationFailed(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BrowserError::InvalidResponse(e.to_string()))
    }

    /// Make a POST request
    async fn post<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> BrowserResult<T> {
        let url = format!("{}{}", self.base_url(), path);
        debug!(url = %url, "Browser POST request");

        let mut request = self.http.post(&url);
        if let Some(b) = body {
            request = request.json(b);
        }

        let response = request.send().await.map_err(|e| {
            if e.is_connect() {
                BrowserError::Unreachable(format!("{} ({})", url, e))
            } else if e.is_timeout() {
                BrowserError::Timeout(self.config.timeout_ms)
            } else {
                BrowserError::Http(e)
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(BrowserError::OperationFailed(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BrowserError::InvalidResponse(e.to_string()))
    }

    /// Make a DELETE request
    async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> BrowserResult<T> {
        let url = format!("{}{}", self.base_url(), path);
        debug!(url = %url, "Browser DELETE request");

        let response = self.http.delete(&url).send().await.map_err(|e| {
            if e.is_connect() {
                BrowserError::Unreachable(format!("{} ({})", url, e))
            } else if e.is_timeout() {
                BrowserError::Timeout(self.config.timeout_ms)
            } else {
                BrowserError::Http(e)
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(BrowserError::OperationFailed(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BrowserError::InvalidResponse(e.to_string()))
    }

    // =========================================================================
    // Status & Control
    // =========================================================================

    /// Get browser status
    pub async fn status(&self) -> BrowserResult<BrowserStatus> {
        self.get("/").await
    }

    /// Start the browser
    pub async fn start(&self) -> BrowserResult<()> {
        let _: serde_json::Value = self.post("/start", None::<&()>).await?;
        info!("Browser started");
        Ok(())
    }

    /// Stop the browser
    pub async fn stop(&self) -> BrowserResult<()> {
        let _: serde_json::Value = self.post("/stop", None::<&()>).await?;
        info!("Browser stopped");
        Ok(())
    }

    /// Reset browser profile
    pub async fn reset_profile(&self) -> BrowserResult<ResetProfileResult> {
        self.post("/reset-profile", None::<&()>).await
    }

    // =========================================================================
    // Tab Management
    // =========================================================================

    /// List all browser tabs
    pub async fn tabs(&self) -> BrowserResult<Vec<BrowserTab>> {
        #[derive(Deserialize)]
        struct TabsResponse {
            running: bool,
            tabs: Vec<BrowserTab>,
        }

        let response: TabsResponse = self.get("/tabs").await?;
        Ok(response.tabs)
    }

    /// Open a new tab
    pub async fn open_tab(&self, url: &str) -> BrowserResult<BrowserTab> {
        #[derive(Serialize)]
        struct OpenTabRequest<'a> {
            url: &'a str,
        }

        self.post("/tabs/open", Some(&OpenTabRequest { url })).await
    }

    /// Focus a tab
    pub async fn focus_tab(&self, target_id: &str) -> BrowserResult<()> {
        #[derive(Serialize)]
        struct FocusRequest<'a> {
            #[serde(rename = "targetId")]
            target_id: &'a str,
        }

        let _: serde_json::Value = self
            .post("/tabs/focus", Some(&FocusRequest { target_id }))
            .await?;
        Ok(())
    }

    /// Close a tab
    pub async fn close_tab(&self, target_id: &str) -> BrowserResult<()> {
        let path = format!("/tabs/{}", urlencoding::encode(target_id));
        let _: serde_json::Value = self.delete(&path).await?;
        Ok(())
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    /// Navigate to a URL
    pub async fn navigate(&self, url: &str, target_id: Option<&str>) -> BrowserResult<BrowserTab> {
        #[derive(Serialize)]
        struct NavigateRequest<'a> {
            url: &'a str,
            #[serde(rename = "targetId", skip_serializing_if = "Option::is_none")]
            target_id: Option<&'a str>,
        }

        self.post("/navigate", Some(&NavigateRequest { url, target_id }))
            .await
    }

    // =========================================================================
    // Snapshots
    // =========================================================================

    /// Take a snapshot (accessibility tree or AI format)
    pub async fn snapshot(
        &self,
        format: SnapshotFormat,
        target_id: Option<&str>,
        limit: Option<u32>,
    ) -> BrowserResult<SnapshotResult> {
        let format_str = match format {
            SnapshotFormat::Aria => "aria",
            SnapshotFormat::Ai => "ai",
        };

        let mut query = format!("format={}", format_str);
        if let Some(id) = target_id {
            query.push_str(&format!("&targetId={}", urlencoding::encode(id)));
        }
        if let Some(l) = limit {
            query.push_str(&format!("&limit={}", l));
        }

        self.get(&format!("/snapshot?{}", query)).await
    }

    /// Take a screenshot
    pub async fn screenshot(&self, opts: ScreenshotOptions) -> BrowserResult<ScreenshotResult> {
        self.post("/screenshot", Some(&opts)).await
    }

    // =========================================================================
    // Actions
    // =========================================================================

    /// Perform a browser action
    pub async fn act(&self, action: BrowserAction) -> BrowserResult<BrowserActionResponse> {
        self.post("/act", Some(&action)).await
    }

    /// Click on an element
    pub async fn click(&self, node_ref: &str, target_id: Option<&str>) -> BrowserResult<BrowserActionResponse> {
        self.act(BrowserAction::Click {
            node_ref: node_ref.to_string(),
            target_id: target_id.map(String::from),
            double_click: None,
            button: None,
            modifiers: None,
        })
        .await
    }

    /// Type text into an element
    pub async fn type_text(
        &self,
        node_ref: &str,
        text: &str,
        target_id: Option<&str>,
    ) -> BrowserResult<BrowserActionResponse> {
        self.act(BrowserAction::Type {
            node_ref: node_ref.to_string(),
            text: text.to_string(),
            target_id: target_id.map(String::from),
            submit: None,
            slowly: None,
        })
        .await
    }

    /// Press a key
    pub async fn press_key(&self, key: &str, target_id: Option<&str>) -> BrowserResult<BrowserActionResponse> {
        self.act(BrowserAction::Press {
            key: key.to_string(),
            target_id: target_id.map(String::from),
        })
        .await
    }

    /// Hover over an element
    pub async fn hover(&self, node_ref: &str, target_id: Option<&str>) -> BrowserResult<BrowserActionResponse> {
        self.act(BrowserAction::Hover {
            node_ref: node_ref.to_string(),
            target_id: target_id.map(String::from),
        })
        .await
    }

    /// Wait for a condition
    pub async fn wait(
        &self,
        time_ms: Option<u64>,
        text: Option<&str>,
        text_gone: Option<&str>,
        target_id: Option<&str>,
    ) -> BrowserResult<BrowserActionResponse> {
        self.act(BrowserAction::Wait {
            time_ms,
            text: text.map(String::from),
            text_gone: text_gone.map(String::from),
            target_id: target_id.map(String::from),
        })
        .await
    }

    /// Evaluate JavaScript
    pub async fn evaluate(
        &self,
        function: &str,
        node_ref: Option<&str>,
        target_id: Option<&str>,
    ) -> BrowserResult<BrowserActionResponse> {
        self.act(BrowserAction::Evaluate {
            function: function.to_string(),
            node_ref: node_ref.map(String::from),
            target_id: target_id.map(String::from),
        })
        .await
    }

    // =========================================================================
    // Hooks
    // =========================================================================

    /// Arm dialog handler (for alert, confirm, prompt)
    pub async fn arm_dialog(&self, opts: DialogOptions) -> BrowserResult<()> {
        let _: serde_json::Value = self.post("/hooks/dialog", Some(&opts)).await?;
        Ok(())
    }

    /// Arm file chooser
    pub async fn arm_file_chooser(&self, opts: FileChooserOptions) -> BrowserResult<()> {
        let _: serde_json::Value = self.post("/hooks/file-chooser", Some(&opts)).await?;
        Ok(())
    }
}

/// Create a browser client from Edith config
pub fn create_browser_client(config: &edith_config::EdithConfig) -> Option<BrowserClient> {
    let browser_config = config.browser.as_ref()?;

    if !browser_config.enabled.unwrap_or(false) {
        return None;
    }

    let control_url = browser_config
        .control_url
        .clone()
        .unwrap_or_else(|| DEFAULT_BROWSER_CONTROL_URL.to_string());

    Some(BrowserClient::new(BrowserConfig {
        control_url,
        timeout_ms: DEFAULT_TIMEOUT_MS,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_config_default() {
        let config = BrowserConfig::default();
        assert_eq!(config.control_url, DEFAULT_BROWSER_CONTROL_URL);
        assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn test_browser_action_serialization() {
        let action = BrowserAction::Click {
            node_ref: "button1".to_string(),
            target_id: Some("tab1".to_string()),
            double_click: Some(true),
            button: None,
            modifiers: None,
        };

        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"kind\":\"click\""));
        assert!(json.contains("\"ref\":\"button1\""));
    }

    #[test]
    fn test_screenshot_options_default() {
        let opts = ScreenshotOptions::default();
        assert!(opts.target_id.is_none());
        assert!(opts.full_page.is_none());
    }

    #[test]
    fn test_snapshot_format() {
        let aria = SnapshotFormat::Aria;
        let ai = SnapshotFormat::Ai;

        let aria_json = serde_json::to_string(&aria).unwrap();
        let ai_json = serde_json::to_string(&ai).unwrap();

        assert_eq!(aria_json, "\"aria\"");
        assert_eq!(ai_json, "\"ai\"");
    }
}
