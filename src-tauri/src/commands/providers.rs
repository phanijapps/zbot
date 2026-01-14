// ============================================================================
// PROVIDERS COMMANDS
// LLM provider management for OpenAI-compatible APIs
// ============================================================================

use crate::settings::AppDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

/// Provider data structure
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Test result for provider connection
#[derive(Debug, Clone, Serialize)]
pub struct ProviderTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
}

/// Get the providers config file path
fn get_providers_config_path() -> Result<std::path::PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.config_dir.join("providers.json"))
}

/// Read all providers from config file
fn read_providers() -> Result<Vec<Provider>, String> {
    let config_path = get_providers_config_path()?;

    if !config_path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read providers config: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse providers config: {}", e))
}

/// Write providers to config file
fn write_providers(providers: &[Provider]) -> Result<(), String> {
    let config_path = get_providers_config_path()?;

    let content = serde_json::to_string_pretty(providers)
        .map_err(|e| format!("Failed to serialize providers: {}", e))?;

    fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write providers config: {}", e))?;

    Ok(())
}

/// Lists all providers
#[tauri::command]
pub async fn list_providers() -> Result<Vec<Provider>, String> {
    read_providers()
}

/// Gets a single provider by ID
#[tauri::command]
pub async fn get_provider(id: String) -> Result<Provider, String> {
    let providers = read_providers()?;
    providers
        .into_iter()
        .find(|p| p.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("Provider not found: {}", id))
}

/// Creates a new provider
#[tauri::command]
pub async fn create_provider(provider: Provider) -> Result<Provider, String> {
    let mut providers = read_providers()?;

    // Generate ID if not provided
    let provider_id = provider.id.clone().unwrap_or_else(|| {
        format!("provider-{}",
            provider.name.to_lowercase().replace(' ', "-"))
    });

    // Check for duplicate ID
    if providers.iter().any(|p| p.id.as_deref() == Some(provider_id.as_str())) {
        return Err(format!("Provider with ID {} already exists", provider_id));
    }

    let mut new_provider = provider.clone();
    new_provider.id = Some(provider_id.clone());

    providers.push(new_provider.clone());
    write_providers(&providers)?;

    Ok(new_provider)
}

/// Updates an existing provider
#[tauri::command]
pub async fn update_provider(id: String, provider: Provider) -> Result<Provider, String> {
    let mut providers = read_providers()?;

    let index = providers
        .iter()
        .position(|p| p.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("Provider not found: {}", id))?;

    // Preserve ID and created_at if not provided
    let mut updated_provider = provider.clone();
    if updated_provider.id.is_none() {
        updated_provider.id = providers[index].id.clone();
    }
    if updated_provider.created_at.is_none() {
        updated_provider.created_at = providers[index].created_at.clone();
    }

    providers[index] = updated_provider.clone();
    write_providers(&providers)?;

    Ok(updated_provider)
}

/// Deletes a provider
#[tauri::command]
pub async fn delete_provider(id: String) -> Result<(), String> {
    let mut providers = read_providers()?;

    let initial_len = providers.len();
    providers.retain(|p| p.id.as_deref() != Some(id.as_str()));

    if providers.len() == initial_len {
        return Err(format!("Provider not found: {}", id));
    }

    write_providers(&providers)?;
    Ok(())
}

/// Tests a provider connection
#[tauri::command]
pub async fn test_provider(provider: Provider) -> Result<ProviderTestResult, String> {
    use tokio::task::spawn_blocking;

    let api_key = provider.api_key.clone();
    let base_url = provider.base_url.clone();
    let name = provider.name.clone();

    // Build curl command to test the endpoint
    let handle = spawn_blocking(move || {
        let models_url = format!("{}/models", base_url.trim_end_matches('/'));

        let mut cmd = Command::new("curl");
        cmd.arg("-s")
            .arg("-S")
            .arg("-X")
            .arg("GET")
            .arg(&models_url)
            .arg("-H")
            .arg(format!("Authorization: Bearer {}", api_key))
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("--max-time")
            .arg("10");

        cmd.output()
    });

    let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(15), handle).await;

    let result = match timeout_result {
        Ok(inner) => match inner {
            Ok(io_result) => match io_result {
                Ok(output) => {
                    if output.status.success() {
                        // Parse the response to extract models
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        match serde_json::from_str::<serde_json::Value>(&stdout) {
                            Ok(json) => {
                                // Try to extract models from OpenAI-style response
                                let models = if let Some(data) = json.get("data") {
                                    if let Some(arr) = data.as_array() {
                                        arr.iter()
                                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                                            .map(String::from)
                                            .collect()
                                    } else {
                                        vec![]
                                    }
                                } else {
                                    vec![]
                                };

                                if !models.is_empty() {
                                    Ok(ProviderTestResult {
                                        success: true,
                                        message: format!("Successfully connected to {}. Found {} available models.",
                                            name, models.len()),
                                        models: Some(models),
                                    })
                                } else {
                                    Ok(ProviderTestResult {
                                        success: true,
                                        message: format!("Successfully connected to {}. Could not auto-detect models.", name),
                                        models: None,
                                    })
                                }
                            }
                            Err(_) => {
                                // Connection succeeded but response parsing failed
                                Ok(ProviderTestResult {
                                    success: true,
                                    message: format!("Successfully connected to {}. Response format not recognized.", name),
                                    models: None,
                                })
                            }
                        }
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let error_msg = if !stderr.is_empty() {
                            stderr.to_string()
                        } else {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            if !stdout.is_empty() {
                                stdout.to_string()
                            } else {
                                format!("HTTP error: {}", output.status)
                            }
                        };
                        Ok(ProviderTestResult {
                            success: false,
                            message: format!("Connection failed: {}",
                                error_msg.lines().next().unwrap_or(&error_msg)),
                            models: None,
                        })
                    }
                }
                Err(e) => {
                    // Command execution failed
                    Ok(ProviderTestResult {
                        success: false,
                        message: format!("Connection test failed: {}. Is curl installed?", e),
                        models: None,
                    })
                }
            },
            Err(e) => {
                // Task spawn failed (JoinError)
                Ok(ProviderTestResult {
                    success: false,
                    message: format!("Test failed: {}", e),
                    models: None,
                })
            }
        },
        Err(_) => {
            // Timeout elapsed
            Ok(ProviderTestResult {
                success: false,
                message: "Connection timed out after 15 seconds. Check the base URL and network connection.".to_string(),
                models: None,
            })
        }
    };

    result
}
