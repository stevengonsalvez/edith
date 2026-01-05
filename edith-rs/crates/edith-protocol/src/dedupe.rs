//! Idempotency deduplication cache
//!
//! Implements a TTL-based cache for deduplicating side-effecting requests
//! (send, agent, chat.send, node.invoke).

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use serde::{Deserialize, Serialize};

use crate::{ErrorShape, DEDUPE_MAX, DEDUPE_TTL_MS};

/// Cached response entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeEntry {
    /// Whether the original request succeeded
    pub ok: bool,
    /// Cached payload (if success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    /// Cached error (if failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
    /// Timestamp when entry was created
    pub ts: i64,
}

impl DedupeEntry {
    /// Create a success entry
    pub fn success(payload: serde_json::Value) -> Self {
        Self {
            ok: true,
            payload: Some(payload),
            error: None,
            ts: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create an error entry
    pub fn failure(error: ErrorShape) -> Self {
        Self {
            ok: false,
            payload: None,
            error: Some(error),
            ts: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Deduplication cache with TTL
#[derive(Clone)]
pub struct DedupeCache {
    cache: Cache<String, DedupeEntry>,
}

impl DedupeCache {
    /// Create a new deduplication cache with default settings
    pub fn new() -> Self {
        Self::with_config(DEDUPE_TTL_MS, DEDUPE_MAX)
    }

    /// Create a new deduplication cache with custom TTL and max entries
    pub fn with_config(ttl_ms: u64, max_entries: usize) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_entries as u64)
            .time_to_live(Duration::from_millis(ttl_ms))
            .build();

        Self { cache }
    }

    /// Build a cache key for a given method and idempotency key
    pub fn key(method: &str, idempotency_key: &str) -> String {
        format!("{}:{}", method, idempotency_key)
    }

    /// Check if a request is cached
    pub async fn get(&self, key: &str) -> Option<DedupeEntry> {
        self.cache.get(key).await
    }

    /// Cache a successful response
    pub async fn set_success(&self, key: &str, payload: serde_json::Value) {
        self.cache.insert(key.to_string(), DedupeEntry::success(payload)).await;
    }

    /// Cache an error response
    pub async fn set_failure(&self, key: &str, error: ErrorShape) {
        self.cache.insert(key.to_string(), DedupeEntry::failure(error)).await;
    }

    /// Cache an entry
    pub async fn set(&self, key: &str, entry: DedupeEntry) {
        self.cache.insert(key.to_string(), entry).await;
    }

    /// Remove an entry
    pub async fn remove(&self, key: &str) {
        self.cache.invalidate(key).await;
    }

    /// Get current cache size
    pub fn len(&self) -> usize {
        self.cache.entry_count() as usize
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries
    pub async fn clear(&self) {
        self.cache.invalidate_all();
        // Wait for invalidation to complete
        self.cache.run_pending_tasks().await;
    }
}

impl Default for DedupeCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Deduplication result - either cached or needs processing
#[derive(Debug)]
pub enum DedupeResult {
    /// Request was cached, use this response
    Cached(DedupeEntry),
    /// Request is new, process it
    New,
}

/// Helper for handling idempotent requests
pub struct IdempotencyHandler {
    cache: Arc<DedupeCache>,
}

impl IdempotencyHandler {
    /// Create a new handler with the given cache
    pub fn new(cache: Arc<DedupeCache>) -> Self {
        Self { cache }
    }

    /// Check if request should be deduplicated
    pub async fn check(&self, method: &str, idempotency_key: &str) -> DedupeResult {
        let key = DedupeCache::key(method, idempotency_key);
        match self.cache.get(&key).await {
            Some(entry) => DedupeResult::Cached(entry),
            None => DedupeResult::New,
        }
    }

    /// Record a successful response
    pub async fn record_success(&self, method: &str, idempotency_key: &str, payload: serde_json::Value) {
        let key = DedupeCache::key(method, idempotency_key);
        self.cache.set_success(&key, payload).await;
    }

    /// Record a failed response
    pub async fn record_failure(&self, method: &str, idempotency_key: &str, error: ErrorShape) {
        let key = DedupeCache::key(method, idempotency_key);
        self.cache.set_failure(&key, error).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dedupe_cache_basic() {
        let cache = DedupeCache::new();

        // Initially empty
        assert!(cache.get("test:key1").await.is_none());

        // Set and get
        cache.set_success("test:key1", serde_json::json!({"id": 123})).await;

        let entry = cache.get("test:key1").await.unwrap();
        assert!(entry.ok);
        assert_eq!(entry.payload.unwrap()["id"], 123);
    }

    #[tokio::test]
    async fn test_dedupe_cache_error() {
        let cache = DedupeCache::new();

        let error = ErrorShape::new("TEST_ERROR", "Test failure");
        cache.set_failure("test:key2", error).await;

        let entry = cache.get("test:key2").await.unwrap();
        assert!(!entry.ok);
        assert!(entry.error.is_some());
        assert_eq!(entry.error.unwrap().code, "TEST_ERROR");
    }

    #[tokio::test]
    async fn test_dedupe_key_format() {
        let key = DedupeCache::key("send", "idem-123");
        assert_eq!(key, "send:idem-123");

        let key = DedupeCache::key("agent", "run-456");
        assert_eq!(key, "agent:run-456");
    }

    #[tokio::test]
    async fn test_idempotency_handler() {
        let cache = Arc::new(DedupeCache::new());
        let handler = IdempotencyHandler::new(cache);

        // First request - should be new
        match handler.check("send", "key-1").await {
            DedupeResult::New => {}
            DedupeResult::Cached(_) => panic!("Should be new"),
        }

        // Record success
        handler.record_success("send", "key-1", serde_json::json!({"sent": true})).await;

        // Second request - should be cached
        match handler.check("send", "key-1").await {
            DedupeResult::Cached(entry) => {
                assert!(entry.ok);
            }
            DedupeResult::New => panic!("Should be cached"),
        }
    }

    #[tokio::test]
    async fn test_cache_max_capacity() {
        // Create cache with small capacity
        let cache = DedupeCache::with_config(60000, 3);

        // Add 5 entries
        for i in 0..5 {
            cache.set_success(&format!("test:key{}", i), serde_json::json!({"i": i})).await;
        }

        // Wait for eviction to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should have at most 3 entries
        assert!(cache.len() <= 3);
    }
}
