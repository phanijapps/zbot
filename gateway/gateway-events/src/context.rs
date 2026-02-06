//! Hook context types for tracking message origin and routing responses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Context for a hook invocation, tracks where a message came from.
///
/// This context is passed through the execution pipeline so that the
/// `respond` tool knows where to route its response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Type of hook that received the message.
    pub hook_type: HookType,

    /// Unique identifier for the source (phone number, email, session ID, etc).
    pub source_id: String,

    /// Optional channel identifier (for group chats, threads, etc).
    pub channel_id: Option<String>,

    /// Hook-specific metadata (original payload, headers, etc).
    #[serde(default)]
    pub metadata: HashMap<String, Value>,

    /// When this hook context was created.
    pub created_at: DateTime<Utc>,
}

impl HookContext {
    /// Create a new hook context.
    pub fn new(hook_type: HookType, source_id: impl Into<String>) -> Self {
        Self {
            hook_type,
            source_id: source_id.into(),
            channel_id: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Set the channel ID.
    pub fn with_channel(mut self, channel_id: impl Into<String>) -> Self {
        self.channel_id = Some(channel_id.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Create a CLI hook context.
    pub fn cli(source_id: impl Into<String>) -> Self {
        Self::new(HookType::Cli, source_id)
    }

    /// Create a Web hook context.
    pub fn web(session_id: impl Into<String>) -> Self {
        let session = session_id.into();
        Self::new(
            HookType::Web {
                session_id: session.clone(),
            },
            session,
        )
    }

    /// Create a Cron hook context.
    pub fn cron(job_id: impl Into<String>) -> Self {
        let job = job_id.into();
        Self::new(
            HookType::Cron {
                job_id: job.clone(),
            },
            job,
        )
    }

    /// Create a webhook hook context.
    pub fn webhook(endpoint_id: impl Into<String>, source_id: impl Into<String>) -> Self {
        Self::new(
            HookType::Webhook {
                endpoint_id: endpoint_id.into(),
            },
            source_id,
        )
    }
}

/// Type of hook that triggered the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookType {
    /// Command-line interface.
    Cli,

    /// Web interface (WebSocket connection).
    Web {
        /// WebSocket session identifier.
        session_id: String,
    },

    /// Scheduled cron job.
    Cron {
        /// Cron job identifier.
        job_id: String,
    },

    /// WhatsApp webhook.
    WhatsApp {
        /// WhatsApp Business phone number ID.
        phone_number_id: String,
    },

    /// Telegram webhook.
    Telegram {
        /// Telegram bot ID.
        bot_id: String,
        /// Telegram chat ID.
        chat_id: i64,
    },

    /// Signal messenger.
    Signal {
        /// Signal phone number.
        number: String,
    },

    /// Email.
    Email {
        /// Email account identifier.
        account_id: String,
    },

    /// Generic webhook endpoint.
    Webhook {
        /// Webhook endpoint identifier.
        endpoint_id: String,
    },
}

impl HookType {
    /// Get a string identifier for this hook type.
    pub fn type_name(&self) -> &'static str {
        match self {
            HookType::Cli => "cli",
            HookType::Web { .. } => "web",
            HookType::Cron { .. } => "cron",
            HookType::WhatsApp { .. } => "whatsapp",
            HookType::Telegram { .. } => "telegram",
            HookType::Signal { .. } => "signal",
            HookType::Email { .. } => "email",
            HookType::Webhook { .. } => "webhook",
        }
    }

    /// Create a HookType from string type and ID.
    pub fn from_type_and_id(hook_type: &str, hook_id: &str) -> Option<Self> {
        match hook_type {
            "cli" => Some(HookType::Cli),
            "web" => Some(HookType::Web {
                session_id: hook_id.to_string(),
            }),
            "cron" => Some(HookType::Cron {
                job_id: hook_id.to_string(),
            }),
            "whatsapp" => Some(HookType::WhatsApp {
                phone_number_id: hook_id.to_string(),
            }),
            "telegram" => {
                // For telegram, hook_id format is "bot_id:chat_id"
                let parts: Vec<&str> = hook_id.split(':').collect();
                if parts.len() == 2 {
                    parts[1].parse().ok().map(|chat_id| HookType::Telegram {
                        bot_id: parts[0].to_string(),
                        chat_id,
                    })
                } else {
                    None
                }
            }
            "signal" => Some(HookType::Signal {
                number: hook_id.to_string(),
            }),
            "email" => Some(HookType::Email {
                account_id: hook_id.to_string(),
            }),
            "webhook" => Some(HookType::Webhook {
                endpoint_id: hook_id.to_string(),
            }),
            _ => None,
        }
    }

    /// Check if this hook type supports responses.
    pub fn supports_response(&self) -> bool {
        match self {
            HookType::Cli => true,
            HookType::Web { .. } => true,
            HookType::Cron { .. } => false, // Cron jobs typically log only
            HookType::WhatsApp { .. } => true,
            HookType::Telegram { .. } => true,
            HookType::Signal { .. } => true,
            HookType::Email { .. } => true,
            HookType::Webhook { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_context_creation() {
        let ctx = HookContext::web("session-123")
            .with_channel("channel-456")
            .with_metadata("key", serde_json::json!("value"));

        assert_eq!(ctx.source_id, "session-123");
        assert_eq!(ctx.channel_id, Some("channel-456".to_string()));
        assert!(ctx.metadata.contains_key("key"));
    }

    #[test]
    fn test_hook_type_from_string() {
        let cli = HookType::from_type_and_id("cli", "").unwrap();
        assert_eq!(cli, HookType::Cli);

        let web = HookType::from_type_and_id("web", "session-123").unwrap();
        assert!(matches!(web, HookType::Web { session_id } if session_id == "session-123"));

        let telegram = HookType::from_type_and_id("telegram", "bot123:456789").unwrap();
        assert!(matches!(telegram, HookType::Telegram { bot_id, chat_id }
            if bot_id == "bot123" && chat_id == 456789));
    }

    #[test]
    fn test_hook_type_supports_response() {
        assert!(HookType::Cli.supports_response());
        assert!(HookType::Web {
            session_id: "s".to_string()
        }
        .supports_response());
        assert!(!HookType::Cron {
            job_id: "j".to_string()
        }
        .supports_response());
    }
}
