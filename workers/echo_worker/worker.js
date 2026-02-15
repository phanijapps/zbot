// ============================================================================
// ECHO WORKER
//
// A reference bridge worker that demonstrates the full AgentZero WebSocket
// worker protocol. Connects to the gateway at /bridge/ws, declares
// capabilities and resources, and handles all message types.
//
// Usage:
//   npm install
//   npm start                           # defaults to ws://localhost:18791
//   GATEWAY_URL=ws://host:port npm start
//
// What it does:
//   - Capability "echo": returns whatever payload it receives
//   - Capability "uppercase": returns the payload text uppercased
//   - Resource "status": returns uptime and message counts
//   - Inbound: you can paste a JSON Inbound message to trigger an agent
//
// ============================================================================

const WebSocket = require("ws");

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const GATEWAY_URL = process.env.GATEWAY_URL || "ws://localhost:18791";
const ADAPTER_ID = process.env.ADAPTER_ID || "echo-worker";
const WS_ENDPOINT = `${GATEWAY_URL}/bridge/ws`;

// Reconnection settings
const RECONNECT_DELAY_MS = 3000;
const MAX_RECONNECT_DELAY_MS = 30000;
let reconnectAttempt = 0;

// Stats
const stats = {
  connectedAt: null,
  messagesReceived: 0,
  outboxItemsProcessed: 0,
  resourceQueriesHandled: 0,
  capabilityInvocations: 0,
};

// ---------------------------------------------------------------------------
// Protocol: Hello message
// ---------------------------------------------------------------------------

/** Build the Hello handshake message sent on connection. */
function buildHello() {
  return {
    type: "hello",
    adapter_id: ADAPTER_ID,
    capabilities: [
      {
        name: "echo",
        description: "Echoes back the payload it receives",
        schema: {
          type: "object",
          properties: {
            text: { type: "string", description: "Text to echo back" },
          },
        },
      },
      {
        name: "uppercase",
        description: "Returns the payload text in uppercase",
        schema: {
          type: "object",
          properties: {
            text: { type: "string", description: "Text to uppercase" },
          },
        },
      },
    ],
    resources: [
      {
        name: "status",
        description: "Worker uptime, connection info, and message counts",
      },
    ],
  };
}

// ---------------------------------------------------------------------------
// Message handlers
// ---------------------------------------------------------------------------

/**
 * Handle an OutboxItem pushed from the server.
 * The server sends these when an agent uses `respond` and the response
 * is routed to this worker's connector.
 *
 * Protocol contract: worker MUST reply with either Ack or Fail.
 */
function handleOutboxItem(ws, msg) {
  stats.outboxItemsProcessed++;
  const { outbox_id, capability, payload } = msg;

  log("outbox", `capability=${capability} outbox_id=${outbox_id}`);
  log("outbox", `payload: ${JSON.stringify(payload)}`);

  // For an echo worker we just acknowledge delivery
  send(ws, { type: "ack", outbox_id });
}

/**
 * Handle a ResourceQuery from the server.
 * The server sends these when an agent uses the `query_resource` tool
 * to read data from this worker.
 *
 * Protocol contract: worker MUST reply with ResourceResponse containing
 * the same request_id.
 */
function handleResourceQuery(ws, msg) {
  stats.resourceQueriesHandled++;
  const { request_id, resource, params } = msg;

  log("resource", `resource=${resource} request_id=${request_id}`);
  if (params) log("resource", `params: ${JSON.stringify(params)}`);

  let data;
  switch (resource) {
    case "status":
      data = {
        adapter_id: ADAPTER_ID,
        uptime_seconds: stats.connectedAt
          ? Math.floor((Date.now() - stats.connectedAt) / 1000)
          : 0,
        messages_received: stats.messagesReceived,
        outbox_items_processed: stats.outboxItemsProcessed,
        resource_queries_handled: stats.resourceQueriesHandled,
        capability_invocations: stats.capabilityInvocations,
      };
      break;
    default:
      data = { error: `Unknown resource: ${resource}` };
  }

  send(ws, { type: "resource_response", request_id, data });
}

/**
 * Handle a CapabilityInvoke from the server.
 * The server sends these when an agent uses the `query_resource` tool
 * with the `invoke` action targeting one of this worker's capabilities.
 *
 * Protocol contract: worker MUST reply with CapabilityResponse containing
 * the same request_id.
 */
function handleCapabilityInvoke(ws, msg) {
  stats.capabilityInvocations++;
  const { request_id, capability, payload } = msg;

  log("invoke", `capability=${capability} request_id=${request_id}`);
  log("invoke", `payload: ${JSON.stringify(payload)}`);

  let result;
  switch (capability) {
    case "echo":
      result = { echoed: payload };
      break;
    case "uppercase":
      result = {
        text: typeof payload?.text === "string"
          ? payload.text.toUpperCase()
          : JSON.stringify(payload).toUpperCase(),
      };
      break;
    default:
      result = { error: `Unknown capability: ${capability}` };
  }

  send(ws, { type: "capability_response", request_id, result });
}

// ---------------------------------------------------------------------------
// WebSocket lifecycle
// ---------------------------------------------------------------------------

function connect() {
  log("ws", `Connecting to ${WS_ENDPOINT}...`);

  const ws = new WebSocket(WS_ENDPOINT);

  ws.on("open", () => {
    reconnectAttempt = 0;
    stats.connectedAt = Date.now();
    log("ws", "Connected! Sending Hello...");
    send(ws, buildHello());
  });

  ws.on("message", (raw) => {
    stats.messagesReceived++;
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch (e) {
      log("error", `Failed to parse message: ${e.message}`);
      return;
    }

    switch (msg.type) {
      // ── Server acknowledgements ──────────────────────────────────────
      case "hello_ack":
        log("ws", `Hello acknowledged! heartbeat=${msg.heartbeat_seconds}s`);
        break;

      // ── Heartbeat ────────────────────────────────────────────────────
      case "ping":
        send(ws, { type: "pong" });
        break;

      // ── Outbox push ──────────────────────────────────────────────────
      case "outbox_item":
        handleOutboxItem(ws, msg);
        break;

      // ── Resource query ───────────────────────────────────────────────
      case "resource_query":
        handleResourceQuery(ws, msg);
        break;

      // ── Capability invocation ────────────────────────────────────────
      case "capability_invoke":
        handleCapabilityInvoke(ws, msg);
        break;

      // ── Server error ─────────────────────────────────────────────────
      case "error":
        log("error", `Server error: ${msg.message}`);
        break;

      default:
        log("warn", `Unknown message type: ${msg.type}`);
        log("warn", JSON.stringify(msg));
    }
  });

  ws.on("close", (code, reason) => {
    stats.connectedAt = null;
    log("ws", `Disconnected (code=${code} reason=${reason || "none"})`);
    scheduleReconnect();
  });

  ws.on("error", (err) => {
    log("error", err.message);
    // 'close' will fire after this, triggering reconnect
  });
}

function scheduleReconnect() {
  reconnectAttempt++;
  const delay = Math.min(
    RECONNECT_DELAY_MS * Math.pow(2, reconnectAttempt - 1),
    MAX_RECONNECT_DELAY_MS
  );
  log("ws", `Reconnecting in ${delay}ms (attempt ${reconnectAttempt})...`);
  setTimeout(connect, delay);
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

function send(ws, msg) {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  }
}

function log(tag, message) {
  const ts = new Date().toISOString().slice(11, 23);
  console.log(`[${ts}] [${tag}] ${message}`);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

console.log(`
  ┌─────────────────────────────────────────────┐
  │         AgentZero Echo Worker                │
  │                                              │
  │  adapter_id:  ${ADAPTER_ID.padEnd(29)}│
  │  gateway:     ${WS_ENDPOINT.padEnd(29)}│
  │                                              │
  │  Capabilities: echo, uppercase               │
  │  Resources:    status                        │
  └─────────────────────────────────────────────┘
`);

connect();
