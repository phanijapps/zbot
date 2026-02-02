# Real-Time Event System v3 (Final)

## Status: APPROVED FOR IMPLEMENTATION

## Context

This is part of the AgentZero project - a multi-agent orchestration platform. See `memory-bank/agent-orchestration.md` for the execution model.

**Deployment Context**: Private network, not exposed to public internet. Authentication/authorization deferred.

**Problem**: UI doesn't receive real-time updates for delegation events. Events broadcast to all clients and filtered client-side, which breaks for delegation events using `parent_conversation_id`.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    GATEWAY SERVER                            │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │            SubscriptionManager                       │   │
│   │                                                      │   │
│   │   Single RwLock<SubscriptionState> protecting:       │   │
│   │   - clients: Map<ClientId, Client>                   │   │
│   │   - subscriptions: Map<ConvId, Set<ClientId>>        │   │
│   │   - client_subs: Map<ClientId, Set<ConvId>>          │   │
│   │   - sequences: Map<ConvId, u64>                      │   │
│   │                                                      │   │
│   │   + Background dead client cleanup task              │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              EventRouter                             │   │
│   │                                                      │   │
│   │   - Serializes events per conversation               │   │
│   │   - Assigns sequence numbers atomically with send    │   │
│   │   - Routes via SubscriptionManager                   │   │
│   └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## Server Implementation

### Data Structures

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{mpsc, RwLock};

/// Client connection state
struct Client {
    id: ClientId,
    sender: mpsc::Sender<ServerMessage>,
    connected_at: std::time::Instant,
    last_activity: std::time::Instant,
    subscription_count: usize,
    /// Track if channel has failed (for cleanup)
    channel_healthy: bool,
}

/// All subscription state protected by a SINGLE lock
struct SubscriptionState {
    clients: HashMap<ClientId, Client>,
    subscriptions: HashMap<String, HashSet<ClientId>>,
    client_subscriptions: HashMap<ClientId, HashSet<String>>,
    sequence_numbers: HashMap<String, u64>,
}

pub struct SubscriptionManager {
    state: RwLock<SubscriptionState>,
    max_subscriptions_per_client: usize,
    max_subscribers_per_conversation: usize,
    metrics: Arc<SubscriptionMetrics>,
}

pub struct SubscriptionMetrics {
    pub total_clients: AtomicU64,
    pub total_subscriptions: AtomicU64,
    pub events_routed: AtomicU64,
    pub events_dropped: AtomicU64,
    pub dead_clients_cleaned: AtomicU64,
}
```

### Core Operations (Race-Condition Free)

```rust
impl SubscriptionManager {
    const DEFAULT_MAX_SUBS_PER_CLIENT: usize = 50;
    const DEFAULT_MAX_SUBS_PER_CONV: usize = 1000;

    pub fn new() -> Self {
        Self {
            state: RwLock::new(SubscriptionState {
                clients: HashMap::new(),
                subscriptions: HashMap::new(),
                client_subscriptions: HashMap::new(),
                sequence_numbers: HashMap::new(),
            }),
            max_subscriptions_per_client: Self::DEFAULT_MAX_SUBS_PER_CLIENT,
            max_subscribers_per_conversation: Self::DEFAULT_MAX_SUBS_PER_CONV,
            metrics: Arc::new(SubscriptionMetrics::default()),
        }
    }

    /// Register a new client - atomic operation
    pub async fn connect(&self, client_id: ClientId, sender: mpsc::Sender<ServerMessage>) {
        let mut state = self.state.write().await;
        state.clients.insert(client_id.clone(), Client {
            id: client_id.clone(),
            sender,
            connected_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            subscription_count: 0,
            channel_healthy: true,
        });
        state.client_subscriptions.insert(client_id, HashSet::new());
        self.metrics.total_clients.fetch_add(1, Ordering::Relaxed);
    }

    /// Disconnect and cleanup - atomic, no race conditions
    pub async fn disconnect(&self, client_id: &ClientId) {
        let mut state = self.state.write().await;
        self.disconnect_internal(&mut state, client_id);
    }

    /// Internal disconnect with state already locked
    fn disconnect_internal(&self, state: &mut SubscriptionState, client_id: &ClientId) {
        if let Some(conversations) = state.client_subscriptions.remove(client_id) {
            for conv_id in &conversations {
                if let Some(subscribers) = state.subscriptions.get_mut(conv_id) {
                    subscribers.remove(client_id);
                    if subscribers.is_empty() {
                        state.subscriptions.remove(conv_id);
                        // Also clean up sequence numbers for empty conversations
                        state.sequence_numbers.remove(conv_id);
                    }
                }
            }
            self.metrics.total_subscriptions.fetch_sub(
                conversations.len() as u64,
                Ordering::Relaxed
            );
        }
        state.clients.remove(client_id);
        self.metrics.total_clients.fetch_sub(1, Ordering::Relaxed);
    }

    /// Subscribe - atomic with limit checks
    pub async fn subscribe(
        &self,
        client_id: &ClientId,
        conversation_id: String
    ) -> Result<SubscribeResult, SubscribeError> {
        let mut state = self.state.write().await;

        let client = state.clients.get_mut(client_id)
            .ok_or(SubscribeError::ClientNotFound)?;

        if client.subscription_count >= self.max_subscriptions_per_client {
            return Err(SubscribeError::TooManySubscriptions {
                limit: self.max_subscriptions_per_client,
            });
        }

        let subscribers = state.subscriptions
            .entry(conversation_id.clone())
            .or_insert_with(HashSet::new);

        if subscribers.len() >= self.max_subscribers_per_conversation {
            return Err(SubscribeError::ConversationFull {
                limit: self.max_subscribers_per_conversation,
            });
        }

        if subscribers.contains(client_id) {
            let current_seq = *state.sequence_numbers.get(&conversation_id).unwrap_or(&0);
            return Ok(SubscribeResult::AlreadySubscribed { current_sequence: current_seq });
        }

        // Atomic update of both maps
        subscribers.insert(client_id.clone());
        state.client_subscriptions
            .get_mut(client_id)
            .unwrap()
            .insert(conversation_id.clone());
        client.subscription_count += 1;

        let current_seq = *state.sequence_numbers
            .entry(conversation_id)
            .or_insert(0);

        self.metrics.total_subscriptions.fetch_add(1, Ordering::Relaxed);

        Ok(SubscribeResult::Subscribed { current_sequence: current_seq })
    }

    /// Unsubscribe - atomic
    pub async fn unsubscribe(&self, client_id: &ClientId, conversation_id: &str) {
        let mut state = self.state.write().await;

        let mut should_cleanup_conv = false;

        if let Some(subscribers) = state.subscriptions.get_mut(conversation_id) {
            if subscribers.remove(client_id) {
                self.metrics.total_subscriptions.fetch_sub(1, Ordering::Relaxed);

                if let Some(client) = state.clients.get_mut(client_id) {
                    client.subscription_count = client.subscription_count.saturating_sub(1);
                }

                if subscribers.is_empty() {
                    should_cleanup_conv = true;
                }
            }
        }

        // Clean up empty conversation state
        if should_cleanup_conv {
            state.subscriptions.remove(conversation_id);
            state.sequence_numbers.remove(conversation_id);
        }

        if let Some(client_subs) = state.client_subscriptions.get_mut(client_id) {
            client_subs.remove(conversation_id);
        }
    }

    /// Route event with atomic sequence assignment
    /// This is the key method - sequence assigned atomically with routing
    pub async fn route_event(
        &self,
        conversation_id: &str,
        mut event: ServerMessage
    ) -> RoutingResult {
        let mut state = self.state.write().await;

        // Assign sequence number atomically
        let seq = state.sequence_numbers
            .entry(conversation_id.to_string())
            .or_insert(0);
        *seq += 1;
        event.set_sequence(*seq);

        let Some(subscribers) = state.subscriptions.get(conversation_id) else {
            return RoutingResult { sent: 0, dropped: 0, dead_clients: vec![] };
        };

        let mut sent = 0;
        let mut dropped = 0;
        let mut dead_clients = Vec::new();

        for client_id in subscribers.iter() {
            if let Some(client) = state.clients.get_mut(client_id) {
                match client.sender.try_send(event.clone()) {
                    Ok(()) => {
                        sent += 1;
                        client.last_activity = std::time::Instant::now();
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        dropped += 1;
                        // Channel full but client still alive
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        dropped += 1;
                        // Mark for cleanup
                        client.channel_healthy = false;
                        dead_clients.push(client_id.clone());
                    }
                }
            }
        }

        // Clean up dead clients while we still hold the lock
        for dead_id in &dead_clients {
            self.disconnect_internal(&mut state, dead_id);
            self.metrics.dead_clients_cleaned.fetch_add(1, Ordering::Relaxed);
        }

        self.metrics.events_routed.fetch_add(sent, Ordering::Relaxed);
        self.metrics.events_dropped.fetch_add(dropped, Ordering::Relaxed);

        RoutingResult { sent, dropped, dead_clients }
    }

    /// Broadcast global event to all clients
    pub async fn broadcast_global(&self, event: ServerMessage) {
        let state = self.state.read().await;

        for client in state.clients.values() {
            if client.channel_healthy {
                let _ = client.sender.try_send(event.clone());
            }
        }
    }

    /// Background cleanup task - call periodically (e.g., every 30s)
    pub async fn cleanup_stale_clients(&self, timeout: std::time::Duration) -> usize {
        let mut state = self.state.write().await;
        let now = std::time::Instant::now();

        let stale: Vec<ClientId> = state.clients
            .iter()
            .filter(|(_, c)| !c.channel_healthy || now.duration_since(c.last_activity) > timeout)
            .map(|(id, _)| id.clone())
            .collect();

        let count = stale.len();
        for client_id in stale {
            self.disconnect_internal(&mut state, &client_id);
        }

        self.metrics.dead_clients_cleaned.fetch_add(count as u64, Ordering::Relaxed);
        count
    }
}

pub struct RoutingResult {
    pub sent: u64,
    pub dropped: u64,
    pub dead_clients: Vec<ClientId>,
}

#[derive(Debug)]
pub enum SubscribeError {
    ClientNotFound,
    TooManySubscriptions { limit: usize },
    ConversationFull { limit: usize },
}

#[derive(Debug)]
pub enum SubscribeResult {
    Subscribed { current_sequence: u64 },
    AlreadySubscribed { current_sequence: u64 },
}
```

### Spawning Cleanup Task

```rust
// In gateway startup
let manager = Arc::new(SubscriptionManager::new());
let cleanup_manager = manager.clone();

tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        let cleaned = cleanup_manager.cleanup_stale_clients(Duration::from_secs(60)).await;
        if cleaned > 0 {
            tracing::info!(cleaned = cleaned, "Cleaned up stale clients");
        }
    }
});
```

---

## Protocol Specification

### Client → Server

```typescript
type ClientMessage =
  | { type: "subscribe"; conversation_id: string }
  | { type: "unsubscribe"; conversation_id: string }
  | { type: "execute"; agent_id: string; conversation_id: string; message: string; session_id?: string }
  | { type: "stop"; conversation_id: string }
  | { type: "end_session"; session_id: string }
  | { type: "ping" };
```

### Server → Client

```typescript
// ═══════════════════════════════════════════════════════════
// SUBSCRIPTION RESPONSES
// ═══════════════════════════════════════════════════════════

interface SubscribedMessage {
  type: "subscribed";
  conversation_id: string;
  current_sequence: number;
}

interface UnsubscribedMessage {
  type: "unsubscribed";
  conversation_id: string;
}

interface SubscriptionErrorMessage {
  type: "subscription_error";
  conversation_id: string;
  code: "NOT_FOUND" | "LIMIT_EXCEEDED" | "UNAUTHORIZED" | "SERVER_ERROR";
  message: string;
}

// ═══════════════════════════════════════════════════════════
// GLOBAL EVENTS (no subscription required)
// ═══════════════════════════════════════════════════════════

interface StatsUpdateMessage {
  type: "stats_update";
  sessions_running: number;
  sessions_completed: number;
  sessions_crashed: number;
}

interface SessionNotificationMessage {
  type: "session_notification";
  action: "created" | "completed" | "crashed";
  session_id: string;
  agent_id: string;
}

// ═══════════════════════════════════════════════════════════
// CONVERSATION EVENTS (with sequence numbers)
// ═══════════════════════════════════════════════════════════

interface ConversationEvent {
  conversation_id: string;
  seq: number;
}

interface AgentStartedMessage extends ConversationEvent {
  type: "agent_started";
  agent_id: string;
  session_id: string;
}

interface TokenMessage extends ConversationEvent {
  type: "token";
  delta: string;
}

interface ToolCallMessage extends ConversationEvent {
  type: "tool_call";
  tool_name: string;
  tool_id: string;
}

interface ToolResultMessage extends ConversationEvent {
  type: "tool_result";
  tool_name: string;
  tool_id: string;
  result: string;
  is_error: boolean;
}

interface DelegationStartedMessage extends ConversationEvent {
  type: "delegation_started";
  child_agent_id: string;
  child_conversation_id: string;
  task: string;
}

interface DelegationCompletedMessage extends ConversationEvent {
  type: "delegation_completed";
  child_agent_id: string;
  child_conversation_id: string;
  result?: string;
  error?: string;
}

interface AgentCompletedMessage extends ConversationEvent {
  type: "agent_completed";
  result?: string;
}

interface ErrorMessage extends ConversationEvent {
  type: "error";
  error: string;
}

// ═══════════════════════════════════════════════════════════
// CONNECTION MANAGEMENT
// ═══════════════════════════════════════════════════════════

interface PongMessage {
  type: "pong";
  server_time: number;
}
```

---

## Frontend Implementation

### Types

```typescript
// ═══════════════════════════════════════════════════════════
// types.ts - Add to existing transport types
// ═══════════════════════════════════════════════════════════

export type ConnectionState =
  | { status: 'disconnected'; reason?: 'user' | 'server' | 'network' }
  | { status: 'connecting' }
  | { status: 'connected' }
  | { status: 'reconnecting'; attempt: number; maxAttempts: number }
  | { status: 'failed'; error: string };

export type ConversationCallback = (event: ConversationEvent) => void;
export type GlobalCallback = (event: GlobalEvent) => void;
export type ConnectionStateCallback = (state: ConnectionState) => void;
export type ErrorCallback = (error: SubscriptionErrorMessage) => void;
export type UnsubscribeFn = () => void;

export interface SubscriptionOptions {
  onEvent: ConversationCallback;
  onError?: ErrorCallback;
  onConfirmed?: (seq: number) => void;
}

interface SubscriptionState {
  callbacks: Set<ConversationCallback>;
  errorCallbacks: Map<ConversationCallback, ErrorCallback>;  // Track per-callback
  confirmed: boolean;
  lastSeq: number;
}
```

### HttpTransport Extensions

```typescript
// ═══════════════════════════════════════════════════════════
// Add to existing HttpTransport class in http.ts
// ═══════════════════════════════════════════════════════════

export class HttpTransport implements Transport {
  // ... existing fields ...

  // ─────────────────────────────────────────────────────────
  // NEW: Subscription state
  // ─────────────────────────────────────────────────────────
  private conversationSubscriptions: Map<string, SubscriptionState> = new Map();
  private globalCallbacks: Set<GlobalCallback> = new Set();
  private connectionStateCallbacks: Set<ConnectionStateCallback> = new Set();
  private connectionState: ConnectionState = { status: 'disconnected' };

  // Heartbeat
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private lastPong: number = Date.now();
  private readonly PING_INTERVAL = 15000;
  private readonly PONG_TIMEOUT = 30000;

  // Browser event handlers (stored for cleanup)
  private visibilityHandler: (() => void) | null = null;
  private onlineHandler: (() => void) | null = null;
  private sleepCheckInterval: ReturnType<typeof setInterval> | null = null;

  // ─────────────────────────────────────────────────────────
  // CONNECTION STATE
  // ─────────────────────────────────────────────────────────

  private setConnectionState(state: ConnectionState): void {
    this.connectionState = state;
    // Use snapshot to avoid iterator invalidation if callback modifies set
    const callbacks = [...this.connectionStateCallbacks];
    for (const callback of callbacks) {
      try {
        callback(state);
      } catch (e) {
        console.error('[Transport] Connection state callback error:', e);
      }
    }
  }

  public getConnectionState(): ConnectionState {
    return this.connectionState;
  }

  public onConnectionStateChange(callback: ConnectionStateCallback): UnsubscribeFn {
    this.connectionStateCallbacks.add(callback);
    callback(this.connectionState);
    return () => this.connectionStateCallbacks.delete(callback);
  }

  // ─────────────────────────────────────────────────────────
  // HEARTBEAT
  // ─────────────────────────────────────────────────────────

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.lastPong = Date.now();

    this.pingInterval = setInterval(() => {
      if (Date.now() - this.lastPong > this.PONG_TIMEOUT) {
        console.warn('[Transport] Ping timeout, reconnecting');
        this.ws?.close(4000, 'Ping timeout');
        return;
      }

      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, this.PING_INTERVAL);
  }

  private stopHeartbeat(): void {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }

  // ─────────────────────────────────────────────────────────
  // BROWSER EVENT HANDLERS (with proper cleanup)
  // ─────────────────────────────────────────────────────────

  private setupBrowserEventHandlers(): void {
    this.cleanupBrowserEventHandlers();

    // Handle tab visibility changes
    this.visibilityHandler = () => {
      if (document.visibilityState === 'visible') {
        if (this.ws?.readyState !== WebSocket.OPEN) {
          this.reconnectAttempts = 0;
          this.connect();
        }
      }
    };
    document.addEventListener('visibilitychange', this.visibilityHandler);

    // Handle network online/offline
    this.onlineHandler = () => {
      if (this.connectionState.status !== 'connected') {
        this.reconnectAttempts = 0;
        this.connect();
      }
    };
    window.addEventListener('online', this.onlineHandler);

    // NOTE: setInterval is throttled in background tabs.
    // We rely primarily on visibilitychange for sleep detection.
    // This is a backup for edge cases.
    let lastCheck = Date.now();
    this.sleepCheckInterval = setInterval(() => {
      const now = Date.now();
      if (now - lastCheck > 60000) {
        console.log('[Transport] Detected wake from sleep');
        if (this.ws?.readyState === WebSocket.OPEN) {
          this.ws.close(4001, 'Wake from sleep');
        }
      }
      lastCheck = now;
    }, 10000);
  }

  private cleanupBrowserEventHandlers(): void {
    if (this.visibilityHandler) {
      document.removeEventListener('visibilitychange', this.visibilityHandler);
      this.visibilityHandler = null;
    }
    if (this.onlineHandler) {
      window.removeEventListener('online', this.onlineHandler);
      this.onlineHandler = null;
    }
    if (this.sleepCheckInterval) {
      clearInterval(this.sleepCheckInterval);
      this.sleepCheckInterval = null;
    }
  }

  // ─────────────────────────────────────────────────────────
  // SUBSCRIPTION API
  // ─────────────────────────────────────────────────────────

  public subscribeConversation(
    conversationId: string,
    options: SubscriptionOptions
  ): UnsubscribeFn {
    let state = this.conversationSubscriptions.get(conversationId);

    if (!state) {
      state = {
        callbacks: new Set(),
        errorCallbacks: new Map(),
        confirmed: false,
        lastSeq: 0,
      };
      this.conversationSubscriptions.set(conversationId, state);
      this.sendSubscribe(conversationId);
    }

    // Wrap callback to include sequence tracking
    const wrappedCallback: ConversationCallback = (event) => {
      const currentState = this.conversationSubscriptions.get(conversationId);
      if (currentState && event.seq) {
        if (event.seq > currentState.lastSeq + 1 && currentState.lastSeq > 0) {
          console.warn(
            `[Transport] Sequence gap: expected ${currentState.lastSeq + 1}, got ${event.seq}. ` +
            `Recommend refreshing conversation state via API.`
          );
        }
        currentState.lastSeq = event.seq;
      }
      options.onEvent(event);
    };

    state.callbacks.add(wrappedCallback);

    // Track error callback per wrapped callback for proper cleanup
    if (options.onError) {
      state.errorCallbacks.set(wrappedCallback, options.onError);
    }

    return () => {
      const state = this.conversationSubscriptions.get(conversationId);
      if (!state) return;

      state.callbacks.delete(wrappedCallback);
      state.errorCallbacks.delete(wrappedCallback);

      if (state.callbacks.size === 0) {
        this.conversationSubscriptions.delete(conversationId);
        this.sendUnsubscribe(conversationId);
      }
    };
  }

  public onGlobalEvent(callback: GlobalCallback): UnsubscribeFn {
    this.globalCallbacks.add(callback);
    return () => this.globalCallbacks.delete(callback);
  }

  // ─────────────────────────────────────────────────────────
  // SEND HELPERS
  // ─────────────────────────────────────────────────────────

  private sendSubscribe(conversationId: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: 'subscribe',
        conversation_id: conversationId,
      }));
    }
  }

  private sendUnsubscribe(conversationId: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: 'unsubscribe',
        conversation_id: conversationId,
      }));
    }
  }

  // ─────────────────────────────────────────────────────────
  // MESSAGE HANDLING (integrate with existing onmessage)
  // ─────────────────────────────────────────────────────────

  // Modify existing onmessage handler to route through these:

  private handleWebSocketMessage(data: unknown): void {
    const message = data as ServerMessage;

    // Try new subscription system first
    if (this.handleSubscriptionMessage(message)) return;
    if (this.handleGlobalMessage(message)) return;
    if (this.handleConversationMessage(message)) return;

    // Fall back to legacy event handling for backwards compatibility
    this.handleEvent(message as StreamEvent);
  }

  private handleSubscriptionMessage(message: ServerMessage): boolean {
    switch (message.type) {
      case 'subscribed': {
        const state = this.conversationSubscriptions.get(message.conversation_id);
        if (state) {
          state.confirmed = true;
          state.lastSeq = message.current_sequence;
          console.log(`[Transport] Subscribed to ${message.conversation_id} at seq ${message.current_sequence}`);
        }
        return true;
      }

      case 'unsubscribed': {
        console.log(`[Transport] Unsubscribed from ${message.conversation_id}`);
        return true;
      }

      case 'subscription_error': {
        console.error(`[Transport] Subscription error: ${message.code} - ${message.message}`);
        const state = this.conversationSubscriptions.get(message.conversation_id);
        if (state) {
          // Notify all error callbacks
          for (const errorCb of state.errorCallbacks.values()) {
            try { errorCb(message); } catch (e) { console.error(e); }
          }
        }
        this.conversationSubscriptions.delete(message.conversation_id);
        return true;
      }

      case 'pong': {
        this.lastPong = Date.now();
        return true;
      }

      default:
        return false;
    }
  }

  private handleGlobalMessage(message: ServerMessage): boolean {
    if (message.type === 'stats_update' || message.type === 'session_notification') {
      const callbacks = [...this.globalCallbacks];
      for (const callback of callbacks) {
        try { callback(message); } catch (e) { console.error(e); }
      }
      return true;
    }
    return false;
  }

  private handleConversationMessage(message: ConversationEvent): boolean {
    if (!message.conversation_id) return false;

    const state = this.conversationSubscriptions.get(message.conversation_id);
    if (state) {
      const callbacks = [...state.callbacks];
      for (const callback of callbacks) {
        try { callback(message); } catch (e) { console.error(e); }
      }
      return true;
    }
    return false;
  }

  // ─────────────────────────────────────────────────────────
  // RECONNECTION
  // ─────────────────────────────────────────────────────────

  private resubscribeAll(): void {
    for (const [conversationId, state] of this.conversationSubscriptions) {
      state.confirmed = false;
      // Don't reset lastSeq - we want to detect gaps after reconnect
      this.sendSubscribe(conversationId);
    }
  }

  /**
   * Manual reconnect - resets attempt counter and tries again.
   * Use this instead of page reload.
   */
  public async reconnect(): Promise<void> {
    this.reconnectAttempts = 0;
    if (this.ws) {
      this.ws.close();
    }
    await this.connect();
  }

  // ─────────────────────────────────────────────────────────
  // MODIFY EXISTING connect() METHOD
  // ─────────────────────────────────────────────────────────

  async connect(): Promise<TransportResult<void>> {
    // ... existing code ...

    // Add these at appropriate points:

    // On first connect, setup browser handlers
    // this.setupBrowserEventHandlers();

    // On successful connect:
    // this.setConnectionState({ status: 'connected' });
    // this.startHeartbeat();
    // this.resubscribeAll();

    // On close:
    // this.stopHeartbeat();
    // this.setConnectionState({ status: 'reconnecting', attempt: this.reconnectAttempts, maxAttempts: 10 });

    // On max retries:
    // this.setConnectionState({ status: 'failed', error: 'Max reconnect attempts reached' });
  }

  // ─────────────────────────────────────────────────────────
  // MODIFY EXISTING disconnect() METHOD
  // ─────────────────────────────────────────────────────────

  async disconnect(): Promise<void> {
    this.cleanupBrowserEventHandlers();
    this.stopHeartbeat();
    this.setConnectionState({ status: 'disconnected', reason: 'user' });
    // ... existing cleanup ...
  }
}
```

### React Hooks

```typescript
// ═══════════════════════════════════════════════════════════
// hooks/useConversationEvents.ts
// ═══════════════════════════════════════════════════════════

import { useEffect, useRef } from 'react';
import { getTransport } from '@/services/transport';
import type { ConversationEvent, SubscriptionErrorMessage } from '@/services/transport/types';

interface UseConversationEventsOptions {
  onError?: (error: SubscriptionErrorMessage) => void;
}

export function useConversationEvents(
  conversationId: string | null,
  onEvent: (event: ConversationEvent) => void,
  options: UseConversationEventsOptions = {}
) {
  const onEventRef = useRef(onEvent);
  const onErrorRef = useRef(options.onError);

  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    onErrorRef.current = options.onError;
  }, [options.onError]);

  useEffect(() => {
    if (!conversationId) return;

    let unsubscribe: UnsubscribeFn | null = null;
    let cancelled = false;

    const subscribe = async () => {
      try {
        const transport = await getTransport();

        if (cancelled) return;

        unsubscribe = transport.subscribeConversation(conversationId, {
          onEvent: (event) => onEventRef.current(event),
          onError: (error) => onErrorRef.current?.(error),
        });
      } catch (error) {
        if (!cancelled && onErrorRef.current) {
          onErrorRef.current({
            type: 'subscription_error',
            conversation_id: conversationId,
            code: 'SERVER_ERROR',
            message: String(error),
          });
        }
      }
    };

    subscribe();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, [conversationId]);
}

// ═══════════════════════════════════════════════════════════
// hooks/useConnectionState.ts
// ═══════════════════════════════════════════════════════════

import { useState, useEffect } from 'react';
import { getTransport } from '@/services/transport';
import type { ConnectionState } from '@/services/transport/types';

export function useConnectionState(): ConnectionState {
  const [state, setState] = useState<ConnectionState>({ status: 'disconnected' });

  useEffect(() => {
    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;
      unsubscribe = transport.onConnectionStateChange(setState);
    };

    setup();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, []);

  return state;
}

// ═══════════════════════════════════════════════════════════
// hooks/useGlobalEvents.ts
// ═══════════════════════════════════════════════════════════

import { useEffect, useRef } from 'react';
import { getTransport } from '@/services/transport';
import type { GlobalEvent } from '@/services/transport/types';

export function useGlobalEvents(onEvent: (event: GlobalEvent) => void) {
  const onEventRef = useRef(onEvent);

  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;
      unsubscribe = transport.onGlobalEvent((event) => onEventRef.current(event));
    };

    setup();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, []);
}
```

### Connection Status Component

```typescript
// ═══════════════════════════════════════════════════════════
// components/ConnectionStatus.tsx
// ═══════════════════════════════════════════════════════════

import { useConnectionState } from '@/hooks/useConnectionState';
import { getTransport } from '@/services/transport';
import { Wifi, WifiOff, Loader2, AlertCircle } from 'lucide-react';

export function ConnectionStatus() {
  const state = useConnectionState();

  const handleReconnect = async () => {
    const transport = await getTransport();
    transport.reconnect();
  };

  switch (state.status) {
    case 'connected':
      return null;

    case 'connecting':
      return (
        <div className="flex items-center gap-2 text-yellow-600 text-sm px-3 py-1.5 bg-yellow-50 rounded-lg">
          <Loader2 className="w-4 h-4 animate-spin" />
          Connecting...
        </div>
      );

    case 'reconnecting':
      return (
        <div className="flex items-center gap-2 text-yellow-600 text-sm px-3 py-1.5 bg-yellow-50 rounded-lg">
          <Loader2 className="w-4 h-4 animate-spin" />
          Reconnecting ({state.attempt}/{state.maxAttempts})...
        </div>
      );

    case 'disconnected':
      return (
        <div className="flex items-center gap-2 text-gray-500 text-sm px-3 py-1.5 bg-gray-100 rounded-lg">
          <WifiOff className="w-4 h-4" />
          Disconnected
          <button onClick={handleReconnect} className="underline ml-1">
            Reconnect
          </button>
        </div>
      );

    case 'failed':
      return (
        <div className="flex items-center gap-2 text-red-600 text-sm px-3 py-1.5 bg-red-50 rounded-lg">
          <AlertCircle className="w-4 h-4" />
          Connection failed
          <button onClick={handleReconnect} className="underline ml-1">
            Retry
          </button>
        </div>
      );
  }
}
```

---

## Important Design Decisions

### 1. Sequence Gap Handling

**Decision**: Log warning, recommend API refresh. No automatic recovery.

**Rationale**: Implementing event replay requires event storage and adds complexity. For a private network with generally reliable connections, gaps are rare. When they occur, a simple API call to refresh conversation state is sufficient.

**Client behavior on gap**:
```typescript
if (event.seq > lastSeq + 1) {
  console.warn('Sequence gap detected. Refresh conversation via API.');
  // Optionally: trigger onError callback so UI can show refresh button
}
```

### 2. Server Restart Behavior

**Decision**: Sequence numbers reset to 0. Client treats reconnection as state reset.

**Rationale**: Persisting sequence numbers adds database complexity with little benefit for a private network. Clients should:
1. Detect disconnect
2. Reconnect
3. Fetch current conversation state via HTTP API
4. Resume streaming from there

### 3. Conversation Hierarchy

**Decision**: Explicit subscription required for each conversation. Children not auto-subscribed.

**Rationale**: Auto-subscribing to children has permission implications and adds complexity. The `delegation_started` event includes `child_conversation_id`, allowing clients to subscribe on demand if they want child events.

### 4. Lock Contention

**Decision**: Accept single RwLock for simplicity. Monitor via metrics.

**Rationale**: For < 100 concurrent clients (private network), contention is minimal. The metrics expose `events_routed` and `events_dropped` for monitoring. If contention becomes an issue, can migrate to per-conversation channels.

---

## Testing

### Dependency Injection

```typescript
interface TransportConfig {
  wsUrl: string;
  httpUrl: string;
  createWebSocket?: (url: string) => WebSocket;
}

// In tests:
const mockWs = createMockWebSocket();
const transport = new HttpTransport({
  wsUrl: 'ws://test',
  httpUrl: 'http://test',
  createWebSocket: () => mockWs,
});
```

### Key Test Cases

```typescript
describe('SubscriptionManager', () => {
  test('atomic subscribe/disconnect', async () => {
    // Concurrent operations should not corrupt state
  });

  test('subscription limits enforced', async () => {
    // Over-limit should return error, not crash
  });

  test('dead client cleanup', async () => {
    // Closed channel triggers cleanup
  });

  test('sequence numbers increment atomically', async () => {
    // No duplicate or skipped sequences
  });
});

describe('useConversationEvents', () => {
  test('cancelled subscription during connect', async () => {
    // Unmount before connect completes
  });

  test('sequence gap detection', async () => {
    // Gap triggers warning
  });

  test('proper cleanup on unmount', async () => {
    // No lingering subscriptions
  });
});
```

---

## Migration Plan

### Phase 1: Add Infrastructure
- Add `SubscriptionManager` to gateway
- Add new message types to protocol
- Keep existing broadcast as fallback
- Add cleanup background task

### Phase 2: Server-Side Routing
- Integrate manager into WebSocket handler
- Route events through manager
- Assign sequence numbers

### Phase 3: Frontend Extensions
- Add new methods to `HttpTransport`
- Add hooks
- Add `ConnectionStatus` component

### Phase 4: Migrate Components
- Update `WebChatPanel` to use `useConversationEvents`
- Update dashboard to use `useGlobalEvents`
- Add connection status to layout

### Phase 5: Cleanup
- Remove old broadcast code
- Remove old client-side filtering
- Deprecate old subscribe method

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `gateway/src/websocket/subscriptions.rs` | CREATE | SubscriptionManager |
| `gateway/src/websocket/handler.rs` | MODIFY | Integrate manager, handle sub messages |
| `gateway/src/websocket/messages.rs` | MODIFY | Add subscription message types |
| `apps/ui/src/services/transport/http.ts` | MODIFY | Add subscription methods |
| `apps/ui/src/services/transport/types.ts` | MODIFY | Add new types |
| `apps/ui/src/hooks/useConversationEvents.ts` | CREATE | React hook |
| `apps/ui/src/hooks/useConnectionState.ts` | CREATE | React hook |
| `apps/ui/src/hooks/useGlobalEvents.ts` | CREATE | React hook |
| `apps/ui/src/components/ConnectionStatus.tsx` | CREATE | UI component |
| `apps/ui/src/features/agent/WebChatPanel.tsx` | MODIFY | Use new hooks |

---

## Success Criteria

1. ✅ No race conditions (single lock, atomic operations)
2. ✅ Sequence numbers on all conversation events
3. ✅ Dead client cleanup (background task + on-send-failure)
4. ✅ Connection state visible in UI
5. ✅ Manual reconnect (not page reload)
6. ✅ Browser event handler cleanup (no memory leaks)
7. ✅ Proper error callback tracking
8. ✅ Testable with dependency injection
9. ✅ Metrics for observability
10. ✅ Gap detection with actionable warning
