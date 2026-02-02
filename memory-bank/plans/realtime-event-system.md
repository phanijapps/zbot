# Real-Time Event System

## Status: PLANNING

## Overview

A proper pub/sub event delivery system for AgentZero. Clients subscribe to specific conversations and receive only relevant events. Global events (dashboard stats) broadcast to all connected clients.

## Requirements

### Functional
1. Multiple browser tabs can connect simultaneously
2. Each tab can subscribe to different conversations
3. Same conversation open in 2 tabs → both receive events
4. Global dashboard stats update for all connected clients
5. Chat panel events only go to subscribers of that conversation
6. Subscription lifecycle: subscribe on panel open, unsubscribe on close

### Non-Functional
1. **Secure**: No event leakage between unauthorized clients
2. **Reliable**: Handle disconnections, reconnections gracefully
3. **Ordered**: Events arrive in correct order
4. **Efficient**: No wasted bandwidth sending irrelevant events
5. **Scalable**: Handle many clients and conversations

---

## Architecture

### Event Tiers

```
┌─────────────────────────────────────────────────────────────┐
│                      TIER 1: GLOBAL                          │
│                   (all connected clients)                    │
│                                                              │
│   • stats_update      - Dashboard counters changed           │
│   • session_created   - New session started                  │
│   • session_completed - Session finished                     │
│   • session_crashed   - Session errored                      │
│                                                              │
│   These are lightweight notifications.                       │
│   UI typically does an API refresh after receiving.          │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                 TIER 2: CONVERSATION-SCOPED                  │
│              (only subscribers of that conversation)         │
│                                                              │
│   • agent_started     - Agent began processing               │
│   • token             - Streaming token                      │
│   • tool_call         - Tool invocation                      │
│   • tool_result       - Tool returned                        │
│   • delegation_started   - Subagent spawned                  │
│   • delegation_completed - Subagent finished                 │
│   • agent_completed   - Agent finished turn                  │
│   • error             - Error occurred                       │
│                                                              │
│   These are the real-time streaming events.                  │
│   UI updates immediately on receipt.                         │
└─────────────────────────────────────────────────────────────┘
```

### Server Components

```
┌─────────────────────────────────────────────────────────────┐
│                    WebSocket Handler                         │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              Connection Manager                      │   │
│   │                                                      │   │
│   │   clients: Map<ClientId, ClientConnection>           │   │
│   │                                                      │   │
│   │   - Tracks all connected WebSocket clients           │   │
│   │   - Handles connect/disconnect lifecycle             │   │
│   │   - Manages per-client send channel                  │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              Subscription Manager                    │   │
│   │                                                      │   │
│   │   subscriptions: Map<ConversationId, Set<ClientId>>  │   │
│   │   client_subs: Map<ClientId, Set<ConversationId>>    │   │
│   │                                                      │   │
│   │   - Tracks who is subscribed to what                 │   │
│   │   - Reverse index for efficient cleanup              │   │
│   │   - Thread-safe (DashMap or RwLock)                  │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │                 Event Router                         │   │
│   │                                                      │   │
│   │   - Receives events from EventBus                    │   │
│   │   - Classifies: global vs conversation-scoped        │   │
│   │   - Routes to appropriate clients                    │   │
│   │   - Handles delegation event remapping               │   │
│   └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## Protocol Specification

### Client → Server Messages

```typescript
// Subscribe to a conversation's events
interface SubscribeMessage {
  type: "subscribe";
  conversation_id: string;
}

// Unsubscribe from a conversation
interface UnsubscribeMessage {
  type: "unsubscribe";
  conversation_id: string;
}

// Execute an agent (existing)
interface ExecuteMessage {
  type: "execute";
  agent_id: string;
  conversation_id: string;
  message: string;
  session_id?: string;
}

// Stop execution (existing)
interface StopMessage {
  type: "stop";
  conversation_id: string;
}

// End session (existing)
interface EndSessionMessage {
  type: "end_session";
  session_id: string;
}

// Keep-alive
interface PingMessage {
  type: "ping";
}
```

### Server → Client Messages

```typescript
// ═══════════════════════════════════════════════════════════
// SUBSCRIPTION MANAGEMENT
// ═══════════════════════════════════════════════════════════

interface SubscribedMessage {
  type: "subscribed";
  conversation_id: string;
}

interface UnsubscribedMessage {
  type: "unsubscribed";
  conversation_id: string;
}

interface SubscriptionErrorMessage {
  type: "subscription_error";
  conversation_id: string;
  error: string;
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
  timestamp: string;
}

// ═══════════════════════════════════════════════════════════
// CONVERSATION-SCOPED EVENTS (require subscription)
// ═══════════════════════════════════════════════════════════

// All conversation events include conversation_id for routing
interface ConversationEvent {
  conversation_id: string;
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
  arguments: Record<string, unknown>;
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
  recoverable: boolean;
}

// ═══════════════════════════════════════════════════════════
// CONNECTION MANAGEMENT
// ═══════════════════════════════════════════════════════════

interface PongMessage {
  type: "pong";
}

interface ConnectionErrorMessage {
  type: "connection_error";
  error: string;
}
```

---

## Event Routing Logic

### Mapping Internal Events to Client Messages

```rust
impl GatewayEvent {
    /// Determine how this event should be routed
    fn routing(&self) -> EventRouting {
        match self {
            // Global broadcasts
            Self::StatsUpdate { .. } => EventRouting::Global,
            Self::SessionCreated { .. } => EventRouting::Global,
            Self::SessionCompleted { .. } => EventRouting::Global,
            Self::SessionCrashed { .. } => EventRouting::Global,

            // Conversation-scoped: use conversation_id directly
            Self::AgentStarted { conversation_id, .. } =>
                EventRouting::Conversation(conversation_id.clone()),
            Self::Token { conversation_id, .. } =>
                EventRouting::Conversation(conversation_id.clone()),
            Self::ToolCall { conversation_id, .. } =>
                EventRouting::Conversation(conversation_id.clone()),
            Self::ToolResult { conversation_id, .. } =>
                EventRouting::Conversation(conversation_id.clone()),
            Self::AgentCompleted { conversation_id, .. } =>
                EventRouting::Conversation(conversation_id.clone()),
            Self::Error { conversation_id, .. } =>
                EventRouting::Conversation(conversation_id.clone()),

            // Delegation events: route to PARENT conversation
            // The parent's UI should see these, not the child's
            Self::DelegationStarted { parent_conversation_id, .. } =>
                EventRouting::Conversation(parent_conversation_id.clone()),
            Self::DelegationCompleted { parent_conversation_id, .. } =>
                EventRouting::Conversation(parent_conversation_id.clone()),

            // Internal events: don't route to clients
            Self::SessionContinuationReady { .. } => EventRouting::Internal,
        }
    }
}

enum EventRouting {
    /// Send to all connected clients
    Global,
    /// Send only to clients subscribed to this conversation
    Conversation(String),
    /// Don't send to any client (internal system event)
    Internal,
}
```

### Delegation Event Transformation

When a delegation event is sent to the parent conversation, we need to include `conversation_id` (the parent) for routing consistency:

```rust
fn delegation_to_client_message(event: GatewayEvent) -> ServerMessage {
    match event {
        GatewayEvent::DelegationStarted {
            parent_conversation_id,
            child_agent_id,
            child_conversation_id,
            task,
            ..
        } => ServerMessage::DelegationStarted {
            // Use parent_conversation_id as the routing conversation_id
            conversation_id: parent_conversation_id,
            child_agent_id,
            child_conversation_id,
            task,
        },

        GatewayEvent::DelegationCompleted {
            parent_conversation_id,
            child_agent_id,
            child_conversation_id,
            result,
            ..
        } => ServerMessage::DelegationCompleted {
            conversation_id: parent_conversation_id,
            child_agent_id,
            child_conversation_id,
            result,
        },

        _ => unreachable!(),
    }
}
```

---

## Server Implementation

### Data Structures

```rust
use dashmap::DashMap;
use std::collections::HashSet;
use tokio::sync::mpsc;

/// Unique identifier for a WebSocket client connection
#[derive(Clone, Hash, Eq, PartialEq)]
struct ClientId(String);

/// A connected WebSocket client
struct ClientConnection {
    id: ClientId,
    sender: mpsc::Sender<ServerMessage>,
    connected_at: std::time::Instant,
    last_activity: std::time::Instant,
}

/// Manages all WebSocket state
struct WebSocketManager {
    /// All connected clients
    clients: DashMap<ClientId, ClientConnection>,

    /// Conversation → subscribers
    subscriptions: DashMap<String, HashSet<ClientId>>,

    /// Client → subscribed conversations (for cleanup)
    client_subscriptions: DashMap<ClientId, HashSet<String>>,
}
```

### Core Operations

```rust
impl WebSocketManager {
    /// Register a new client connection
    fn connect(&self, client_id: ClientId, sender: mpsc::Sender<ServerMessage>) {
        self.clients.insert(client_id.clone(), ClientConnection {
            id: client_id.clone(),
            sender,
            connected_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
        });
        self.client_subscriptions.insert(client_id, HashSet::new());
    }

    /// Remove a client and clean up all subscriptions
    fn disconnect(&self, client_id: &ClientId) {
        // Remove from all conversation subscriptions
        if let Some((_, conversations)) = self.client_subscriptions.remove(client_id) {
            for conv_id in conversations {
                if let Some(mut subscribers) = self.subscriptions.get_mut(&conv_id) {
                    subscribers.remove(client_id);
                    // Clean up empty subscription sets
                    if subscribers.is_empty() {
                        drop(subscribers);
                        self.subscriptions.remove(&conv_id);
                    }
                }
            }
        }
        self.clients.remove(client_id);
    }

    /// Subscribe a client to a conversation
    fn subscribe(&self, client_id: &ClientId, conversation_id: String) -> Result<(), String> {
        // Verify client exists
        if !self.clients.contains_key(client_id) {
            return Err("Client not connected".to_string());
        }

        // Add to conversation subscribers
        self.subscriptions
            .entry(conversation_id.clone())
            .or_insert_with(HashSet::new)
            .insert(client_id.clone());

        // Add to client's subscription list
        if let Some(mut client_subs) = self.client_subscriptions.get_mut(client_id) {
            client_subs.insert(conversation_id);
        }

        Ok(())
    }

    /// Unsubscribe a client from a conversation
    fn unsubscribe(&self, client_id: &ClientId, conversation_id: &str) {
        if let Some(mut subscribers) = self.subscriptions.get_mut(conversation_id) {
            subscribers.remove(client_id);
        }
        if let Some(mut client_subs) = self.client_subscriptions.get_mut(client_id) {
            client_subs.remove(conversation_id);
        }
    }

    /// Send to all connected clients
    fn broadcast_global(&self, message: ServerMessage) {
        for client in self.clients.iter() {
            // Use try_send to avoid blocking if client is slow
            let _ = client.sender.try_send(message.clone());
        }
    }

    /// Send to clients subscribed to a specific conversation
    fn send_to_conversation(&self, conversation_id: &str, message: ServerMessage) {
        if let Some(subscribers) = self.subscriptions.get(conversation_id) {
            for client_id in subscribers.iter() {
                if let Some(client) = self.clients.get(client_id) {
                    let _ = client.sender.try_send(message.clone());
                }
            }
        }
    }

    /// Route an event to appropriate clients
    fn route_event(&self, event: GatewayEvent) {
        let message: ServerMessage = event.clone().into();

        match event.routing() {
            EventRouting::Global => {
                self.broadcast_global(message);
            }
            EventRouting::Conversation(conv_id) => {
                self.send_to_conversation(&conv_id, message);
            }
            EventRouting::Internal => {
                // Don't send to clients
            }
        }
    }
}
```

### Message Handler

```rust
async fn handle_client_message(
    manager: &WebSocketManager,
    client_id: &ClientId,
    message: ClientMessage,
) -> Option<ServerMessage> {
    match message {
        ClientMessage::Subscribe { conversation_id } => {
            match manager.subscribe(client_id, conversation_id.clone()) {
                Ok(()) => Some(ServerMessage::Subscribed { conversation_id }),
                Err(e) => Some(ServerMessage::SubscriptionError {
                    conversation_id,
                    error: e
                }),
            }
        }

        ClientMessage::Unsubscribe { conversation_id } => {
            manager.unsubscribe(client_id, &conversation_id);
            Some(ServerMessage::Unsubscribed { conversation_id })
        }

        ClientMessage::Ping => Some(ServerMessage::Pong),

        // Execute, Stop, EndSession handled by existing logic
        _ => None,
    }
}
```

---

## Frontend Implementation

### Transport Layer

```typescript
// ═══════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════

type ConversationCallback = (event: ConversationEvent) => void;
type GlobalCallback = (event: GlobalEvent) => void;
type UnsubscribeFn = () => void;

interface SubscriptionState {
  callbacks: Set<ConversationCallback>;
  confirmed: boolean;
}

// ═══════════════════════════════════════════════════════════
// EVENT TRANSPORT
// ═══════════════════════════════════════════════════════════

class EventTransport {
  private ws: WebSocket | null = null;
  private wsUrl: string;

  // Subscription tracking
  private subscriptions: Map<string, SubscriptionState> = new Map();
  private globalCallbacks: Set<GlobalCallback> = new Set();

  // Reconnection state
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectDelay = 1000;

  // Connection state
  private isConnecting = false;
  private connectionPromise: Promise<void> | null = null;

  constructor(wsUrl: string) {
    this.wsUrl = wsUrl;
  }

  // ─────────────────────────────────────────────────────────
  // CONNECTION MANAGEMENT
  // ─────────────────────────────────────────────────────────

  async connect(): Promise<void> {
    if (this.ws?.readyState === WebSocket.OPEN) {
      return;
    }

    if (this.isConnecting && this.connectionPromise) {
      return this.connectionPromise;
    }

    this.isConnecting = true;
    this.connectionPromise = new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.wsUrl);

        this.ws.onopen = () => {
          console.log("[EventTransport] Connected");
          this.isConnecting = false;
          this.reconnectAttempts = 0;

          // Resubscribe to all conversations we were tracking
          this.resubscribeAll();

          resolve();
        };

        this.ws.onmessage = (event) => {
          this.handleMessage(JSON.parse(event.data));
        };

        this.ws.onclose = (event) => {
          console.log("[EventTransport] Disconnected", event.code);
          this.isConnecting = false;

          // Mark all subscriptions as unconfirmed
          for (const state of this.subscriptions.values()) {
            state.confirmed = false;
          }

          // Attempt reconnection
          this.attemptReconnect();
        };

        this.ws.onerror = (error) => {
          console.error("[EventTransport] Error", error);
          this.isConnecting = false;
          reject(error);
        };

      } catch (error) {
        this.isConnecting = false;
        reject(error);
      }
    });

    return this.connectionPromise;
  }

  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error("[EventTransport] Max reconnect attempts reached");
      return;
    }

    this.reconnectAttempts++;
    const delay = Math.min(
      this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1),
      30000
    );

    console.log(`[EventTransport] Reconnecting in ${delay}ms...`);
    setTimeout(() => this.connect(), delay);
  }

  private resubscribeAll(): void {
    for (const conversationId of this.subscriptions.keys()) {
      this.sendSubscribe(conversationId);
    }
  }

  // ─────────────────────────────────────────────────────────
  // SUBSCRIPTION API
  // ─────────────────────────────────────────────────────────

  /**
   * Subscribe to conversation events.
   * Call this when the chat panel opens.
   */
  subscribe(conversationId: string, callback: ConversationCallback): UnsubscribeFn {
    let state = this.subscriptions.get(conversationId);

    if (!state) {
      // First subscriber to this conversation
      state = { callbacks: new Set(), confirmed: false };
      this.subscriptions.set(conversationId, state);
      this.sendSubscribe(conversationId);
    }

    state.callbacks.add(callback);

    // Return unsubscribe function
    return () => {
      const state = this.subscriptions.get(conversationId);
      if (!state) return;

      state.callbacks.delete(callback);

      if (state.callbacks.size === 0) {
        // No more subscribers, unsubscribe from server
        this.subscriptions.delete(conversationId);
        this.sendUnsubscribe(conversationId);
      }
    };
  }

  /**
   * Subscribe to global events (dashboard stats, session notifications).
   * These are received automatically when connected.
   */
  onGlobal(callback: GlobalCallback): UnsubscribeFn {
    this.globalCallbacks.add(callback);
    return () => this.globalCallbacks.delete(callback);
  }

  // ─────────────────────────────────────────────────────────
  // MESSAGE SENDING
  // ─────────────────────────────────────────────────────────

  private sendSubscribe(conversationId: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: "subscribe",
        conversation_id: conversationId,
      }));
    }
  }

  private sendUnsubscribe(conversationId: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: "unsubscribe",
        conversation_id: conversationId,
      }));
    }
  }

  // ─────────────────────────────────────────────────────────
  // MESSAGE HANDLING
  // ─────────────────────────────────────────────────────────

  private handleMessage(message: ServerMessage): void {
    switch (message.type) {
      // Subscription confirmations
      case "subscribed": {
        const state = this.subscriptions.get(message.conversation_id);
        if (state) {
          state.confirmed = true;
          console.log(`[EventTransport] Subscribed to ${message.conversation_id}`);
        }
        break;
      }

      case "unsubscribed": {
        console.log(`[EventTransport] Unsubscribed from ${message.conversation_id}`);
        break;
      }

      case "subscription_error": {
        console.error(`[EventTransport] Subscription error for ${message.conversation_id}: ${message.error}`);
        this.subscriptions.delete(message.conversation_id);
        break;
      }

      // Global events
      case "stats_update":
      case "session_notification": {
        for (const callback of this.globalCallbacks) {
          try {
            callback(message);
          } catch (e) {
            console.error("[EventTransport] Global callback error:", e);
          }
        }
        break;
      }

      // Conversation events
      case "agent_started":
      case "token":
      case "tool_call":
      case "tool_result":
      case "delegation_started":
      case "delegation_completed":
      case "agent_completed":
      case "error": {
        const state = this.subscriptions.get(message.conversation_id);
        if (state) {
          for (const callback of state.callbacks) {
            try {
              callback(message);
            } catch (e) {
              console.error("[EventTransport] Callback error:", e);
            }
          }
        }
        break;
      }

      case "pong": {
        // Keep-alive response, ignore
        break;
      }
    }
  }
}

// ═══════════════════════════════════════════════════════════
// SINGLETON INSTANCE
// ═══════════════════════════════════════════════════════════

let transport: EventTransport | null = null;

export function getEventTransport(): EventTransport {
  if (!transport) {
    transport = new EventTransport("ws://localhost:18790");
  }
  return transport;
}
```

### React Integration

```typescript
// ═══════════════════════════════════════════════════════════
// HOOK: useConversationEvents
// ═══════════════════════════════════════════════════════════

function useConversationEvents(
  conversationId: string | null,
  onEvent: (event: ConversationEvent) => void
) {
  const transport = getEventTransport();
  const onEventRef = useRef(onEvent);

  // Keep callback ref updated
  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    if (!conversationId) return;

    // Subscribe when conversation is set
    const unsubscribe = transport.subscribe(conversationId, (event) => {
      onEventRef.current(event);
    });

    // Unsubscribe when conversation changes or component unmounts
    return unsubscribe;
  }, [conversationId, transport]);
}

// ═══════════════════════════════════════════════════════════
// HOOK: useGlobalEvents
// ═══════════════════════════════════════════════════════════

function useGlobalEvents(onEvent: (event: GlobalEvent) => void) {
  const transport = getEventTransport();
  const onEventRef = useRef(onEvent);

  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    const unsubscribe = transport.onGlobal((event) => {
      onEventRef.current(event);
    });

    return unsubscribe;
  }, [transport]);
}

// ═══════════════════════════════════════════════════════════
// USAGE IN WEBCHATPANEL
// ═══════════════════════════════════════════════════════════

function WebChatPanel({ conversationId }: { conversationId: string }) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);

  useConversationEvents(conversationId, (event) => {
    switch (event.type) {
      case "agent_started":
        setIsProcessing(true);
        break;

      case "token":
        setMessages(prev => {
          // Append token to last assistant message or create new
          // ... existing token handling logic
        });
        break;

      case "delegation_started":
        setMessages(prev => [...prev, {
          role: "delegation",
          content: `Delegating to ${event.child_agent_id}...`,
          status: "started",
        }]);
        break;

      case "delegation_completed":
        setMessages(prev => {
          // Update delegation message to show completion
          // ... existing delegation completion logic
        });
        break;

      case "agent_completed":
        setIsProcessing(false);
        break;
    }
  });

  // ... rest of component
}
```

---

## Migration Strategy

### Phase 1: Add New Infrastructure (Non-Breaking)

1. Add `WebSocketManager` struct alongside existing handler
2. Add `Subscribe`/`Unsubscribe` message types to protocol
3. Add subscription tracking data structures
4. Add `EventRouting` enum and routing logic

**Test**: Existing functionality still works

### Phase 2: Implement Server-Side Routing

1. Integrate `WebSocketManager` into WebSocket handler
2. Handle `Subscribe`/`Unsubscribe` messages
3. Route events through manager instead of broadcast
4. Add global events (stats_update, session_notification)

**Test**:
- Subscribe/unsubscribe works
- Events only go to subscribers
- Global events go to all

### Phase 3: Update Frontend

1. Create new `EventTransport` class
2. Add `useConversationEvents` and `useGlobalEvents` hooks
3. Update `WebChatPanel` to use new subscription API
4. Update dashboard to use global events

**Test**:
- Chat panel receives events when open
- No events when panel closed
- Multiple tabs work correctly
- Reconnection works

### Phase 4: Cleanup

1. Remove old event forwarding code
2. Remove client-side conversation_id filtering
3. Update documentation
4. Performance testing

---

## Testing Plan

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_subscribe_unsubscribe() {
        let manager = WebSocketManager::new();
        let (tx, _rx) = mpsc::channel(16);
        let client_id = ClientId::new();

        manager.connect(client_id.clone(), tx);

        // Subscribe
        manager.subscribe(&client_id, "conv-1".to_string()).unwrap();
        assert!(manager.subscriptions.get("conv-1").unwrap().contains(&client_id));

        // Unsubscribe
        manager.unsubscribe(&client_id, "conv-1");
        assert!(!manager.subscriptions.contains_key("conv-1"));
    }

    #[test]
    fn test_disconnect_cleanup() {
        let manager = WebSocketManager::new();
        let (tx, _rx) = mpsc::channel(16);
        let client_id = ClientId::new();

        manager.connect(client_id.clone(), tx);
        manager.subscribe(&client_id, "conv-1".to_string()).unwrap();
        manager.subscribe(&client_id, "conv-2".to_string()).unwrap();

        manager.disconnect(&client_id);

        // All subscriptions should be cleaned up
        assert!(!manager.subscriptions.contains_key("conv-1"));
        assert!(!manager.subscriptions.contains_key("conv-2"));
        assert!(!manager.client_subscriptions.contains_key(&client_id));
    }

    #[tokio::test]
    async fn test_event_routing() {
        let manager = WebSocketManager::new();

        let (tx1, mut rx1) = mpsc::channel(16);
        let (tx2, mut rx2) = mpsc::channel(16);

        let client1 = ClientId::new();
        let client2 = ClientId::new();

        manager.connect(client1.clone(), tx1);
        manager.connect(client2.clone(), tx2);

        manager.subscribe(&client1, "conv-a".to_string()).unwrap();
        manager.subscribe(&client2, "conv-b".to_string()).unwrap();

        // Send to conv-a
        manager.send_to_conversation("conv-a", ServerMessage::Token {
            conversation_id: "conv-a".to_string(),
            delta: "hello".to_string(),
        });

        // Client 1 should receive
        assert!(rx1.try_recv().is_ok());
        // Client 2 should not
        assert!(rx2.try_recv().is_err());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_subscription_flow() {
    let server = setup_test_server().await;

    // Connect WebSocket
    let ws = connect_websocket(&server).await;

    // Subscribe to conversation
    ws.send(json!({"type": "subscribe", "conversation_id": "web-123"})).await;
    let response = ws.recv().await;
    assert_eq!(response["type"], "subscribed");

    // Trigger an agent execution
    server.execute_agent("root", "web-123", "Hello").await;

    // Should receive events
    let event = ws.recv().await;
    assert_eq!(event["type"], "agent_started");

    // Eventually receive tokens
    loop {
        let event = ws.recv().await;
        if event["type"] == "agent_completed" {
            break;
        }
    }

    // Unsubscribe
    ws.send(json!({"type": "unsubscribe", "conversation_id": "web-123"})).await;
    let response = ws.recv().await;
    assert_eq!(response["type"], "unsubscribed");
}
```

### E2E Tests

```typescript
test('multiple tabs receive same events', async ({ browser }) => {
  // Open two browser contexts (simulates two tabs)
  const context1 = await browser.newContext();
  const context2 = await browser.newContext();

  const page1 = await context1.newPage();
  const page2 = await context2.newPage();

  // Both navigate to same conversation
  await page1.goto('/chat/web-123');
  await page2.goto('/chat/web-123');

  // Send message from page1
  await page1.fill('[data-testid="message-input"]', 'Hello');
  await page1.click('[data-testid="send-button"]');

  // Both should see the response streaming
  await expect(page1.locator('[data-testid="assistant-message"]')).toBeVisible();
  await expect(page2.locator('[data-testid="assistant-message"]')).toBeVisible();
});

test('closing panel stops receiving events', async ({ page }) => {
  await page.goto('/dashboard');

  // Open chat panel
  await page.click('[data-testid="open-chat"]');
  await page.waitForSelector('[data-testid="chat-panel"]');

  // Trigger execution
  await page.fill('[data-testid="message-input"]', 'Hello');
  await page.click('[data-testid="send-button"]');

  // Close panel mid-stream
  await page.click('[data-testid="close-panel"]');

  // Verify no more events are processed (check console or network)
  // ...
});
```

---

## Security Considerations

### Authentication (Future)
- Currently no auth on WebSocket
- Future: Send auth token on connect
- Validate token, associate with user
- Only allow subscription to user's conversations

### Authorization (Future)
- Before confirming subscription, verify user owns conversation
- `manager.subscribe()` should check permissions
- Return `SubscriptionError` if unauthorized

### Rate Limiting
- Limit subscribe/unsubscribe rate per client
- Limit total subscriptions per client
- Prevent subscription flooding

### Input Validation
- Validate conversation_id format
- Reject malformed messages
- Handle JSON parse errors gracefully

---

## Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `gateway/src/websocket/manager.rs` | WebSocketManager, subscription tracking |
| `gateway/src/websocket/routing.rs` | EventRouting logic |
| `apps/ui/src/services/events/transport.ts` | New EventTransport class |
| `apps/ui/src/services/events/hooks.ts` | React hooks for subscription |

### Modified Files

| File | Changes |
|------|---------|
| `gateway/src/websocket/messages.rs` | Add Subscribe/Unsubscribe, Subscribed/Unsubscribed messages |
| `gateway/src/websocket/handler.rs` | Integrate WebSocketManager, handle subscription messages |
| `gateway/src/events/mod.rs` | Add routing() method to GatewayEvent |
| `apps/ui/src/features/agent/WebChatPanel.tsx` | Use new subscription hooks |
| `apps/ui/src/features/dashboard/Dashboard.tsx` | Use global event hook for stats |

---

## Rollback Plan

If issues arise after deployment:

1. **Feature flag**: Add `USE_NEW_EVENT_SYSTEM` flag
2. **Fallback**: Keep old broadcast code behind flag
3. **Gradual rollout**: Enable for subset of users first
4. **Monitoring**: Track subscription counts, event delivery latency

---

## Success Criteria

1. ✅ Multiple browser tabs can subscribe to different conversations
2. ✅ Same conversation in multiple tabs: all receive events
3. ✅ Panel open → receives events; panel closed → no events
4. ✅ Delegation events appear in real-time in parent conversation
5. ✅ Dashboard stats update for all connected clients
6. ✅ Reconnection re-establishes subscriptions
7. ✅ No event leakage between conversations
8. ✅ No duplicate events
9. ✅ Proper cleanup on disconnect
