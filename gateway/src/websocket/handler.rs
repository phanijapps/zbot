//! # WebSocket Handler
//!
//! Handles WebSocket connections and message routing.

use super::messages::{ClientMessage, ServerMessage};
use super::session::{SessionRegistry, WsSession};
use crate::error::{GatewayError, Result};
use crate::events::{EventBus, GatewayEvent};
use crate::hooks::HookContext;
use crate::services::RuntimeService;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// WebSocket handler for managing connections.
pub struct WebSocketHandler {
    event_bus: Arc<EventBus>,
    sessions: Arc<SessionRegistry>,
    runtime: Arc<RuntimeService>,
}

impl WebSocketHandler {
    /// Create a new WebSocket handler.
    pub fn new(event_bus: Arc<EventBus>, runtime: Arc<RuntimeService>) -> Self {
        Self {
            event_bus,
            sessions: Arc::new(SessionRegistry::new()),
            runtime,
        }
    }

    /// Get the session registry.
    pub fn sessions(&self) -> Arc<SessionRegistry> {
        self.sessions.clone()
    }

    /// Run the WebSocket server.
    pub async fn run(&self, addr: &str, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| GatewayError::ServerStartup(e.to_string()))?;

        info!("WebSocket server listening on {}", addr);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            debug!("New WebSocket connection from {}", peer_addr);
                            let sessions = self.sessions.clone();
                            let event_bus = self.event_bus.clone();
                            let runtime = self.runtime.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, sessions, event_bus, runtime).await {
                                    warn!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("WebSocket server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Broadcast a message to all sessions for an agent.
    pub async fn broadcast_to_agent(&self, agent_id: &str, msg: ServerMessage) {
        self.sessions.broadcast_to_agent(agent_id, msg).await;
    }
}

/// Handle a single WebSocket connection.
async fn handle_connection(
    stream: TcpStream,
    sessions: Arc<SessionRegistry>,
    event_bus: Arc<EventBus>,
    runtime: Arc<RuntimeService>,
) -> Result<()> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| GatewayError::WebSocket(e.to_string()))?;

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Create message channel for this session
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Create and register session
    let session = WsSession::new(tx.clone());
    let session_id = sessions.register(session).await;

    // Send connected message
    let connected_msg = ServerMessage::Connected {
        session_id: session_id.clone(),
    };
    let msg_text =
        serde_json::to_string(&connected_msg).map_err(|e| GatewayError::Serialization(e))?;
    ws_tx
        .send(Message::Text(msg_text.into()))
        .await
        .map_err(|e| GatewayError::WebSocket(e.to_string()))?;

    // Subscribe to global event bus and forward events to this session
    let mut event_rx = event_bus.subscribe_all();
    let tx_for_events = tx.clone();
    let session_id_for_events = session_id.clone();
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Some(server_msg) = gateway_event_to_server_message(event) {
                if tx_for_events.send(server_msg).is_err() {
                    debug!("Event forwarder for {} stopped (channel closed)", session_id_for_events);
                    break;
                }
            }
        }
    });

    // Spawn task to forward messages from channel to WebSocket
    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(text) => {
                    if ws_tx.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                }
            }
        }
        debug!("Message forwarder for {} stopped", session_id_clone);
    });

    // Handle incoming messages
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Message::Text(text) = msg {
                    match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            if let Err(e) =
                                handle_client_message(&session_id, client_msg, &sessions, &runtime).await
                            {
                                warn!("Error handling message: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Invalid message format: {}", e);
                        }
                    }
                } else if let Message::Close(_) = msg {
                    break;
                }
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }

    // Cleanup
    sessions.unregister(&session_id).await;
    info!("Session {} disconnected", session_id);

    Ok(())
}

/// Handle a client message.
async fn handle_client_message(
    session_id: &str,
    msg: ClientMessage,
    sessions: &SessionRegistry,
    runtime: &RuntimeService,
) -> Result<()> {
    match msg {
        ClientMessage::Invoke {
            agent_id,
            conversation_id,
            message,
            ..
        } => {
            debug!(
                "Session {} invoking agent {} conversation {}: {}",
                session_id, agent_id, conversation_id, message
            );

            // Create hook context for WebSocket connection
            let mut hook_context = HookContext::web(session_id);
            hook_context.metadata.insert(
                "conversation_id".to_string(),
                serde_json::Value::String(conversation_id.clone()),
            );

            // Invoke the agent via runtime service with hook context
            match runtime
                .invoke_with_hook(&agent_id, &conversation_id, &message, hook_context)
                .await
            {
                Ok(_handle) => {
                    debug!(
                        "Agent {} invocation started for conversation {}",
                        agent_id, conversation_id
                    );
                    // Events will be broadcast via EventBus -> ServerMessage forwarding
                }
                Err(e) => {
                    warn!("Failed to invoke agent {}: {}", agent_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            Some(conversation_id),
                            "invocation_failed",
                            &e,
                        ));
                    }
                }
            }
        }
        ClientMessage::Stop { conversation_id } => {
            debug!(
                "Session {} stopping conversation {}",
                session_id, conversation_id
            );

            match runtime.stop(&conversation_id).await {
                Ok(()) => {
                    debug!("Stop requested for conversation {}", conversation_id);
                }
                Err(e) => {
                    warn!("Failed to stop conversation {}: {}", conversation_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            Some(conversation_id),
                            "stop_failed",
                            &e,
                        ));
                    }
                }
            }
        }
        ClientMessage::Continue { conversation_id, additional_iterations } => {
            debug!(
                "Session {} continuing conversation {} with {} more iterations",
                session_id, conversation_id, additional_iterations
            );

            match runtime.continue_execution(&conversation_id, additional_iterations).await {
                Ok(()) => {
                    debug!("Continuation requested for conversation {}", conversation_id);
                }
                Err(e) => {
                    warn!("Failed to continue conversation {}: {}", conversation_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            Some(conversation_id),
                            "continue_failed",
                            &e,
                        ));
                    }
                }
            }
        }
        ClientMessage::Ping => {
            if let Some(session) = sessions.get(session_id).await {
                let _ = session.send(ServerMessage::Pong);
            }
        }
        ClientMessage::Pause { session_id: exec_session_id } => {
            debug!(
                "Session {} pausing execution session {}",
                session_id, exec_session_id
            );

            match runtime.pause(&exec_session_id).await {
                Ok(()) => {
                    debug!("Execution session {} paused", exec_session_id);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::SessionPaused {
                            session_id: exec_session_id,
                        });
                    }
                }
                Err(e) => {
                    warn!("Failed to pause session {}: {}", exec_session_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            None,
                            "pause_failed",
                            &e,
                        ));
                    }
                }
            }
        }
        ClientMessage::Resume { session_id: exec_session_id } => {
            debug!(
                "Session {} resuming execution session {}",
                session_id, exec_session_id
            );

            match runtime.resume(&exec_session_id).await {
                Ok(()) => {
                    debug!("Execution session {} resumed", exec_session_id);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::SessionResumed {
                            session_id: exec_session_id,
                        });
                    }
                }
                Err(e) => {
                    warn!("Failed to resume session {}: {}", exec_session_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            None,
                            "resume_failed",
                            &e,
                        ));
                    }
                }
            }
        }
        ClientMessage::Cancel { session_id: exec_session_id } => {
            debug!(
                "Session {} cancelling execution session {}",
                session_id, exec_session_id
            );

            match runtime.cancel(&exec_session_id).await {
                Ok(()) => {
                    debug!("Execution session {} cancelled", exec_session_id);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::SessionCancelled {
                            session_id: exec_session_id,
                        });
                    }
                }
                Err(e) => {
                    warn!("Failed to cancel session {}: {}", exec_session_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            None,
                            "cancel_failed",
                            &e,
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Convert a GatewayEvent to a ServerMessage.
fn gateway_event_to_server_message(event: GatewayEvent) -> Option<ServerMessage> {
    match event {
        GatewayEvent::AgentStarted { agent_id, conversation_id } => {
            Some(ServerMessage::AgentStarted { agent_id, conversation_id })
        }
        GatewayEvent::AgentCompleted { agent_id, conversation_id, result } => {
            Some(ServerMessage::AgentCompleted { agent_id, conversation_id, result })
        }
        GatewayEvent::AgentStopped { agent_id, conversation_id, iteration } => {
            Some(ServerMessage::AgentStopped { agent_id, conversation_id, iteration })
        }
        GatewayEvent::Token { conversation_id, delta, .. } => {
            Some(ServerMessage::Token { conversation_id, delta })
        }
        GatewayEvent::Thinking { conversation_id, content, .. } => {
            Some(ServerMessage::Thinking { conversation_id, content })
        }
        GatewayEvent::ToolCall { conversation_id, tool_id, tool_name, args, .. } => {
            Some(ServerMessage::ToolCall {
                conversation_id,
                tool_call_id: tool_id,
                tool: tool_name,
                args,
            })
        }
        GatewayEvent::ToolResult { conversation_id, tool_id, result, error, .. } => {
            Some(ServerMessage::ToolResult {
                conversation_id,
                tool_call_id: tool_id,
                result,
                error,
            })
        }
        GatewayEvent::TurnComplete { conversation_id, message, .. } => {
            Some(ServerMessage::TurnComplete {
                conversation_id,
                final_message: Some(message),
            })
        }
        GatewayEvent::Error { conversation_id, message, .. } => {
            Some(ServerMessage::Error {
                conversation_id,
                code: "execution_error".to_string(),
                message,
            })
        }
        GatewayEvent::IterationUpdate { conversation_id, current, max, .. } => {
            Some(ServerMessage::Iteration {
                conversation_id,
                current,
                max,
            })
        }
        GatewayEvent::ContinuationPrompt { conversation_id, iteration, message, .. } => {
            Some(ServerMessage::ContinuationPrompt {
                conversation_id,
                iteration,
                message,
            })
        }

        // Respond events are handled by the hook system, not WebSocket directly
        GatewayEvent::Respond { conversation_id, message, .. } => {
            Some(ServerMessage::TurnComplete {
                conversation_id,
                final_message: Some(message),
            })
        }

        // Delegation events are internal and don't need WebSocket messages for now
        GatewayEvent::DelegationStarted { .. } | GatewayEvent::DelegationCompleted { .. } => None,

        // New message added - notify frontend to refresh
        GatewayEvent::MessageAdded { conversation_id, role, content } => {
            Some(ServerMessage::MessageAdded { conversation_id, role, content })
        }

        // Token usage update for real-time metrics
        GatewayEvent::TokenUsage { conversation_id, session_id, tokens_in, tokens_out } => {
            Some(ServerMessage::TokenUsage { conversation_id, session_id, tokens_in, tokens_out })
        }
    }
}
