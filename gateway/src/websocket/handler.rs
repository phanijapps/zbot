//! # WebSocket Handler
//!
//! Handles WebSocket connections and message routing.

use super::messages::{ClientMessage, ServerMessage, SubscriptionErrorCode};
use super::session::{SessionRegistry, WsSession};
use super::subscriptions::{SubscribeError, SubscribeResult, SubscriptionManager};
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
    subscriptions: Arc<SubscriptionManager>,
}

impl WebSocketHandler {
    /// Create a new WebSocket handler.
    pub fn new(event_bus: Arc<EventBus>, runtime: Arc<RuntimeService>) -> Self {
        Self {
            event_bus,
            sessions: Arc::new(SessionRegistry::new()),
            runtime,
            subscriptions: Arc::new(SubscriptionManager::new()),
        }
    }

    /// Get the subscription manager.
    pub fn subscriptions(&self) -> Arc<SubscriptionManager> {
        self.subscriptions.clone()
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

        // Spawn background cleanup task for stale clients
        let cleanup_subscriptions = self.subscriptions.clone();
        let mut cleanup_shutdown = shutdown.resubscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let cleaned = cleanup_subscriptions
                            .cleanup_stale_clients(std::time::Duration::from_secs(60))
                            .await;
                        if cleaned > 0 {
                            info!("Cleaned up {} stale WebSocket clients", cleaned);
                        }
                    }
                    _ = cleanup_shutdown.recv() => {
                        debug!("Subscription cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        // Spawn central event router - routes events to subscribed clients
        let router_subscriptions = self.subscriptions.clone();
        let mut router_event_rx = self.event_bus.subscribe_all();
        let mut router_shutdown = shutdown.resubscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = router_event_rx.recv() => {
                        match result {
                            Ok(event) => {
                                // Debug: log delegation events
                                if matches!(&event, GatewayEvent::DelegationCompleted { .. } | GatewayEvent::DelegationStarted { .. }) {
                                    info!("Event router received: {:?}", event);
                                }

                                if let Some(server_msg) = gateway_event_to_server_message(event) {
                                    // Route to subscribed clients only
                                    // Clone conv_id to avoid borrow issues when passing server_msg
                                    let conv_id = server_msg.conversation_id().map(|s| s.to_string());
                                    if let Some(conv_id) = conv_id {
                                        let result = router_subscriptions
                                            .route_event(&conv_id, server_msg)
                                            .await;

                                        // Debug: always log delegation routing results
                                        if result.sent > 0 || conv_id.starts_with("sess-") {
                                            info!(
                                                conv_id = %conv_id,
                                                sent = result.sent,
                                                dropped = result.dropped,
                                                "Routed event"
                                            );
                                        }
                                    } else {
                                        // Global events (like Pong) - broadcast to all
                                        router_subscriptions.broadcast_global(server_msg).await;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Event router receive error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = router_shutdown.recv() => {
                        debug!("Event router shutting down");
                        break;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            debug!("New WebSocket connection from {}", peer_addr);
                            let sessions = self.sessions.clone();
                            let runtime = self.runtime.clone();
                            let subscriptions = self.subscriptions.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, sessions, runtime, subscriptions).await {
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
    runtime: Arc<RuntimeService>,
    subscriptions: Arc<SubscriptionManager>,
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

    // Register with subscription manager for subscription-based routing
    subscriptions.connect(session_id.clone(), tx.clone()).await;

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

    // NOTE: Event routing is now handled by the central event router
    // which routes events through SubscriptionManager to subscribed clients only.
    // Clients must explicitly subscribe to conversations to receive events.

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
                                handle_client_message(&session_id, client_msg, &sessions, &runtime, &subscriptions).await
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
    subscriptions.disconnect(&session_id).await;
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
    subscriptions: &SubscriptionManager,
) -> Result<()> {
    match msg {
        ClientMessage::Invoke {
            agent_id,
            conversation_id,
            message,
            session_id: exec_session_id,
            ..
        } => {
            debug!(
                "Session {} invoking agent {} conversation {} (exec_session: {:?}): {}",
                session_id, agent_id, conversation_id, exec_session_id, message
            );

            // Create hook context for WebSocket connection
            let mut hook_context = HookContext::web(session_id);
            hook_context.metadata.insert(
                "conversation_id".to_string(),
                serde_json::Value::String(conversation_id.clone()),
            );

            // Invoke the agent via runtime service with hook context
            // Pass session_id to continue existing session or None to create new
            match runtime
                .invoke_with_hook(&agent_id, &conversation_id, &message, hook_context, exec_session_id)
                .await
            {
                Ok((_handle, returned_session_id)) => {
                    debug!(
                        "Agent {} invocation started for conversation {} (session: {})",
                        agent_id, conversation_id, returned_session_id
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
        ClientMessage::EndSession { session_id: exec_session_id } => {
            debug!(
                "Session {} ending execution session {}",
                session_id, exec_session_id
            );

            match runtime.end_session(&exec_session_id).await {
                Ok(()) => {
                    info!("Execution session {} ended (completed)", exec_session_id);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::SessionEnded {
                            session_id: exec_session_id.clone(),
                        });
                    }
                }
                Err(e) => {
                    warn!("Failed to end session {}: {}", exec_session_id, e);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::error(
                            None,
                            "end_session_failed",
                            &e,
                        ));
                    }
                }
            }
        }
        ClientMessage::Subscribe { conversation_id } => {
            debug!(
                "Session {} subscribing to conversation {}",
                session_id, conversation_id
            );

            match subscriptions.subscribe(&session_id.to_string(), conversation_id.clone()).await {
                Ok(SubscribeResult::Subscribed { current_sequence }) => {
                    debug!(
                        "Session {} subscribed to {} (seq: {})",
                        session_id, conversation_id, current_sequence
                    );
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::Subscribed {
                            conversation_id,
                            current_sequence,
                        });
                    }
                }
                Ok(SubscribeResult::AlreadySubscribed { current_sequence }) => {
                    debug!(
                        "Session {} already subscribed to {} (seq: {})",
                        session_id, conversation_id, current_sequence
                    );
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::Subscribed {
                            conversation_id,
                            current_sequence,
                        });
                    }
                }
                Err(SubscribeError::ClientNotFound) => {
                    warn!("Subscribe failed: client {} not found", session_id);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::subscription_error(
                            &conversation_id,
                            SubscriptionErrorCode::ServerError,
                            "Client not registered",
                        ));
                    }
                }
                Err(SubscribeError::TooManySubscriptions { limit }) => {
                    warn!("Subscribe failed: client {} exceeded limit of {}", session_id, limit);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::subscription_error(
                            &conversation_id,
                            SubscriptionErrorCode::LimitExceeded,
                            &format!("Maximum {} subscriptions per client", limit),
                        ));
                    }
                }
                Err(SubscribeError::ConversationFull { limit }) => {
                    warn!("Subscribe failed: conversation {} at limit of {}", conversation_id, limit);
                    if let Some(session) = sessions.get(session_id).await {
                        let _ = session.send(ServerMessage::subscription_error(
                            &conversation_id,
                            SubscriptionErrorCode::LimitExceeded,
                            &format!("Conversation has maximum {} subscribers", limit),
                        ));
                    }
                }
            }
        }
        ClientMessage::Unsubscribe { conversation_id } => {
            debug!(
                "Session {} unsubscribing from conversation {}",
                session_id, conversation_id
            );

            subscriptions.unsubscribe(&session_id.to_string(), &conversation_id).await;

            if let Some(session) = sessions.get(session_id).await {
                let _ = session.send(ServerMessage::Unsubscribed { conversation_id });
            }
        }
    }

    Ok(())
}

/// Convert a GatewayEvent to a ServerMessage.
fn gateway_event_to_server_message(event: GatewayEvent) -> Option<ServerMessage> {
    match event {
        GatewayEvent::AgentStarted { agent_id, session_id, conversation_id, .. } => {
            Some(ServerMessage::AgentStarted {
                agent_id,
                conversation_id: conversation_id.unwrap_or_default(),
                session_id,
                seq: None,
            })
        }
        GatewayEvent::AgentCompleted { agent_id, session_id, result, conversation_id, .. } => {
            Some(ServerMessage::AgentCompleted {
                agent_id,
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                result,
                seq: None,
            })
        }
        GatewayEvent::AgentStopped { agent_id, session_id, iteration, conversation_id, .. } => {
            Some(ServerMessage::AgentStopped {
                agent_id,
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                iteration,
                seq: None,
            })
        }
        GatewayEvent::Token { session_id, delta, conversation_id, .. } => {
            Some(ServerMessage::Token {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                delta,
                seq: None,
            })
        }
        GatewayEvent::Thinking { session_id, content, conversation_id, .. } => {
            Some(ServerMessage::Thinking {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                content,
                seq: None,
            })
        }
        GatewayEvent::ToolCall { session_id, tool_id, tool_name, args, conversation_id, .. } => {
            Some(ServerMessage::ToolCall {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                tool_call_id: tool_id,
                tool: tool_name,
                args,
                seq: None,
            })
        }
        GatewayEvent::ToolResult { session_id, tool_id, result, error, conversation_id, .. } => {
            Some(ServerMessage::ToolResult {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                tool_call_id: tool_id,
                result,
                error,
                seq: None,
            })
        }
        GatewayEvent::TurnComplete { session_id, message, conversation_id, .. } => {
            Some(ServerMessage::TurnComplete {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                final_message: Some(message),
                seq: None,
            })
        }
        GatewayEvent::Error { session_id, message, conversation_id, .. } => {
            Some(ServerMessage::Error {
                conversation_id: conversation_id.or_else(|| session_id.clone()),
                code: "execution_error".to_string(),
                message,
                seq: None,
            })
        }
        GatewayEvent::IterationUpdate { session_id, current, max, conversation_id, .. } => {
            Some(ServerMessage::Iteration {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                current,
                max,
                seq: None,
            })
        }
        GatewayEvent::ContinuationPrompt { session_id, iteration, message, conversation_id, .. } => {
            Some(ServerMessage::ContinuationPrompt {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                iteration,
                message,
                seq: None,
            })
        }

        // Respond events are handled by the hook system, not WebSocket directly
        GatewayEvent::Respond { session_id, message, conversation_id, .. } => {
            Some(ServerMessage::TurnComplete {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                final_message: Some(message),
                seq: None,
            })
        }

        // Delegation events - sent to frontend for UI updates
        GatewayEvent::DelegationStarted {
            session_id,
            parent_execution_id,
            child_execution_id,
            parent_agent_id,
            child_agent_id,
            task,
            parent_conversation_id,
            child_conversation_id,
        } => Some(ServerMessage::DelegationStarted {
            parent_agent_id,
            parent_conversation_id: parent_conversation_id.unwrap_or_else(|| parent_execution_id.clone()),
            child_agent_id,
            child_conversation_id: child_conversation_id.unwrap_or_else(|| child_execution_id.clone()),
            task,
            seq: None,
        }),

        GatewayEvent::DelegationCompleted {
            session_id,
            parent_execution_id,
            child_execution_id,
            parent_agent_id,
            child_agent_id,
            result,
            parent_conversation_id,
            child_conversation_id,
        } => Some(ServerMessage::DelegationCompleted {
            parent_agent_id,
            parent_conversation_id: parent_conversation_id.unwrap_or_else(|| parent_execution_id.clone()),
            child_agent_id,
            child_conversation_id: child_conversation_id.unwrap_or_else(|| child_execution_id.clone()),
            result,
            seq: None,
        }),

        // Internal continuation events are handled by the system, not WebSocket
        GatewayEvent::SessionContinuationReady { .. } => None,

        // New message added - notify frontend to refresh
        GatewayEvent::MessageAdded { session_id, role, content, conversation_id, .. } => {
            Some(ServerMessage::MessageAdded {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                role,
                content,
                seq: None,
            })
        }

        // Token usage update for real-time metrics
        GatewayEvent::TokenUsage { session_id, execution_id, tokens_in, tokens_out, conversation_id, .. } => {
            Some(ServerMessage::TokenUsage {
                conversation_id: conversation_id.unwrap_or_else(|| session_id.clone()),
                session_id,
                tokens_in,
                tokens_out,
                seq: None,
            })
        }
    }
}
