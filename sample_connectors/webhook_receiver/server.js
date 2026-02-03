/**
 * AgentZero Webhook Receiver
 *
 * A simple HTTP server that receives webhook callbacks from AgentZero connectors.
 * Use this as a starting point for building your own connectors.
 *
 * Usage:
 *   npm install
 *   npm start
 *
 * The server runs on port 8080 by default (configurable via PORT env var).
 */

const express = require('express');
const app = express();

const PORT = process.env.PORT || 8080;

// Middleware to parse JSON bodies
app.use(express.json());

// Request logging middleware
app.use((req, res, next) => {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${req.method} ${req.path}`);
  next();
});

// Health check endpoint
app.get('/health', (req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// HEAD request for connectivity testing (used by AgentZero connector test)
app.head('/webhook', (req, res) => {
  res.status(200).end();
});

/**
 * Main webhook endpoint
 *
 * AgentZero sends POST requests here with the following payload:
 * {
 *   "context": {
 *     "session_id": "sess-xxx",
 *     "thread_id": null,
 *     "agent_id": "root",
 *     "timestamp": "2024-01-15T09:00:00Z"
 *   },
 *   "capability": "respond",
 *   "payload": {
 *     "message": "The agent's response",
 *     "execution_id": "exec-xxx",
 *     "conversation_id": "conv-xxx"
 *   }
 * }
 */
app.post('/webhook', (req, res) => {
  const { context, capability, payload } = req.body;

  // Extract fields from nested structure
  const message = payload?.message || '';
  const executionId = payload?.execution_id || 'N/A';
  const conversationId = payload?.conversation_id || 'N/A';

  console.log('\n' + '='.repeat(60));
  console.log('WEBHOOK RECEIVED');
  console.log('='.repeat(60));
  console.log(`Capability: ${capability}`);
  console.log(`Session: ${context?.session_id || 'N/A'}`);
  console.log(`Execution: ${executionId}`);
  console.log(`Conversation: ${conversationId}`);
  console.log(`Agent: ${context?.agent_id || 'N/A'}`);
  console.log(`Timestamp: ${context?.timestamp || 'N/A'}`);
  console.log('-'.repeat(60));
  console.log('MESSAGE:');
  console.log(message || '(empty)');
  console.log('='.repeat(60) + '\n');

  // Process the webhook payload here
  // Examples:
  // - Store in database
  // - Forward to another service
  // - Send notification
  // - Trigger workflow

  // Respond with success
  res.json({
    success: true,
    message: 'Webhook processed successfully',
    received_at: new Date().toISOString()
  });
});

// Catch-all for testing
app.post('*', (req, res) => {
  console.log('\nReceived POST to:', req.path);
  console.log('Body:', JSON.stringify(req.body, null, 2));
  res.json({ success: true, path: req.path });
});

// Start server
app.listen(PORT, () => {
  console.log(`
╔══════════════════════════════════════════════════════════╗
║        AgentZero Webhook Receiver                        ║
╠══════════════════════════════════════════════════════════╣
║  Server running on http://localhost:${PORT}                 ║
║                                                          ║
║  Endpoints:                                              ║
║    GET  /health   - Health check                         ║
║    POST /webhook  - Webhook receiver                     ║
║                                                          ║
║  Register this connector in AgentZero:                   ║
║    curl -X POST http://localhost:18791/api/connectors \\  ║
║      -H "Content-Type: application/json" \\               ║
║      -d '{                                               ║
║        "id": "local-webhook",                            ║
║        "name": "Local Webhook",                          ║
║        "transport": {                                    ║
║          "type": "http",                                 ║
║          "callback_url": "http://localhost:${PORT}/webhook",║
║          "method": "POST",                               ║
║          "headers": {}                                   ║
║        },                                                ║
║        "enabled": true                                   ║
║      }'                                                  ║
╚══════════════════════════════════════════════════════════╝
  `);
});
