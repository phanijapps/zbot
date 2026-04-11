// ============================================================================
// GATEWAY CLIENT
// HTTP and WebSocket client for communicating with Zero Gateway daemon
// ============================================================================

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub status: String,
    pub version: String,
    pub agent_count: Option<usize>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GatewayEvent {
    Connected {
        session_id: String,
    },
    Token {
        content: String,
    },
    Thinking {
        content: String,
    },
    ToolCall {
        tool_call_id: String,
        tool: String,
        args: serde_json::Value,
    },
    ToolResult {
        tool_call_id: String,
        result: Option<String>,
        error: Option<String>,
    },
    Iteration {
        current: u32,
        max: u32,
    },
    Done {
        final_message: Option<String>,
    },
    Error {
        code: Option<String>,
        message: String,
    },
}

// ============================================================================
// Client Messages (to server)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ClientMessage {
    Invoke {
        agent_id: String,
        conversation_id: String,
        message: String,
    },
    Stop {
        conversation_id: String,
    },
    Continue {
        conversation_id: String,
        additional_iterations: u32,
    },
    Ping,
}

// ============================================================================
// Server Messages (from server)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ServerMessage {
    Connected {
        session_id: String,
    },
    AgentStarted {
        agent_id: String,
        conversation_id: String,
    },
    AgentCompleted {
        agent_id: String,
        conversation_id: String,
        result: Option<String>,
    },
    AgentStopped {
        agent_id: String,
        conversation_id: String,
        iteration: u32,
    },
    Token {
        conversation_id: String,
        delta: String,
    },
    Thinking {
        conversation_id: String,
        content: String,
    },
    ToolCall {
        conversation_id: String,
        tool_call_id: String,
        tool: String,
        args: serde_json::Value,
    },
    ToolResult {
        conversation_id: String,
        tool_call_id: String,
        result: Option<String>,
        error: Option<String>,
    },
    TurnComplete {
        conversation_id: String,
        final_message: Option<String>,
    },
    Iteration {
        conversation_id: String,
        current: u32,
        max: u32,
    },
    ContinuationPrompt {
        conversation_id: String,
        iteration: u32,
        message: String,
    },
    Error {
        conversation_id: Option<String>,
        code: Option<String>,
        message: String,
    },
    Pong,
}

// ============================================================================
// Gateway Client
// ============================================================================

pub struct GatewayClient {
    http_url: String,
    ws_url: String,
    http_client: reqwest::Client,
}

impl GatewayClient {
    pub fn new(http_url: &str, ws_url: &str) -> Self {
        Self {
            http_url: http_url.to_string(),
            ws_url: ws_url.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Check if the gateway daemon is running
    pub async fn is_running(&self) -> bool {
        self.http_client
            .get(format!("{}/api/health", self.http_url))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .is_ok()
    }

    /// Get gateway status
    pub async fn get_status(&self) -> Result<GatewayStatus> {
        let resp = self
            .http_client
            .get(format!("{}/api/status", self.http_url))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json::<GatewayStatus>()
            .await?;
        Ok(resp)
    }

    /// List available agents
    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        let resp = self
            .http_client
            .get(format!("{}/api/agents", self.http_url))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?
            .json::<Vec<Agent>>()
            .await?;
        Ok(resp)
    }

    /// Invoke an agent and return a stream of events
    pub async fn invoke(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
    ) -> Result<mpsc::Receiver<GatewayEvent>> {
        let (tx, rx) = mpsc::channel(100);

        let ws_url = format!("{}/ws", self.ws_url);
        let agent_id = agent_id.to_string();
        let conversation_id = conversation_id.to_string();
        let message = message.to_string();

        tokio::spawn(async move {
            if let Err(e) =
                Self::run_websocket(ws_url, agent_id, conversation_id, message, tx.clone()).await
            {
                let _ = tx
                    .send(GatewayEvent::Error {
                        code: None,
                        message: e.to_string(),
                    })
                    .await;
            }
        });

        Ok(rx)
    }

    async fn run_websocket(
        ws_url: String,
        agent_id: String,
        conversation_id: String,
        message: String,
        tx: mpsc::Sender<GatewayEvent>,
    ) -> Result<()> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| anyhow!("Failed to connect to gateway: {}", e))?;

        let (mut write, mut read) = ws_stream.split();

        // Wait for connected message
        let mut connected = false;
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(ServerMessage::Connected { session_id }) =
                        serde_json::from_str::<ServerMessage>(&text)
                    {
                        let _ = tx.send(GatewayEvent::Connected { session_id }).await;
                        connected = true;
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    return Err(anyhow!("Connection closed before connected"));
                }
                Err(e) => {
                    return Err(anyhow!("WebSocket error: {}", e));
                }
                _ => {}
            }
        }

        if !connected {
            return Err(anyhow!("Failed to receive connected message"));
        }

        // Send invoke message
        let invoke_msg = ClientMessage::Invoke {
            agent_id,
            conversation_id: conversation_id.clone(),
            message,
        };
        let json = serde_json::to_string(&invoke_msg)?;
        write.send(Message::Text(json)).await?;

        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                        let event = match server_msg {
                            ServerMessage::Token { delta, .. } => {
                                Some(GatewayEvent::Token { content: delta })
                            }
                            ServerMessage::Thinking { content, .. } => {
                                Some(GatewayEvent::Thinking { content })
                            }
                            ServerMessage::ToolCall {
                                tool_call_id,
                                tool,
                                args,
                                ..
                            } => Some(GatewayEvent::ToolCall {
                                tool_call_id,
                                tool,
                                args,
                            }),
                            ServerMessage::ToolResult {
                                tool_call_id,
                                result,
                                error,
                                ..
                            } => Some(GatewayEvent::ToolResult {
                                tool_call_id,
                                result,
                                error,
                            }),
                            ServerMessage::Iteration { current, max, .. } => {
                                Some(GatewayEvent::Iteration { current, max })
                            }
                            ServerMessage::TurnComplete { final_message, .. } => {
                                Some(GatewayEvent::Done { final_message })
                            }
                            ServerMessage::AgentCompleted { result, .. } => {
                                Some(GatewayEvent::Done {
                                    final_message: result,
                                })
                            }
                            ServerMessage::AgentStopped { .. } => Some(GatewayEvent::Done {
                                final_message: None,
                            }),
                            ServerMessage::Error { code, message, .. } => {
                                Some(GatewayEvent::Error { code, message })
                            }
                            _ => None,
                        };

                        if let Some(event) = event {
                            let is_done = matches!(
                                event,
                                GatewayEvent::Done { .. } | GatewayEvent::Error { .. }
                            );
                            let _ = tx.send(event).await;
                            if is_done {
                                break;
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Err(e) => {
                    let _ = tx
                        .send(GatewayEvent::Error {
                            code: None,
                            message: e.to_string(),
                        })
                        .await;
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Stop an agent execution
    #[allow(dead_code)]
    pub async fn stop(&self, _conversation_id: &str) -> Result<()> {
        // This would need an active WebSocket connection
        // For now, we'll just note that this needs the WS connection
        tracing::warn!("Stop not implemented for standalone calls - use chat mode");
        Ok(())
    }

    /// Continue an agent execution
    #[allow(dead_code)]
    pub async fn continue_execution(
        &self,
        _conversation_id: &str,
        _additional_iterations: u32,
    ) -> Result<()> {
        tracing::warn!("Continue not implemented for standalone calls - use chat mode");
        Ok(())
    }
}
