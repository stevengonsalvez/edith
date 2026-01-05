//! Session store persistence

use parking_lot::RwLock;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{debug, error, info, warn};

use crate::{SessionEntry, SessionError, SessionResult, SessionStore};

/// Persistent session store
#[derive(Clone)]
pub struct PersistentSessionStore {
    inner: Arc<PersistentSessionStoreInner>,
}

struct PersistentSessionStoreInner {
    /// In-memory store
    store: RwLock<SessionStore>,
    /// Path to the store file
    store_path: PathBuf,
    /// Path to transcripts directory
    transcripts_dir: PathBuf,
}

impl PersistentSessionStore {
    /// Create a new persistent session store
    pub fn new(store_path: impl Into<PathBuf>, transcripts_dir: impl Into<PathBuf>) -> Self {
        let store_path = store_path.into();
        let transcripts_dir = transcripts_dir.into();

        // Load existing store if present
        let store = load_store_from_file(&store_path).unwrap_or_default();

        Self {
            inner: Arc::new(PersistentSessionStoreInner {
                store: RwLock::new(store),
                store_path,
                transcripts_dir,
            }),
        }
    }

    /// Create from config
    pub fn from_config(config: &edith_config::EdithConfig) -> Self {
        let store_path = config
            .session
            .as_ref()
            .and_then(|s| s.store.as_ref())
            .map(|s| resolve_path(s))
            .unwrap_or_else(default_store_path);

        let transcripts_dir = edith_config::sessions_dir();

        Self::new(store_path, transcripts_dir)
    }

    /// Get a session by key
    pub fn get(&self, key: &str) -> Option<SessionEntry> {
        self.inner.store.read().get(key).cloned()
    }

    /// Get or create a session
    pub fn get_or_create(&self, key: &str) -> SessionEntry {
        let mut store = self.inner.store.write();
        store
            .entry(key.to_string())
            .or_insert_with(SessionEntry::new)
            .clone()
    }

    /// Update a session
    pub fn update(&self, key: &str, entry: SessionEntry) -> SessionResult<()> {
        {
            let mut store = self.inner.store.write();
            store.insert(key.to_string(), entry);
        }
        self.save()
    }

    /// Update with a closure
    pub fn update_with<F>(&self, key: &str, f: F) -> SessionResult<SessionEntry>
    where
        F: FnOnce(&mut SessionEntry),
    {
        let entry = {
            let mut store = self.inner.store.write();
            let entry = store.entry(key.to_string()).or_insert_with(SessionEntry::new);
            f(entry);
            entry.clone()
        };
        self.save()?;
        Ok(entry)
    }

    /// Remove a session
    pub fn remove(&self, key: &str) -> SessionResult<Option<SessionEntry>> {
        let entry = {
            let mut store = self.inner.store.write();
            store.remove(key)
        };
        self.save()?;
        Ok(entry)
    }

    /// List all session keys
    pub fn keys(&self) -> Vec<String> {
        self.inner.store.read().keys().cloned().collect()
    }

    /// Get all sessions
    pub fn all(&self) -> SessionStore {
        self.inner.store.read().clone()
    }

    /// Check if a session exists
    pub fn contains(&self, key: &str) -> bool {
        self.inner.store.read().contains_key(key)
    }

    /// Get session count
    pub fn len(&self) -> usize {
        self.inner.store.read().len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.inner.store.read().is_empty()
    }

    /// Save the store to disk
    pub fn save(&self) -> SessionResult<()> {
        let store = self.inner.store.read();
        save_store_to_file(&self.inner.store_path, &store)
    }

    /// Reload from disk
    pub fn reload(&self) -> SessionResult<()> {
        let new_store = load_store_from_file(&self.inner.store_path)?;
        let mut store = self.inner.store.write();
        *store = new_store;
        Ok(())
    }

    /// Get transcript path for a session
    pub fn transcript_path(&self, session_id: &str) -> PathBuf {
        self.inner
            .transcripts_dir
            .join(format!("{}.jsonl", session_id))
    }

    /// Prune expired sessions
    pub fn prune_expired(&self, idle_minutes: u64) -> SessionResult<Vec<String>> {
        let pruned: Vec<String> = {
            let mut store = self.inner.store.write();
            let expired: Vec<_> = store
                .iter()
                .filter(|(_, entry)| entry.is_expired(idle_minutes))
                .map(|(key, _)| key.clone())
                .collect();

            for key in &expired {
                store.remove(key);
            }

            expired
        };

        if !pruned.is_empty() {
            self.save()?;
            info!(count = pruned.len(), "Pruned expired sessions");
        }

        Ok(pruned)
    }
}

/// Load store from JSON file
fn load_store_from_file(path: &Path) -> SessionResult<SessionStore> {
    if !path.exists() {
        debug!("Session store not found at {:?}, starting fresh", path);
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(path).map_err(|e| {
        SessionError::LoadStore(format!("failed to read {}: {}", path.display(), e))
    })?;

    // Try JSON5 first for compatibility, then regular JSON
    let store: SessionStore = json5::from_str(&content).or_else(|_| {
        serde_json::from_str(&content)
            .map_err(|e| SessionError::LoadStore(format!("failed to parse: {}", e)))
    })?;

    info!(
        sessions = store.len(),
        path = %path.display(),
        "Loaded session store"
    );

    Ok(store)
}

/// Save store to JSON file
fn save_store_to_file(path: &Path, store: &SessionStore) -> SessionResult<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            SessionError::SaveStore(format!(
                "failed to create directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    // Write atomically via temp file
    let tmp_path = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(store)?;

    fs::write(&tmp_path, &json).map_err(|e| {
        SessionError::SaveStore(format!("failed to write {}: {}", tmp_path.display(), e))
    })?;

    fs::rename(&tmp_path, path).map_err(|e| {
        SessionError::SaveStore(format!(
            "failed to rename {} to {}: {}",
            tmp_path.display(),
            path.display(),
            e
        ))
    })?;

    debug!(path = %path.display(), "Saved session store");
    Ok(())
}

/// Resolve a path, expanding ~ to home directory
fn resolve_path(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(path.strip_prefix("~/").unwrap_or(&path[1..]));
        }
    }
    PathBuf::from(path)
}

/// Get default store path
fn default_store_path() -> PathBuf {
    edith_config::sessions_dir().join("sessions.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_save() {
        let temp = TempDir::new().unwrap();
        let store_path = temp.path().join("sessions.json");
        let transcripts_dir = temp.path().join("transcripts");

        let store = PersistentSessionStore::new(&store_path, &transcripts_dir);

        // Create a session
        let entry = store.get_or_create("test-session");
        assert!(!entry.session_id.is_empty());

        // Update it
        store
            .update_with("test-session", |e| {
                e.model = Some("claude-3".to_string());
            })
            .unwrap();

        // Reload and verify
        let store2 = PersistentSessionStore::new(&store_path, &transcripts_dir);
        let entry2 = store2.get("test-session").unwrap();
        assert_eq!(entry2.model, Some("claude-3".to_string()));
    }

    #[test]
    fn test_prune_expired() {
        let temp = TempDir::new().unwrap();
        let store_path = temp.path().join("sessions.json");
        let transcripts_dir = temp.path().join("transcripts");

        let store = PersistentSessionStore::new(&store_path, &transcripts_dir);

        // Create a session with old timestamp
        store
            .update_with("old-session", |e| {
                e.updated_at = 0; // Very old
            })
            .unwrap();

        // Create a fresh session
        let _ = store.get_or_create("new-session");

        // Prune
        let pruned = store.prune_expired(60).unwrap();
        assert_eq!(pruned.len(), 1);
        assert!(pruned.contains(&"old-session".to_string()));

        // Verify
        assert!(store.get("old-session").is_none());
        assert!(store.get("new-session").is_some());
    }
}
