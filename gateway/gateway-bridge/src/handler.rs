//! # Bridge Handler
//!
//! Per-worker WebSocket session loop.
//!
//! Handles the full lifecycle of a bridge worker connection:
//! 1. Wait for Hello (5s timeout)
//! 2. Register in BridgeRegistry
//! 3. Send HelloAck
//! 4. Replay pending outbox items
//! 5. Message loop (inbound, ack, fail, resource/capability responses, pong)
//! 6. Cleanup on disconnect

use crate::error::BridgeError;
use crate::outbox::OutboxRepository;
use crate::pending_requests::PendingRequests;
use crate::protocol::{BridgeServerMessage, WorkerMessage};
use crate::push;
use crate::registry::BridgeRegistry;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Heartbeat interval in seconds.
const HEARTBEAT_SECONDS: u64 = 20;

/// Hello handshake timeout in seconds.
const HELLO_TIMEOUT_SECONDS: u64 = 5;

/// Handle a new WebSocket connection from a bridge worker.
///
/// This is called after Axum upgrades the connection. It runs the full
/// worker lifecycle and returns when the connection closes.
pub async fn handle_worker_connection(
    ws_stream: axum::extract::ws::WebSocket,
    registry: Arc<BridgeRegistry>,
    outbox_repo: Arc<OutboxRepository>,
    bus: Option<Arc<dyn gateway_bus::GatewayBus>>,
) {
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Step 1: Wait for Hello
    let hello = tokio::time::timeout(
        std::time::Duration::from_secs(HELLO_TIMEOUT_SECONDS),
        async {
            while let Some(msg) = ws_rx.next().await {
                match msg {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        match serde_json::from_str::<WorkerMessage>(&text) {
                            Ok(WorkerMessage::Hello {
                                adapter_id,
                                capabilities,
                                resources,
                                resume,
                            }) => {
                                return Ok((adapter_id, capabilities, resources, resume));
                            }
                            Ok(_) => {
                                return Err(BridgeError::InvalidMessage(
                                    "Expected Hello as first message".to_string(),
                                ));
                            }
                            Err(e) => {
                                return Err(BridgeError::InvalidMessage(e.to_string()));
                            }
                        }
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        return Err(BridgeError::Channel("Connection closed before Hello".to_string()));
                    }
                    Err(e) => {
                        return Err(BridgeError::Channel(e.to_string()));
                    }
                    _ => continue, // Skip binary/ping/pong frames
                }
            }
            Err(BridgeError::Channel("Connection closed before Hello".to_string()))
        },
    )
    .await;

    let (adapter_id, capabilities, resources, resume) = match hello {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            tracing::warn!("Bridge hello failed: {}", e);
            let err_msg = BridgeServerMessage::Error {
                message: e.to_string(),
            };
            let _ = ws_tx
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&err_msg).unwrap().into(),
                ))
                .await;
            return;
        }
        Err(_) => {
            tracing::warn!("Bridge hello timed out");
            let err_msg = BridgeServerMessage::Error {
                message: format!("Hello timeout after {}s", HELLO_TIMEOUT_SECONDS),
            };
            let _ = ws_tx
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&err_msg).unwrap().into(),
                ))
                .await;
            return;
        }
    };

    tracing::info!(
        adapter_id = %adapter_id,
        capabilities = capabilities.len(),
        resources = resources.len(),
        "Bridge worker connected"
    );

    // Step 2: Create channel for sending messages to this worker
    let (tx, mut rx) = mpsc::unbounded_channel::<BridgeServerMessage>();
    let pending = Arc::new(PendingRequests::new());

    // Step 3: Register in BridgeRegistry
    if let Err(e) = registry
        .register(
            adapter_id.clone(),
            capabilities,
            resources,
            tx,
            pending.clone(),
        )
        .await
    {
        tracing::warn!(adapter_id = %adapter_id, "Registration failed: {}", e);
        let err_msg = BridgeServerMessage::Error {
            message: e.to_string(),
        };
        let _ = ws_tx
            .send(axum::extract::ws::Message::Text(
                serde_json::to_string(&err_msg).unwrap().into(),
            ))
            .await;
        return;
    }

    // Step 4: Send HelloAck
    let ack = BridgeServerMessage::HelloAck {
        server_time: chrono::Utc::now().to_rfc3339(),
        heartbeat_seconds: HEARTBEAT_SECONDS,
    };
    if ws_tx
        .send(axum::extract::ws::Message::Text(
            serde_json::to_string(&ack).unwrap().into(),
        ))
        .await
        .is_err()
    {
        registry.unregister(&adapter_id).await;
        return;
    }

    // Step 5: Replay pending outbox items
    let replay_items = if let Some(ref resume_state) = resume {
        outbox_repo
            .get_since(&adapter_id, &resume_state.last_acked_id)
            .unwrap_or_default()
    } else {
        outbox_repo.get_unacked(&adapter_id).unwrap_or_default()
    };

    if !replay_items.is_empty() {
        tracing::info!(
            adapter_id = %adapter_id,
            count = replay_items.len(),
            "Replaying pending outbox items"
        );
        for item in &replay_items {
            push::push_single_item(&outbox_repo, &registry, &adapter_id, item).await;
        }
    }

    // Step 6: Spawn heartbeat task
    let heartbeat_adapter_id = adapter_id.clone();
    let heartbeat_registry = registry.clone();
    let heartbeat_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(HEARTBEAT_SECONDS));
        loop {
            interval.tick().await;
            if heartbeat_registry
                .send(&heartbeat_adapter_id, BridgeServerMessage::Ping)
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Step 7: Message loop
    let adapter_id_clone = adapter_id.clone();
    let outbox_clone = outbox_repo.clone();
    let pending_clone = pending.clone();
    let bus_clone = bus.clone();

    loop {
        tokio::select! {
            // Messages from the worker
            ws_msg = ws_rx.next() => {
                match ws_msg {
                    Some(Ok(axum::extract::ws::Message::Text(text))) => {
                        match serde_json::from_str::<WorkerMessage>(&text) {
                            Ok(msg) => {
                                handle_worker_message(
                                    &adapter_id_clone,
                                    msg,
                                    &outbox_clone,
                                    &pending_clone,
                                    bus_clone.as_deref(),
                                ).await;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    adapter_id = %adapter_id_clone,
                                    "Invalid message from worker: {}",
                                    e
                                );
                            }
                        }
                    }
                    Some(Ok(axum::extract::ws::Message::Close(_))) | None => {
                        tracing::info!(adapter_id = %adapter_id_clone, "Worker disconnected");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!(adapter_id = %adapter_id_clone, "WS error: {}", e);
                        break;
                    }
                    _ => {} // Skip binary/ping/pong
                }
            }

            // Messages to send to the worker (from registry.send())
            server_msg = rx.recv() => {
                match server_msg {
                    Some(msg) => {
                        let json = match serde_json::to_string(&msg) {
                            Ok(j) => j,
                            Err(e) => {
                                tracing::warn!("Failed to serialize server message: {}", e);
                                continue;
                            }
                        };
                        if ws_tx.send(axum::extract::ws::Message::Text(json.into())).await.is_err() {
                            tracing::info!(adapter_id = %adapter_id_clone, "Send failed, disconnecting");
                            break;
                        }
                    }
                    None => {
                        // Channel closed
                        break;
                    }
                }
            }
        }
    }

    // Step 8: Cleanup
    heartbeat_handle.abort();
    pending.cancel_all();
    registry.unregister(&adapter_id).await;

    // Reset inflight items back to pending for retry on reconnect
    if let Err(e) = outbox_repo.reset_inflight(&adapter_id) {
        tracing::warn!(adapter_id = %adapter_id, "Failed to reset inflight: {}", e);
    }

    tracing::info!(adapter_id = %adapter_id, "Bridge worker session ended");
}

/// Handle a parsed worker message.
async fn handle_worker_message(
    adapter_id: &str,
    msg: WorkerMessage,
    outbox_repo: &OutboxRepository,
    pending: &PendingRequests,
    bus: Option<&dyn gateway_bus::GatewayBus>,
) {
    match msg {
        WorkerMessage::Ack { outbox_id } => {
            tracing::debug!(adapter_id = %adapter_id, outbox_id = %outbox_id, "ACK received");
            if let Err(e) = outbox_repo.mark_sent(&outbox_id) {
                tracing::warn!("Failed to mark outbox sent: {}", e);
            }
        }

        WorkerMessage::Fail {
            outbox_id,
            error,
            retry_after_seconds,
        } => {
            tracing::warn!(
                adapter_id = %adapter_id,
                outbox_id = %outbox_id,
                error = %error,
                "FAIL received from worker"
            );
            let retry_after = retry_after_seconds.map(|s| {
                chrono::Utc::now() + chrono::Duration::seconds(s as i64)
            });
            if let Err(e) = outbox_repo.mark_failed(&outbox_id, &error, retry_after) {
                tracing::warn!("Failed to mark outbox failed: {}", e);
            }
        }

        WorkerMessage::ResourceResponse { request_id, data } => {
            tracing::debug!(adapter_id = %adapter_id, request_id = %request_id, "ResourceResponse received");
            if !pending.resolve(&request_id, data) {
                tracing::warn!("No pending request for: {}", request_id);
            }
        }

        WorkerMessage::CapabilityResponse { request_id, result } => {
            tracing::debug!(adapter_id = %adapter_id, request_id = %request_id, "CapabilityResponse received");
            if !pending.resolve(&request_id, result) {
                tracing::warn!("No pending request for: {}", request_id);
            }
        }

        WorkerMessage::Inbound {
            text,
            thread_id,
            sender,
            agent_id,
            metadata,
        } => {
            tracing::info!(adapter_id = %adapter_id, "Inbound message from worker");

            if let Some(bus) = bus {
                let mut request = gateway_bus::SessionRequest::new(
                    agent_id.unwrap_or_else(|| "root".to_string()),
                    text,
                )
                .with_respond_to(vec![adapter_id.to_string()])
                .with_connector_id(adapter_id.to_string());

                // Set source to "connector" via serde (avoids execution_state dep)
                request.source = serde_json::from_str("\"connector\"")
                    .unwrap_or_default();

                if let Some(tid) = thread_id {
                    request = request.with_thread_id(tid);
                }
                if let Some(meta) = metadata {
                    request = request.with_metadata(meta);
                }

                match bus.submit(request).await {
                    Ok(handle) => {
                        tracing::info!(
                            adapter_id = %adapter_id,
                            session_id = %handle.session_id,
                            "Inbound message submitted"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            adapter_id = %adapter_id,
                            "Failed to submit inbound: {:?}",
                            e
                        );
                    }
                }
            } else {
                tracing::warn!(adapter_id = %adapter_id, "No bus available for inbound messages");
            }

            // Suppress unused variable warnings for sender (logged but not yet used for routing)
            let _ = sender;
        }

        WorkerMessage::Pong => {
            tracing::trace!(adapter_id = %adapter_id, "Pong received");
        }

        WorkerMessage::Hello { .. } => {
            tracing::warn!(adapter_id = %adapter_id, "Unexpected Hello after handshake");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(HEARTBEAT_SECONDS, 20);
        assert_eq!(HELLO_TIMEOUT_SECONDS, 5);
    }
}
