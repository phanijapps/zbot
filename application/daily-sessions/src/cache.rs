// ============================================================================
// CONVERSATION CACHE
// High-performance caching for sessions using Moka
// ============================================================================

use crate::types::{DailySession, SessionMessage};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_capacity: u64,
    pub ttl: Duration,
    pub time_to_idle: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 1000,              // Max 1000 sessions
            ttl: Duration::from_secs(3600),  // 1 hour TTL
            time_to_idle: Duration::from_secs(1800), // 30 min idle
        }
    }
}

/// Cached session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSession {
    pub session_id: String,
    pub agent_id: String,
    pub messages: Vec<SessionMessage>,
    pub message_count: usize,
    pub token_count: i64,
    pub last_accessed: i64,
}

impl CachedSession {
    pub fn from_session(session: &DailySession, messages: Vec<SessionMessage>) -> Self {
        Self {
            session_id: session.id.clone(),
            agent_id: session.agent_id.clone(),
            messages,
            message_count: session.message_count as usize,
            token_count: session.token_count,
            last_accessed: chrono::Utc::now().timestamp(),
        }
    }
}

/// Conversation cache using Moka
pub struct ConversationCache {
    sessions: Cache<String, CachedSession>,
    config: CacheConfig,
}

impl ConversationCache {
    /// Create new conversation cache
    pub fn new(config: CacheConfig) -> Self {
        let sessions = Cache::builder()
            .max_capacity(config.max_capacity)
            .time_to_live(config.ttl)
            .time_to_idle(config.time_to_idle)
            .build();

        Self { sessions, config }
    }

    /// Get cached session
    pub async fn get(&self, session_id: &str) -> Option<CachedSession> {
        self.sessions.get(session_id).await
    }

    /// Insert or update session in cache
    pub async fn insert(&self, session: CachedSession) {
        self.sessions.insert(session.session_id.clone(), session).await;
    }

    /// Invalidate specific session
    pub async fn invalidate(&self, session_id: &str) {
        self.sessions.invalidate(session_id).await;
    }

    /// Invalidate all sessions for an agent
    pub async fn invalidate_agent(&self, agent_id: &str) {
        let sessions = self.sessions.iter();
        for (session_id, session) in sessions {
            if session.agent_id == agent_id {
                self.sessions.invalidate(session_id.as_ref()).await;
            }
        }
    }

    /// Clear entire cache
    pub async fn clear(&self) {
        self.sessions.invalidate_all();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.sessions.entry_count(),
            hit_count: 0,  // Moka 0.12 doesn't expose these directly
            miss_count: 0, // Track entry count only for now
        }
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub entry_count: u64,
    pub hit_count: u64,
    pub miss_count: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f64 / total as f64
        }
    }
}

/// Global conversation cache instance
lazy_static::lazy_static! {
    pub static ref CONVERSATION_CACHE: Arc<ConversationCache> =
        Arc::new(ConversationCache::new(CacheConfig::default()));
}
