#!/usr/bin/env node

/**
 * Example Plugin - Reference Implementation
 *
 * This plugin demonstrates the AgentZero bridge protocol for STDIO plugins.
 * It communicates with the gateway via newline-delimited JSON messages.
 *
 * Protocol Overview:
 * - Plugin sends messages to stdout (each message is a JSON line)
 * - Plugin receives messages from stdin (each message is a JSON line)
 * - First message must be 'hello' to register capabilities
 * - Server responds with 'hello_ack' and periodic 'ping' messages
 * - Plugin can send 'inbound' messages to trigger agent sessions
 */

const readline = require('readline');

// Plugin configuration (from plugin.json)
const PLUGIN_ID = 'example-plugin';

// Capabilities this plugin provides (outbound actions the agent can invoke)
const CAPABILITIES = [
  {
    name: 'echo',
    description: 'Echo back the provided message',
    schema: {
      type: 'object',
      properties: {
        message: { type: 'string', description: 'Message to echo back' }
      },
      required: ['message']
    }
  },
  {
    name: 'get_time',
    description: 'Get the current server time',
    schema: {
      type: 'object',
      properties: {}
    }
  }
];

// Resources this plugin exposes (queryable data)
const RESOURCES = [
  {
    name: 'status',
    description: 'Get the current plugin status'
  },
  {
    name: 'info',
    description: 'Get plugin information'
  }
];

// State
let isRunning = true;
let messageCount = 0;

/**
 * Send a message to the gateway (stdout).
 * Each message must be a single line of JSON.
 */
function sendMessage(msg) {
  const json = JSON.stringify(msg);
  process.stdout.write(json + '\n');
}

/**
 * Send hello message to register with the gateway.
 */
function sendHello() {
  sendMessage({
    type: 'hello',
    adapter_id: PLUGIN_ID,
    capabilities: CAPABILITIES,
    resources: RESOURCES,
    resume: null
  });
}

/**
 * Send pong response for heartbeat.
 */
function sendPong() {
  sendMessage({ type: 'pong' });
}

/**
 * Send an inbound message to trigger an agent session.
 */
function sendInbound(text, options = {}) {
  sendMessage({
    type: 'inbound',
    text,
    thread_id: options.threadId || null,
    sender: options.sender || null,
    agent_id: options.agentId || null,
    metadata: options.metadata || null
  });
}

/**
 * Send capability response.
 */
function sendCapabilityResponse(requestId, result) {
  sendMessage({
    type: 'capability_response',
    request_id: requestId,
    result
  });
}

/**
 * Send resource response.
 */
function sendResourceResponse(requestId, data) {
  sendMessage({
    type: 'resource_response',
    request_id: requestId,
    data
  });
}

/**
 * Send ack for outbox item delivery.
 */
function sendAck(outboxId) {
  sendMessage({
    type: 'ack',
    outbox_id: outboxId
  });
}

/**
 * Handle a capability invocation.
 */
async function handleCapabilityInvoke(requestId, capability, payload) {
  console.error(`[DEBUG] Capability invoke: ${capability}`, payload);

  switch (capability) {
    case 'echo':
      sendCapabilityResponse(requestId, {
        success: true,
        echoed: payload.message,
        timestamp: new Date().toISOString()
      });
      break;

    case 'get_time':
      sendCapabilityResponse(requestId, {
        success: true,
        time: new Date().toISOString(),
        unix: Date.now()
      });
      break;

    default:
      sendCapabilityResponse(requestId, {
        success: false,
        error: `Unknown capability: ${capability}`
      });
  }
}

/**
 * Handle a resource query.
 */
async function handleResourceQuery(requestId, resource, params) {
  console.error(`[DEBUG] Resource query: ${resource}`, params);

  switch (resource) {
    case 'status':
      sendResourceResponse(requestId, {
        running: isRunning,
        message_count: messageCount,
        uptime: process.uptime(),
        memory: process.memoryUsage()
      });
      break;

    case 'info':
      sendResourceResponse(requestId, {
        id: PLUGIN_ID,
        name: 'Example Plugin',
        version: '1.0.0',
        capabilities: CAPABILITIES.map(c => c.name),
        resources: RESOURCES.map(r => r.name)
      });
      break;

    default:
      sendResourceResponse(requestId, {
        error: `Unknown resource: ${resource}`
      });
  }
}

/**
 * Handle an outbox item (message to deliver externally).
 * In a real plugin, this would send to an external service.
 */
async function handleOutboxItem(outboxId, capability, payload) {
  console.error(`[DEBUG] Outbox item: ${outboxId}, capability: ${capability}`, payload);

  // For this example, we just ACK everything
  // In a real plugin (like Slack), you'd actually deliver the message
  sendAck(outboxId);
}

/**
 * Handle incoming message from gateway.
 */
async function handleMessage(msg) {
  messageCount++;

  try {
    const { type } = msg;

    switch (type) {
      case 'hello_ack':
        console.error('[INFO] Connected to gateway:', msg.server_time, 'heartbeat:', msg.heartbeat_seconds);
        break;

      case 'ping':
        sendPong();
        break;

      case 'capability_invoke':
        await handleCapabilityInvoke(msg.request_id, msg.capability, msg.payload);
        break;

      case 'resource_query':
        await handleResourceQuery(msg.request_id, msg.resource, msg.params);
        break;

      case 'outbox_item':
        await handleOutboxItem(msg.outbox_id, msg.capability, msg.payload);
        break;

      case 'error':
        console.error('[ERROR] Gateway error:', msg.message);
        break;

      default:
        console.error('[WARN] Unknown message type:', type);
    }
  } catch (error) {
    console.error('[ERROR] Failed to handle message:', error);
  }
}

/**
 * Main entry point.
 */
async function main() {
  console.error('[INFO] Starting example plugin...');

  // Set up readline interface for stdin
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false
  });

  // Handle incoming lines
  rl.on('line', (line) => {
    if (!line.trim()) return;

    try {
      const msg = JSON.parse(line);
      handleMessage(msg);
    } catch (error) {
      console.error('[ERROR] Failed to parse message:', error);
    }
  });

  // Handle stdin close (gateway disconnected)
  rl.on('close', () => {
    console.error('[INFO] Gateway disconnected, exiting...');
    isRunning = false;
    process.exit(0);
  });

  // Send hello to register with gateway
  sendHello();

  // Example: Send an inbound message after 5 seconds (for testing)
  // In a real plugin, this would be triggered by external events
  if (process.env.EXAMPLE_AUTO_MESSAGE === 'true') {
    setTimeout(() => {
      console.error('[INFO] Sending example inbound message...');
      sendInbound('Hello from the example plugin!', {
        sender: { id: 'example-plugin', name: 'Example Plugin' },
        threadId: 'example-thread-1'
      });
    }, 5000);
  }

  console.error('[INFO] Plugin ready, waiting for messages...');
}

// Run main
main().catch((error) => {
  console.error('[FATAL]', error);
  process.exit(1);
});
