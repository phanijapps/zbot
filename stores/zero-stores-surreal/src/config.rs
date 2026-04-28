//! Configuration types for the SurrealDB backend.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurrealConfig {
    /// Connection URL. Supported schemes: `rocksdb://`, `mem://`, `ws://`, `wss://`.
    /// `$VAULT` is expanded against the daemon's vault root before connecting.
    pub url: String,

    /// SurrealDB namespace. Defaults to `memory_kg`.
    #[serde(default = "default_namespace")]
    pub namespace: String,

    /// SurrealDB database. Defaults to `main`.
    #[serde(default = "default_database")]
    pub database: String,

    /// Optional credentials. `None` for Mode A (embedded). `Some(...)` for Mode B.
    #[serde(default)]
    pub credentials: Option<SurrealCredentials>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurrealCredentials {
    pub username: String,
    pub password: String,
}

fn default_namespace() -> String {
    "memory_kg".into()
}

fn default_database() -> String {
    "main".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_when_only_url_present() {
        let json = r#"{"url": "mem://"}"#;
        let cfg: SurrealConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.url, "mem://");
        assert_eq!(cfg.namespace, "memory_kg");
        assert_eq!(cfg.database, "main");
        assert!(cfg.credentials.is_none());
    }

    #[test]
    fn full_config_parses() {
        let json = r#"{
            "url": "ws://127.0.0.1:18792",
            "namespace": "memory_kg",
            "database": "main",
            "credentials": {"username": "agentzero", "password": "secret"}
        }"#;
        let cfg: SurrealConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.url, "ws://127.0.0.1:18792");
        assert!(cfg.credentials.is_some());
    }
}
