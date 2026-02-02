# Real-Time Event System v2

## Status: PLANNING (Revised after architectural review)

## Context

This is part of the AgentZero project - a multi-agent orchestration platform. See `memory-bank/agent-orchestration.md` for the execution model and `memory-bank/architecture.md` for overall system design.

**Deployment Context**: This system runs on a private network, not exposed to public internet. Authentication/authorization is deferred but the architecture should support it later.

**Current Problem**: The UI doesn't receive real-time updates for delegation events. Events are broadcast to all clients and filtered client-side, which is fragile and breaks for delegation events that use `parent_conversation_id` instead of `conversation_id`.

---

## Requirements

### Functional
1. Multiple browser tabs can connect simultaneously
2. Each tab can subscribe to specific conversations
3. Same conversation in multiple tabs → all receive events
4. Global events (dashboard stats) broadcast to all connected clients
5. Conversation events only go to subscribers
6. Proper subscription lifecycle with confirmation

### Non-Functional
1. **Reliable**: No silent event loss, handle disconnections gracefully
2. **Ordered**: Events arrive in correct order with sequence numbers
3. **Observable**: Metrics for debugging and monitoring
4. **Testable**: Dependency injection, mockable interfaces
5. **Consistent**: No race conditions in subscription state

---

## Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────┐
│                    GATEWAY SERVER                            │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │            SubscriptionManager                       │   │
│   │                                                      │   │
│   │   ┌─────────────────────────────────────────────┐   │   │
│   │   │  Single RwLock protecting ALL state:        │   │   │
│   │   │                                             │   │   │
│   │   │  clients: Map<ClientId, Client>             │   │   │
│   │   │  subscriptions: Map<ConvId, Set<ClientId>>  │   │   │
│   │   │  client_subs: Map<ClientId, Set<ConvId>>    │   │   │
│   │   │  sequence_nums: Map<ConvId, u64>            │   │   │
│   │   └─────────────────────────────────────────────┘   │   │
│   │                                                      │   │
│   │   - Atomic subscribe/unsubscribe operations          │   │
│   │   - Consistent cleanup on disconnect                 │   │
│   │   - Sequence numbers per conversation                │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              EventRouter                             │   │
│   │                                                      │   │
│   │   - Receives events from EventBus                    │   │
│   │   - Assigns sequence numbers                         │   │
│   │   - Routes to subscribers via SubscriptionManager    │   │
│   │   - Tracks delivery failures for metrics             │   │
│   └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## Server Implementation

### Data Structures (Fixing Race Conditions)

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Client connection state
struct Client {
    id: ClientId,
    sender: mpsc::Sender<ServerMessage>,
    connected_at: std::time::Instant,
    last_ping: std::time::Instant,
    // Limit subscriptions per client
    subscription_count: usize,
}

/// All subscription state protected by a SINGLE lock
/// This eliminates race conditions between dual maps
struct SubscriptionState {
    clients: HashMap<ClientId, Client>,
    subscriptions: HashMap<String, HashSet<ClientId>>,
    client_subscriptions: HashMap<ClientId, HashSet<String>>,
    // Sequence number per conversation for ordering
    sequence_numbers: HashMap<String, u64>,
}

/// Thread-safe subscription manager
pub struct SubscriptionManager {
    state: RwLock<SubscriptionState>,
    // Configuration
    max_subscriptions_per_client: usize,
    max_subscribers_per_conversation: usize,
    // Metrics
    metrics: SubscriptionMetrics,
}

/// Observable metrics
pub struct SubscriptionMetrics {
    pub total_clients: AtomicU64,
    pub total_subscriptions: AtomicU64,
    pub events_routed: AtomicU64,
    pub events_dropped: AtomicU64,
}
```

### Atomic Operations (No TOCTOU)

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
            metrics: SubscriptionMetrics::default(),
        }
    }

    /// Register a new client - atomic operation
    pub async fn connect(&self, client_id: ClientId, sender: mpsc::Sender<ServerMessage>) {
        let mut state = self.state.write().await;
        state.clients.insert(client_id.clone(), Client {
            id: client_id.clone(),
            sender,
            connected_at: std::time::Instant::now(),
            last_ping: std::time::Instant::now(),
            subscription_count: 0,
        });
        state.client_subscriptions.insert(client_id, HashSet::new());
        self.metrics.total_clients.fetch_add(1, Ordering::Relaxed);
    }

    /// Disconnect and cleanup - atomic, no race conditions
    pub async fn disconnect(&self, client_id: &ClientId) {
        let mut state = self.state.write().await;

        // Get all conversations this client was subscribed to
        if let Some(conversations) = state.client_subscriptions.remove(client_id) {
            for conv_id in conversations {
                if let Some(subscribers) = state.subscriptions.get_mut(&conv_id) {
                    subscribers.remove(client_id);
                    // Clean up empty sets immediately (while we hold the lock)
                    if subscribers.is_empty() {
                        state.subscriptions.remove(&conv_id);
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

        // Check client exists (atomic with the insert)
        let client = state.clients.get_mut(client_id)
            .ok_or(SubscribeError::ClientNotFound)?;

        // Check subscription limits
        if client.subscription_count >= self.max_subscriptions_per_client {
            return Err(SubscribeError::TooManySubscriptions {
                limit: self.max_subscriptions_per_client,
            });
        }

        // Check conversation subscriber limit
        let subscribers = state.subscriptions
            .entry(conversation_id.clone())
            .or_insert_with(HashSet::new);

        if subscribers.len() >= self.max_subscribers_per_conversation {
            return Err(SubscribeError::ConversationFull {
                limit: self.max_subscribers_per_conversation,
            });
        }

        // Already subscribed?
        if subscribers.contains(client_id) {
            return Ok(SubscribeResult::AlreadySubscribed);
        }

        // Add subscription (both maps updated atomically)
        subscribers.insert(client_id.clone());
        state.client_subscriptions
            .get_mut(client_id)
            .unwrap() // Safe: we verified client exists above
            .insert(conversation_id.clone());
        client.subscription_count += 1;

        // Initialize sequence number if needed
        state.sequence_numbers
            .entry(conversation_id.clone())
            .or_insert(0);

        self.metrics.total_subscriptions.fetch_add(1, Ordering::Relaxed);

        Ok(SubscribeResult::Subscribed {
            current_sequence: *state.sequence_numbers.get(&conversation_id).unwrap(),
        })
    }

    /// Unsubscribe - atomic
    pub async fn unsubscribe(&self, client_id: &ClientId, conversation_id: &str) {
        let mut state = self.state.write().await;

        // Update both maps atomically
        if let Some(subscribers) = state.subscriptions.get_mut(conversation_id) {
            if subscribers.remove(client_id) {
                self.metrics.total_subscriptions.fetch_sub(1, Ordering::Relaxed);

                if let Some(client) = state.clients.get_mut(client_id) {
                    client.subscription_count = client.subscription_count.saturating_sub(1);
                }
            }
            if subscribers.is_empty() {
                state.subscriptions.remove(conversation_id);
            }
        }

        if let Some(client_subs) = state.client_subscriptions.get_mut(client_id) {
            client_subs.remove(conversation_id);
        }
    }

    /// Route event to subscribers with sequence number
    pub async fn route_event(&self, conversation_id: &str, event: ServerMessage) {
        let state = self.state.read().await;

        if let Some(subscribers) = state.subscriptions.get(conversation_id) {
            for client_id in subscribers {
                if let Some(client) = state.clients.get(client_id) {
                    match client.sender.try_send(event.clone()) {
                        Ok(()) => {
                            self.metrics.events_routed.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            // Channel full - client is slow
                            // Log but don't disconnect (could add backpressure later)
                            self.metrics.events_dropped.fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(
                                client_id = %client_id.0,
                                conversation_id = %conversation_id,
                                "Event dropped: client channel full"
                            );
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            // Client disconnected, will be cleaned up
                            self.metrics.events_dropped.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }

    /// Broadcast global event to all clients
    pub async fn broadcast_global(&self, event: ServerMessage) {
        let state = self.state.read().await;

        for client in state.clients.values() {
            let _ = client.sender.try_send(event.clone());
        }
    }

    /// Get next sequence number for a conversation
    pub async fn next_sequence(&self, conversation_id: &str) -> u64 {
        let mut state = self.state.write().await;
        let seq = state.sequence_numbers
            .entry(conversation_id.to_string())
            .or_insert(0);
        *seq += 1;
        *seq
    }
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
    AlreadySubscribed,
}
```

---

## Protocol Specification (Revised)

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

### Server → Client (with sequence numbers and error codes)

```typescript
// ═══════════════════════════════════════════════════════════
// SUBSCRIPTION RESPONSES (with current sequence for catch-up)
// ═══════════════════════════════════════════════════════════

interface SubscribedMessage {
  type: "subscribed";
  conversation_id: string;
  current_sequence: number;  // Client can detect if they missed events
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
  executions_active: number;
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
  seq: number;  // Sequence number for ordering/gap detection
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
  server_time: number;  // For latency calculation
}

interface ConnectionErrorMessage {
  type: "connection_error";
  error: string;
}
```

---

## Frontend Implementation (Revised)

### Key Changes from v1:
1. **Extend existing HttpTransport** instead of creating new class
2. **Keep the `cancelled` flag pattern** that works in current code
3. **Expose connection state** for UI feedback
4. **Add heartbeat** for stale connection detection
5. **Handle visibility changes** and browser sleep
6. **Dependency injection** for testability

### Transport Extension

```typescript
// ═══════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════

export type ConnectionState =
  | { status: 'disconnected' }
  | { status: 'connecting' }
  | { status: 'connected' }
  | { status: 'reconnecting'; attempt: number; maxAttempts: number }
  | { status: 'failed'; error: string };

export type ConversationCallback = (event: ConversationEvent) => void;
export type GlobalCallback = (event: GlobalEvent) => void;
export type ConnectionStateCallback = (state: ConnectionState) => void;
export type UnsubscribeFn = () => void;

interface SubscriptionState {
  callbacks: Set<ConversationCallback>;
  confirmed: boolean;
  lastSeq: number;  // Track sequence for gap detection
}

interface SubscriptionOptions {
  onEvent: ConversationCallback;
  onError?: (error: SubscriptionErrorMessage) => void;
  onConfirmed?: (seq: number) => void;
}

// ═══════════════════════════════════════════════════════════
// EXTEND EXISTING HTTP TRANSPORT
// ═══════════════════════════════════════════════════════════

// Add these methods to the existing HttpTransport class in http.ts

export class HttpTransport implements Transport {
  // ... existing fields ...

  // NEW: Subscription state
  private conversationSubscriptions: Map<string, SubscriptionState> = new Map();
  private globalCallbacks: Set<GlobalCallback> = new Set();
  private connectionStateCallbacks: Set<ConnectionStateCallback> = new Set();
  private connectionState: ConnectionState = { status: 'disconnected' };

  // NEW: Heartbeat
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private lastPong: number = Date.now();
  private readonly PING_INTERVAL = 15000;  // 15 seconds
  private readonly PONG_TIMEOUT = 30000;   // 30 seconds

  // NEW: Pending subscriptions for reconnection
  private pendingSubscriptions: Set<string> = new Set();

  // ─────────────────────────────────────────────────────────
  // CONNECTION STATE
  // ─────────────────────────────────────────────────────────

  private setConnectionState(state: ConnectionState): void {
    this.connectionState = state;
    for (const callback of this.connectionStateCallbacks) {
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
    // Immediately notify of current state
    callback(this.connectionState);
    return () => this.connectionStateCallbacks.delete(callback);
  }

  // ─────────────────────────────────────────────────────────
  // HEARTBEAT (detect stale connections)
  // ─────────────────────────────────────────────────────────

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.lastPong = Date.now();

    this.pingInterval = setInterval(() => {
      if (Date.now() - this.lastPong > this.PONG_TIMEOUT) {
        console.warn('[Transport] Ping timeout, connection may be stale');
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
  // VISIBILITY & NETWORK HANDLING
  // ─────────────────────────────────────────────────────────

  private setupBrowserEventHandlers(): void {
    // Handle tab visibility changes
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') {
        // Tab became visible - check connection health
        if (this.ws?.readyState !== WebSocket.OPEN) {
          this.reconnectAttempts = 0;  // Reset backoff
          this.connect();
        }
      }
    });

    // Handle network online/offline
    window.addEventListener('online', () => {
      if (this.connectionState.status !== 'connected') {
        this.reconnectAttempts = 0;
        this.connect();
      }
    });

    // Detect wake from sleep (large time gap)
    let lastCheck = Date.now();
    setInterval(() => {
      const now = Date.now();
      if (now - lastCheck > 60000) {  // 60s gap = likely sleep
        console.log('[Transport] Detected wake from sleep');
        if (this.ws?.readyState === WebSocket.OPEN) {
          // Connection might be stale, force reconnect
          this.ws.close(4001, 'Wake from sleep');
        }
      }
      lastCheck = now;
    }, 5000);
  }

  // ─────────────────────────────────────────────────────────
  // SUBSCRIPTION API
  // ─────────────────────────────────────────────────────────

  /**
   * Subscribe to conversation events.
   * Returns unsubscribe function.
   */
  public subscribeConversation(
    conversationId: string,
    options: SubscriptionOptions
  ): UnsubscribeFn {
    let state = this.conversationSubscriptions.get(conversationId);

    if (!state) {
      state = {
        callbacks: new Set(),
        confirmed: false,
        lastSeq: 0,
      };
      this.conversationSubscriptions.set(conversationId, state);
      this.pendingSubscriptions.add(conversationId);
      this.sendSubscribe(conversationId);
    }

    // Wrap callback to include error handling option
    const wrappedCallback: ConversationCallback = (event) => {
      // Check for sequence gaps
      if (event.seq && state && event.seq > state.lastSeq + 1) {
        console.warn(
          `[Transport] Sequence gap detected: expected ${state.lastSeq + 1}, got ${event.seq}`
        );
        // Could trigger a state refresh here
      }
      if (event.seq && state) {
        state.lastSeq = event.seq;
      }
      options.onEvent(event);
    };

    state.callbacks.add(wrappedCallback);

    // Store error callback for this subscription
    const errorCallbacks = this.subscriptionErrorCallbacks.get(conversationId) || new Set();
    if (options.onError) {
      errorCallbacks.add(options.onError);
      this.subscriptionErrorCallbacks.set(conversationId, errorCallbacks);
    }

    return () => {
      const state = this.conversationSubscriptions.get(conversationId);
      if (!state) return;

      state.callbacks.delete(wrappedCallback);

      if (options.onError) {
        this.subscriptionErrorCallbacks.get(conversationId)?.delete(options.onError);
      }

      if (state.callbacks.size === 0) {
        this.conversationSubscriptions.delete(conversationId);
        this.pendingSubscriptions.delete(conversationId);
        this.sendUnsubscribe(conversationId);
      }
    };
  }

  private subscriptionErrorCallbacks: Map<string, Set<(error: SubscriptionErrorMessage) => void>> = new Map();

  /**
   * Subscribe to global events (stats, notifications).
   */
  public onGlobalEvent(callback: GlobalCallback): UnsubscribeFn {
    this.globalCallbacks.add(callback);
    return () => this.globalCallbacks.delete(callback);
  }

  // ─────────────────────────────────────────────────────────
  // MESSAGE HANDLING (revised)
  // ─────────────────────────────────────────────────────────

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
        console.error(`[Transport] Subscription error for ${message.conversation_id}: ${message.code}`);
        // Notify error callbacks
        const errorCallbacks = this.subscriptionErrorCallbacks.get(message.conversation_id);
        if (errorCallbacks) {
          for (const cb of errorCallbacks) {
            try { cb(message); } catch (e) { console.error(e); }
          }
        }
        // Clean up failed subscription
        this.conversationSubscriptions.delete(message.conversation_id);
        this.pendingSubscriptions.delete(message.conversation_id);
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

  private handleGlobalEvent(message: ServerMessage): boolean {
    if (message.type === 'stats_update' || message.type === 'session_notification') {
      for (const callback of this.globalCallbacks) {
        try {
          callback(message);
        } catch (e) {
          console.error('[Transport] Global callback error:', e);
        }
      }
      return true;
    }
    return false;
  }

  private handleConversationEvent(message: ConversationEvent): boolean {
    const state = this.conversationSubscriptions.get(message.conversation_id);
    if (state) {
      for (const callback of state.callbacks) {
        try {
          callback(message);
        } catch (e) {
          console.error('[Transport] Conversation callback error:', e);
        }
      }
      return true;
    }
    return false;
  }

  // ─────────────────────────────────────────────────────────
  // RECONNECTION (with resubscription)
  // ─────────────────────────────────────────────────────────

  private resubscribeAll(): void {
    for (const conversationId of this.pendingSubscriptions) {
      this.sendSubscribe(conversationId);
      // Mark as unconfirmed until server responds
      const state = this.conversationSubscriptions.get(conversationId);
      if (state) {
        state.confirmed = false;
      }
    }
  }

  // Modify existing connect() to call resubscribeAll on reconnect
  // and use setConnectionState for status updates
}
```

### React Hooks (Revised)

```typescript
// ═══════════════════════════════════════════════════════════
// hooks/useConversationEvents.ts
// ═══════════════════════════════════════════════════════════

import { useEffect, useRef, useCallback } from 'react';
import { getTransport } from '@/services/transport';
import type { ConversationEvent, SubscriptionErrorMessage } from '@/services/transport/types';

interface UseConversationEventsOptions {
  onError?: (error: SubscriptionErrorMessage) => void;
}

/**
 * Subscribe to events for a conversation.
 * Properly handles async connection and cleanup.
 */
export function useConversationEvents(
  conversationId: string | null,
  onEvent: (event: ConversationEvent) => void,
  options: UseConversationEventsOptions = {}
) {
  // Use ref to avoid stale closure issues
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

    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const subscribe = async () => {
      try {
        const transport = await getTransport();

        // Check if we were cancelled while awaiting
        if (cancelled) return;

        unsubscribe = transport.subscribeConversation(conversationId, {
          onEvent: (event) => onEventRef.current(event),
          onError: (error) => onErrorRef.current?.(error),
        });
      } catch (error) {
        console.error('[useConversationEvents] Failed to subscribe:', error);
      }
    };

    subscribe();

    return () => {
      cancelled = true;
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, [conversationId]);
}

// ═══════════════════════════════════════════════════════════
// hooks/useConnectionState.ts
// ═══════════════════════════════════════════════════════════

import { useState, useEffect } from 'react';
import { getTransport } from '@/services/transport';
import type { ConnectionState } from '@/services/transport/types';

/**
 * Get real-time connection state for UI feedback.
 */
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

/**
 * Subscribe to global events (dashboard stats, session notifications).
 */
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

      unsubscribe = transport.onGlobalEvent((event) => {
        onEventRef.current(event);
      });
    };

    setup();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, []);
}
```

### Connection Status UI Component

```typescript
// ═══════════════════════════════════════════════════════════
// components/ConnectionStatus.tsx
// ═══════════════════════════════════════════════════════════

import { useConnectionState } from '@/hooks/useConnectionState';
import { Wifi, WifiOff, Loader2, AlertCircle } from 'lucide-react';

export function ConnectionStatus() {
  const state = useConnectionState();

  switch (state.status) {
    case 'connected':
      return null;  // Don't show anything when connected

    case 'connecting':
      return (
        <div className="flex items-center gap-2 text-yellow-600 text-sm">
          <Loader2 className="w-4 h-4 animate-spin" />
          Connecting...
        </div>
      );

    case 'reconnecting':
      return (
        <div className="flex items-center gap-2 text-yellow-600 text-sm">
          <Loader2 className="w-4 h-4 animate-spin" />
          Reconnecting ({state.attempt}/{state.maxAttempts})...
        </div>
      );

    case 'disconnected':
      return (
        <div className="flex items-center gap-2 text-gray-500 text-sm">
          <WifiOff className="w-4 h-4" />
          Disconnected
        </div>
      );

    case 'failed':
      return (
        <div className="flex items-center gap-2 text-red-600 text-sm">
          <AlertCircle className="w-4 h-4" />
          Connection failed
          <button
            onClick={() => window.location.reload()}
            className="underline ml-2"
          >
            Reload
          </button>
        </div>
      );
  }
}
```

---

## Testing Strategy (Revised)

### Dependency Injection for Testability

```typescript
// Transport factory that accepts WebSocket implementation
interface TransportConfig {
  wsUrl: string;
  httpUrl: string;
  // Inject for testing
  createWebSocket?: (url: string) => WebSocket;
}

class HttpTransport {
  constructor(private config: TransportConfig) {}

  async connect(): Promise<void> {
    const createWs = this.config.createWebSocket || ((url) => new WebSocket(url));
    this.ws = createWs(this.config.wsUrl);
    // ...
  }
}

// Test utility
export function createMockTransport() {
  const mockWs = {
    send: vi.fn(),
    close: vi.fn(),
    readyState: WebSocket.OPEN,
    onmessage: null as ((event: MessageEvent) => void) | null,
    onopen: null as (() => void) | null,
    onclose: null as (() => void) | null,
    onerror: null as ((error: Event) => void) | null,
  };

  const transport = new HttpTransport({
    wsUrl: 'ws://test',
    httpUrl: 'http://test',
    createWebSocket: () => mockWs as unknown as WebSocket,
  });

  return {
    transport,
    mockWs,
    // Simulate server sending a message
    receiveMessage: (data: ServerMessage) => {
      mockWs.onmessage?.({ data: JSON.stringify(data) } as MessageEvent);
    },
    // Simulate connection open
    connect: () => {
      mockWs.onopen?.();
    },
  };
}
```

### Test Cases

```typescript
describe('SubscriptionManager', () => {
  it('should handle concurrent subscribe/disconnect atomically', async () => {
    const manager = new SubscriptionManager();
    const clientId = new ClientId('test');
    const sender = createMockSender();

    await manager.connect(clientId, sender);

    // Concurrent operations
    await Promise.all([
      manager.subscribe(clientId, 'conv-1'),
      manager.subscribe(clientId, 'conv-2'),
      manager.disconnect(clientId),
    ]);

    // State should be consistent (client gone, no orphaned subscriptions)
    const state = await manager.getState();
    expect(state.clients.has(clientId)).toBe(false);
    expect(state.subscriptions.get('conv-1')?.has(clientId)).toBeFalsy();
    expect(state.subscriptions.get('conv-2')?.has(clientId)).toBeFalsy();
  });

  it('should enforce subscription limits', async () => {
    const manager = new SubscriptionManager();
    manager.maxSubscriptionsPerClient = 2;

    const clientId = new ClientId('test');
    await manager.connect(clientId, createMockSender());

    await manager.subscribe(clientId, 'conv-1');
    await manager.subscribe(clientId, 'conv-2');

    const result = await manager.subscribe(clientId, 'conv-3');
    expect(result).toEqual({
      error: 'LIMIT_EXCEEDED',
    });
  });
});

describe('useConversationEvents', () => {
  it('should not subscribe if unmounted during connection', async () => {
    const { transport, mockWs, connect } = createMockTransport();

    const { unmount } = renderHook(() =>
      useConversationEvents('conv-1', vi.fn())
    );

    // Unmount before connection completes
    unmount();

    // Now complete connection
    connect();

    // Should not have sent subscribe message
    expect(mockWs.send).not.toHaveBeenCalledWith(
      expect.stringContaining('subscribe')
    );
  });

  it('should detect sequence gaps', async () => {
    const { transport, receiveMessage, connect } = createMockTransport();
    const onEvent = vi.fn();
    const consoleSpy = vi.spyOn(console, 'warn');

    renderHook(() => useConversationEvents('conv-1', onEvent));
    connect();

    // Receive events with gap
    receiveMessage({ type: 'token', conversation_id: 'conv-1', seq: 1, delta: 'a' });
    receiveMessage({ type: 'token', conversation_id: 'conv-1', seq: 5, delta: 'b' }); // Gap!

    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining('Sequence gap detected')
    );
  });
});
```

---

## Migration Strategy

### Phase 1: Add Infrastructure (Non-Breaking)
1. Add `SubscriptionManager` to gateway
2. Add new message types to protocol
3. Keep existing broadcast behavior as fallback

### Phase 2: Wire Up Server-Side Routing
1. Integrate `SubscriptionManager` into WebSocket handler
2. Route events through manager
3. Add sequence numbers to events

### Phase 3: Extend Frontend Transport
1. Add new methods to `HttpTransport` (don't create new class)
2. Add hooks
3. Add `ConnectionStatus` component

### Phase 4: Migrate Components
1. Update `WebChatPanel` to use `useConversationEvents`
2. Update dashboard to use `useGlobalEvents`
3. Add connection status to header

### Phase 5: Cleanup
1. Remove old broadcast code
2. Remove old client-side filtering
3. Update tests

---

## Metrics & Observability

```rust
// Expose via /metrics endpoint or logs
pub struct Metrics {
    // Connection metrics
    pub clients_connected: Gauge,
    pub connections_total: Counter,
    pub disconnections_total: Counter,

    // Subscription metrics
    pub subscriptions_active: Gauge,
    pub subscriptions_total: Counter,
    pub unsubscriptions_total: Counter,
    pub subscription_errors_total: Counter,

    // Event metrics
    pub events_routed_total: Counter,
    pub events_dropped_total: Counter,
    pub events_by_type: CounterVec,  // Labeled by event type

    // Latency
    pub event_routing_latency: Histogram,
}
```

---

## What's Deferred (Private Network Context)

Since this runs on a private network:

1. **Authentication**: Deferred, but architecture supports adding token validation in `connect()`
2. **Authorization**: Deferred, but `subscribe()` has a hook point for permission checks
3. **Rate limiting**: Basic subscription limits implemented, advanced rate limiting deferred
4. **TLS/WSS**: Assumed handled by network layer

These should be added before any public exposure.

---

## Success Criteria

1. ✅ No race conditions in subscription state
2. ✅ Events include sequence numbers
3. ✅ Connection state visible in UI
4. ✅ Proper cleanup on disconnect
5. ✅ Handles browser sleep/visibility changes
6. ✅ Testable with dependency injection
7. ✅ Metrics for observability
8. ✅ Subscription limits enforced
9. ✅ Extends existing code instead of duplicating
