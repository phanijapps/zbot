// ============================================================================
// PROVIDERS SERVICE
// LLM provider management for the gateway
// ============================================================================

use crate::paths::SharedVaultPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub models: Vec<String>,
    #[serde(rename = "embeddingModels", skip_serializing_if = "Option::is_none")]
    pub embedding_models: Option<Vec<String>>,
    #[serde(rename = "embeddingDimensions", skip_serializing_if = "Option::is_none")]
    pub embedding_dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    #[serde(rename = "isDefault", default)]
    pub is_default: bool,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Maximum concurrent LLM requests for this provider (default: 3).
    /// Set lower for rate-limited providers (e.g., 1 for free tiers).
    #[serde(rename = "maxConcurrentRequests", skip_serializing_if = "Option::is_none")]
    pub max_concurrent_requests: Option<u32>,
    /// Context window size in tokens. Overrides the hardcoded model lookup.
    /// Set this when using models not in the built-in lookup table.
    #[serde(rename = "contextWindow", skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
}

// ============================================================================
// Service
// ============================================================================

pub struct ProviderService {
    paths: SharedVaultPaths,
    cache: RwLock<Option<Vec<Provider>>>,
}

impl ProviderService {
    pub fn new(paths: SharedVaultPaths) -> Self {
        Self {
            paths,
            cache: RwLock::new(None),
        }
    }

    /// Get the config file path.
    fn config_path(&self) -> PathBuf {
        self.paths.providers()
    }

    /// Read all providers from config file (bypasses cache).
    fn read_providers_from_disk(&self) -> Result<Vec<Provider>, String> {
        if !self.config_path().exists() {
            return Ok(vec![]);
        }

        let content = fs::read_to_string(&self.config_path())
            .map_err(|e| format!("Failed to read providers config: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse providers config: {}", e))
    }

    /// Write providers to config file and update cache.
    fn write_providers(&self, providers: &[Provider]) -> Result<(), String> {
        let content = serde_json::to_string_pretty(providers)
            .map_err(|e| format!("Failed to serialize providers: {}", e))?;

        fs::write(&self.config_path(), content)
            .map_err(|e| format!("Failed to write providers config: {}", e))?;

        // Update cache with the data we just wrote
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(providers.to_vec());
        }

        Ok(())
    }

    /// Invalidate the cache, forcing next read to go to disk.
    pub fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }
    }

    /// List all providers (cached).
    pub fn list(&self) -> Result<Vec<Provider>, String> {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(providers) = cache.as_ref() {
                return Ok(providers.clone());
            }
        }

        // Cache miss: read from disk
        let providers = self.read_providers_from_disk()?;

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(providers.clone());
        }

        Ok(providers)
    }

    /// Get a single provider by ID
    pub fn get(&self, id: &str) -> Result<Provider, String> {
        let providers = self.list()?;
        providers
            .into_iter()
            .find(|p| p.id.as_deref() == Some(id))
            .ok_or_else(|| format!("Provider not found: {}", id))
    }

    /// Create a new provider
    pub fn create(&self, mut provider: Provider) -> Result<Provider, String> {
        let mut providers = self.list()?;

        // Generate ID if not provided
        let provider_id = provider.id.clone().unwrap_or_else(|| {
            format!(
                "provider-{}",
                provider.name.to_lowercase().replace(' ', "-")
            )
        });

        // Check for duplicate ID
        if providers
            .iter()
            .any(|p| p.id.as_deref() == Some(provider_id.as_str()))
        {
            return Err(format!("Provider with ID {} already exists", provider_id));
        }

        provider.id = Some(provider_id);
        provider.created_at = Some(chrono::Utc::now().to_rfc3339());

        providers.push(provider.clone());
        self.write_providers(&providers)?;

        Ok(provider)
    }

    /// Update an existing provider
    pub fn update(&self, id: &str, mut provider: Provider) -> Result<Provider, String> {
        let mut providers = self.list()?;

        let index = providers
            .iter()
            .position(|p| p.id.as_deref() == Some(id))
            .ok_or_else(|| format!("Provider not found: {}", id))?;

        // Preserve ID and created_at
        if provider.id.is_none() {
            provider.id = providers[index].id.clone();
        }
        if provider.created_at.is_none() {
            provider.created_at = providers[index].created_at.clone();
        }

        providers[index] = provider.clone();
        self.write_providers(&providers)?;

        Ok(provider)
    }

    /// Delete a provider
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut providers = self.list()?;

        let initial_len = providers.len();
        providers.retain(|p| p.id.as_deref() != Some(id));

        if providers.len() == initial_len {
            return Err(format!("Provider not found: {}", id));
        }

        self.write_providers(&providers)?;
        Ok(())
    }

    /// Set a provider as the default (unsets all others)
    pub fn set_default(&self, id: &str) -> Result<Provider, String> {
        let mut providers = self.list()?;

        let mut found = false;
        for provider in providers.iter_mut() {
            if provider.id.as_deref() == Some(id) {
                provider.is_default = true;
                found = true;
            } else {
                provider.is_default = false;
            }
        }

        if !found {
            return Err(format!("Provider not found: {}", id));
        }

        self.write_providers(&providers)?;

        // Return the updated provider
        self.get(id)
    }

    /// Test a provider connection
    pub async fn test(&self, provider: &Provider) -> ProviderTestResult {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => {
                return ProviderTestResult {
                    success: false,
                    message: format!("Failed to create HTTP client: {}", e),
                    models: None,
                }
            }
        };

        let models_url = format!("{}/models", provider.base_url.trim_end_matches('/'));

        let response = client
            .get(&models_url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => {
                            // Try to extract models from OpenAI-style response
                            let models: Vec<String> = json
                                .get("data")
                                .and_then(|d| d.as_array())
                                .map(|arr: &Vec<serde_json::Value>| {
                                    arr.iter()
                                        .filter_map(|m: &serde_json::Value| {
                                            m.get("id").and_then(|id| id.as_str()).map(String::from)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();

                            if !models.is_empty() {
                                ProviderTestResult {
                                    success: true,
                                    message: format!(
                                        "Successfully connected to {}. Found {} models.",
                                        provider.name,
                                        models.len()
                                    ),
                                    models: Some(models),
                                }
                            } else {
                                ProviderTestResult {
                                    success: true,
                                    message: format!(
                                        "Connected to {}. Could not auto-detect models.",
                                        provider.name
                                    ),
                                    models: None,
                                }
                            }
                        }
                        Err(_) => ProviderTestResult {
                            success: true,
                            message: format!(
                                "Connected to {}. Response format not recognized.",
                                provider.name
                            ),
                            models: None,
                        },
                    }
                } else {
                    let status = resp.status();
                    let error_text: String = resp.text().await.unwrap_or_default();
                    ProviderTestResult {
                        success: false,
                        message: format!("HTTP {}: {}", status, error_text),
                        models: None,
                    }
                }
            }
            Err(e) => ProviderTestResult {
                success: false,
                message: format!("Connection failed: {}", e),
                models: None,
            },
        }
    }
}
