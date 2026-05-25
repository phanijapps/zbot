//! HTTP + WebSocket client for the z-Bot daemon.
//!
//! Phase 1 scope: minimal HTTP wrapper with a `/api/health` smoke test.
//! Subsequent phases extend this with `chat/init`, `sessions`, `wards`,
//! `memory/search`, and the WebSocket event stream.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::config::Config;

/// Thin wrapper over `reqwest::Client` bound to a daemon URL.
#[derive(Debug, Clone)]
pub struct DaemonClient {
    http: reqwest::Client,
    base: String,
}

impl DaemonClient {
    pub fn new(cfg: Config) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("zbot/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest::Client should always build");
        Self { http, base: cfg.daemon_url }
    }

    /// `GET /api/health` — startup smoke test.
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/api/health", self.base);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "daemon health check returned HTTP {}",
                resp.status()
            ));
        }
        let body: HealthResponse = resp.json().await.context("parse /api/health body")?;
        Ok(body)
    }
}

/// Shape of the `/api/health` response.
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    /// Daemon uptime in seconds. Optional — older daemons may not include it.
    /// Phase 2 reads this when rendering the header.
    #[serde(default)]
    #[allow(dead_code)]
    pub uptime: Option<u64>,
}
