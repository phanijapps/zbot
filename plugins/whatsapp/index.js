#!/usr/bin/env node

/**
 * WhatsApp Plugin for AgentZero
 *
 * Receives messages from WhatsApp and forwards them to AgentZero.
 * Responds back to WhatsApp when the agent sends replies.
 *
 * Authentication: QR code scan (first run) or saved auth state (subsequent runs)
 *
 * Optional Environment Variables:
 * - WHATSAPP_PLUGIN_DEBUG: Set to 'true' or '1' to enable debug logging (default: off)
 * - WHATSAPP_MARK_READ: Set to 'true' to mark messages as read when processed (default: true)
 */

const readline = require('readline');
const fs = require('fs');
const path = require('path');
const pino = require('pino');

// Configuration
const PLUGIN_ID = 'whatsapp';
const LOGGING_ENABLED = process.env.WHATSAPP_PLUGIN_DEBUG === 'true' ||
                        process.env.WHATSAPP_PLUGIN_DEBUG === '1' ||
                        process.env.WHATSAPP_PLUGIN_DEBUG === true;
const MARK_READ = process.env.WHATSAPP_MARK_READ !== 'false'; // Default true

// Rolling log configuration
const LOG_DIR = __dirname;
const LOG_FILE = path.join(LOG_DIR, 'plugin.log');
const AUTH_DIR = path.join(__dirname, 'auth');
const MAX_LOG_SIZE = 5 * 1024 * 1024; // 5MB
const MAX_LOG_FILES = 3;

/**
 * Rolling file logger
 */
class Logger {
  constructor(file, maxSize, maxFiles, enabled) {
    this.file = file;
    this.maxSize = maxSize;
    this.maxFiles = maxFiles;
    this.enabled = enabled;
  }

  _rotate() {
    if (!this.enabled) return;
    try {
      const oldest = `${this.file}.${this.maxFiles}`;
      if (fs.existsSync(oldest)) {
        fs.unlinkSync(oldest);
      }

      for (let i = this.maxFiles - 1; i >= 1; i--) {
        const oldFile = `${this.file}.${i}`;
        const newFile = `${this.file}.${i + 1}`;
        if (fs.existsSync(oldFile)) {
          fs.renameSync(oldFile, newFile);
        }
      }

      if (fs.existsSync(this.file)) {
        fs.renameSync(this.file, `${this.file}.1`);
      }
    } catch (e) {
      // Ignore rotation errors
    }
  }

  _checkRotate() {
    if (!this.enabled) return;
    try {
      const stats = fs.statSync(this.file);
      if (stats.size >= this.maxSize) {
        this._rotate();
      }
    } catch (e) {
      // File doesn't exist yet
    }
  }

  log(level, ...args) {
    if (!this.enabled) return;

    this._checkRotate();

    const timestamp = new Date().toISOString();
    const message = args.map(a => {
      if (typeof a === 'object') {
        try {
          return JSON.stringify(a);
        } catch (e) {
          return String(a);
        }
      }
      return String(a);
    }).join(' ');

    const line = `[${timestamp}] [${level}] ${message}\n`;

    try {
      fs.appendFileSync(this.file, line);
    } catch (e) {
      console.error(line.trim());
    }

    console.error(`[${level}] ${message}`);
  }

  info(...args) { this.log('INFO', ...args); }
  warn(...args) { this.log('WARN', ...args); }
  error(...args) { this.log('ERROR', ...args); }
  debug(...args) { this.log('DEBUG', ...args); }
}

const logger = new Logger(LOG_FILE, MAX_LOG_SIZE, MAX_LOG_FILES, LOGGING_ENABLED);

// Capabilities this plugin provides
const CAPABILITIES = [
  {
    name: 'send_message',
    description: 'Send a text message to a WhatsApp contact or group',
    schema: {
      type: 'object',
      properties: {
        to: { type: 'string', description: 'WhatsApp JID (e.g., 1234567890@s.whatsapp.net or group@g.us)' },
        text: { type: 'string', description: 'Message text' }
      },
      required: ['to', 'text']
    }
  },
{
  name: 'mark_read',
  description: 'Mark messages as read',
  schema: {
    type: 'object',
    properties: {
      jid: { type: 'string', description: 'Chat JID to mark as read' },
      message_ids: { type: 'array', items: { type: 'string' }, description: 'Message IDs to mark as read' }
    },
    required: ['jid', 'message_ids']
  }
}
];

// Resources this plugin exposes
const RESOURCES = [
  {
    name: 'chats',
    description: 'List of recent WhatsApp chats'
  }
];

// State
let isRunning = true;
let messageCount = 0;
let sock = null;
let connectionState = 'disconnected';

// Ensure auth directory exists
if (!fs.existsSync(AUTH_DIR)) {
  fs.mkdirSync(AUTH_DIR, { recursive: true });
}

/**
 * Send a message to AgentZero gateway (stdout).
 * IMPORTANT: Only valid JSON should go to stdout!
 */
function sendMessage(msg) {
  try {
    const json = JSON.stringify(msg);
    process.stdout.write(json + '\n');
    logger.debug('Sent to gateway:', msg.type || 'unknown');
  } catch (e) {
    logger.error('Failed to serialize message for gateway:', e);
  }
}

/**
 * Send hello to register with gateway.
 */
function sendHello() {
  logger.info('Sending hello to gateway...');
  sendMessage({
    type: 'hello',
    adapter_id: PLUGIN_ID,
    capabilities: CAPABILITIES,
    resources: RESOURCES,
    resume: null
  });
}

/**
 * Send pong for heartbeat.
 */
function sendPong() {
  sendMessage({ type: 'pong' });
}

/**
 * Forward inbound message to AgentZero.
 */
function sendInbound(msg) {
  const jid = msg.key.remoteJid;
  const messageId = msg.key.id;
  const threadId = `${jid}:${messageId}`;

  // Extract text from various message types
  let text = '';
  if (msg.message?.conversation) {
    text = msg.message.conversation;
  } else if (msg.message?.extendedTextMessage?.text) {
    text = msg.message.extendedTextMessage.text;
  } else if (msg.message?.imageMessage?.caption) {
    text = `[Image] ${msg.message.imageMessage.caption}`;
  } else if (msg.message?.videoMessage?.caption) {
    text = `[Video] ${msg.message.videoMessage.caption}`;
  } else if (msg.message?.documentMessage?.fileName) {
    text = `[Document] ${msg.message.documentMessage.fileName}`;
  } else if (msg.message?.audioMessage) {
    text = '[Audio message]';
  }

  if (!text) {
    logger.debug('Skipping message without extractable text');
    return;
  }

  const isGroup = jid.endsWith('@g.us');
  const senderId = isGroup ? msg.key.participant || jid : jid;
  const pushName = msg.pushName || '';

  logger.info('Forwarding inbound message to gateway:', {
    jid,
    sender: senderId,
    threadId,
    textLength: text.length,
    isGroup
  });

  sendMessage({
    type: 'inbound',
    text,
    thread_id: threadId,
    sender: {
      id: senderId,
      name: pushName
    },
    agent_id: null,
    metadata: {
      whatsapp_jid: jid,
      whatsapp_message_id: messageId,
      whatsapp_from_me: msg.key.fromMe,
      whatsapp_push_name: pushName,
      whatsapp_is_group: isGroup,
      whatsapp_participant: msg.key.participant || null
    }
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
 * Send ack for outbox item.
 */
function sendAck(outboxId) {
  sendMessage({
    type: 'ack',
    outbox_id: outboxId
  });
}

/**
 * Send fail for outbox item.
 */
function sendFail(outboxId, error, retryAfter = null) {
  sendMessage({
    type: 'fail',
    outbox_id: outboxId,
    error,
    retry_after_seconds: retryAfter
  });
}

/**
 * Connect to WhatsApp using Baileys.
 */
async function connectWhatsApp() {
  const { makeWASocket, DisconnectReason, useMultiFileAuthState } = require('@whiskeysockets/baileys');

  logger.info('Connecting to WhatsApp...');
  console.error('[WhatsApp] Initializing connection...');

  const { state, saveCreds } = await useMultiFileAuthState(AUTH_DIR);
  console.error('[WhatsApp] Auth state loaded, creating socket...');

  sock = makeWASocket({
    auth: state,
    logger: pino({ level: 'silent' }),
    browser: ['AgentZero', 'Chrome', '120.0.0'],
    connectOnMobile: true,
    getMessage: async (key) => {
      return { conversation: '' };
    }
  });

  console.error('[WhatsApp] Socket created, setting up event handlers...');

  // Handle connection updates
  sock.ev.on('connection.update', async (update) => {
    const { connection, lastDisconnect, qr } = update;

    // Log all connection updates to stderr for debugging
    console.error('[WhatsApp] Connection update:', JSON.stringify({
      connection,
      hasQr: !!qr,
      disconnectReason: lastDisconnect?.error?.output?.statusCode,
      disconnectMessage: lastDisconnect?.error?.message
    }));

    if (qr) {
      // QR code received - save as image and display in terminal
      const QRCode = require('qrcode');
      const qrImagePath = path.join(AUTH_DIR, 'qr.png');

      console.error('\n========================================');
      console.error('WhatsApp QR Code - Scan with your phone:');
      console.error('Settings > Linked Devices > Link a Device');
      console.error('========================================\n');

      try {
        // Save QR as PNG image
        await QRCode.toFile(qrImagePath, qr, { width: 400 });
        console.error(`QR code saved to: ${qrImagePath}`);
        console.error('Open this image file and scan it with WhatsApp.\n');
      } catch (e) {
        logger.error('Failed to save QR image:', e.message);
      }

      try {
        // Also display in terminal
        const qrString = await QRCode.toString(qr, { type: 'terminal', small: true });
        console.error(qrString);
      } catch (e) {
        // Fallback: just print the raw QR string
        console.error('QR String (use online generator):', qr);
      }
      connectionState = 'qr_pending';
    }

    if (connection === 'close') {
      connectionState = 'disconnected';
      const statusCode = lastDisconnect?.error?.output?.statusCode;
      const shouldReconnect = statusCode !== DisconnectReason.loggedOut;

      logger.warn('WhatsApp connection closed:', { statusCode, message: lastDisconnect?.error?.message });

      // Handle restartRequired - this is EXPECTED after QR scan
      if (statusCode === DisconnectReason.restartRequired) {
        console.error('\n[WhatsApp] QR scanned! Reconnecting with credentials...\n');
        logger.info('QR scanned, reconnecting with new credentials...');
        // Reconnect immediately - auth state now has credentials
        setTimeout(() => connectWhatsApp(), 1000);
        return;
      }

      // Handle specific error codes
      if (statusCode === 405) {
        console.error('\n========================================');
        console.error('WhatsApp Connection Blocked (405)');
        console.error('This usually means:');
        console.error('  - Too many connection attempts (wait 15-30 min)');
        console.error('  - IP flagged as suspicious (try different network)');
        console.error('  - Try using a VPN with residential IP');
        console.error('========================================\n');
      }

      if (shouldReconnect) {
        // Use longer delay for rate limiting
        const delay = statusCode === 405 ? 30000 : 5000;
        logger.info(`Reconnecting to WhatsApp in ${delay/1000}s...`);
        setTimeout(() => connectWhatsApp(), delay);
      } else {
        logger.error('WhatsApp logged out. Delete auth folder and restart to re-authenticate.');
      }
    } else if (connection === 'open') {
      connectionState = 'connected';
      logger.info('WhatsApp connected successfully');
      console.error('WhatsApp connected successfully!');
    }
  });

  // Save credentials when updated
  sock.ev.on('creds.update', saveCreds);

  // Handle incoming messages
  sock.ev.on('messages.upsert', async ({ messages, type }) => {
    logger.debug(`messages.upsert event: type=${type}, count=${messages.length}`);

    if (type !== 'notify') {
      logger.debug(`Skipping non-notify message type: ${type}`);
      return;
    }

    for (const msg of messages) {
      logger.debug(`Message: fromMe=${msg.key.fromMe}, jid=${msg.key.remoteJid}, id=${msg.key.id}`);

      // Skip messages from self
      if (msg.key.fromMe) {
        logger.debug('Skipping message from self');
        continue;
      }

      // Skip status messages
      if (msg.key.remoteJid === 'status@broadcast') {
        logger.debug('Skipping status broadcast');
        continue;
      }

      messageCount++;
      logger.info(`Processing message #${messageCount} from ${msg.key.remoteJid}`);

      // Forward to AgentZero
      sendInbound(msg);

      // Mark as read if configured
      if (MARK_READ && sock) {
        try {
          await sock.readMessages([msg.key]);
        } catch (e) {
          logger.debug('Failed to mark message as read:', e.message);
        }
      }
    }
  });
}

/**
 * Handle capability invocation from AgentZero.
 */
async function handleCapabilityInvoke(requestId, capability, payload) {
  logger.debug('Capability invoke:', capability, payload);

  if (!sock || connectionState !== 'connected') {
    sendCapabilityResponse(requestId, {
      success: false,
      error: 'WhatsApp not connected'
    });
    return;
  }

  try {
    switch (capability) {
      case 'send_message': {
        const { to, text } = payload;

        if (!to || !text) {
          sendCapabilityResponse(requestId, {
            success: false,
            error: 'Missing required fields: to, text'
          });
          return;
        }

        const result = await sock.sendMessage(to, { text });
        logger.info('Sent message to WhatsApp:', { to, id: result.key.id });

        sendCapabilityResponse(requestId, {
          success: true,
          message_id: result.key.id,
          to: result.key.remoteJid
        });
        break;
      }

      case 'mark_read': {
        const { jid, message_ids } = payload;

        if (!jid || !message_ids || !Array.isArray(message_ids)) {
          sendCapabilityResponse(requestId, {
            success: false,
            error: 'Missing required fields: jid, message_ids'
          });
          return;
        }

        // Build message keys for read receipts
        const keys = message_ids.map(id => ({
          remoteJid: jid,
          id,
          fromMe: false
        }));

        await sock.readMessages(keys);

        sendCapabilityResponse(requestId, {
          success: true
        });
        break;
      }

      default:
        sendCapabilityResponse(requestId, {
          success: false,
          error: `Unknown capability: ${capability}`
        });
    }
  } catch (error) {
    logger.error(`Capability ${capability} failed:`, error);
    sendCapabilityResponse(requestId, {
      success: false,
      error: error.message || String(error)
    });
  }
}

/**
 * Handle resource query from AgentZero.
 */
async function handleResourceQuery(requestId, resource, params) {
  logger.debug('Resource query:', resource, params);

  if (!sock || connectionState !== 'connected') {
    sendResourceResponse(requestId, {
      error: 'WhatsApp not connected'
    });
    return;
  }

  try {
    switch (resource) {
      case 'chats': {
        // Get chats from store if available
        const chats = [];
        try {
          const chatIds = await sock.groupFetchAllParticipating();
          for (const [id, metadata] of Object.entries(chatIds || {})) {
            chats.push({
              id,
              name: metadata.subject || id,
              is_group: true
            });
          }
        } catch (e) {
          // Groups not available
        }

        sendResourceResponse(requestId, { chats });
        break;
      }

      default:
        sendResourceResponse(requestId, {
          error: `Unknown resource: ${resource}`
        });
    }
  } catch (error) {
    logger.error(`Resource ${resource} query failed:`, error);
    sendResourceResponse(requestId, {
      error: error.message || String(error)
    });
  }
}

/**
 * Handle outbox item (message to send to WhatsApp).
 */
async function handleOutboxItem(outboxId, capability, payload) {
  logger.info('Outbox item:', outboxId, capability, JSON.stringify(payload));

  if (!sock || connectionState !== 'connected') {
    sendFail(outboxId, 'WhatsApp not connected');
    return;
  }

  try {
    if (capability === 'send_message' || capability === 'reply' || capability === 'respond') {
      const text = payload.text || payload.message;
      let targetJid = payload.to;

      // If thread_id is provided, extract the JID from it (format: jid:message_id)
      if (payload.thread_id && payload.thread_id.includes(':')) {
        targetJid = payload.thread_id.split(':')[0];
      }

      if (!targetJid) {
        logger.error('No target JID found in payload:', payload);
        sendFail(outboxId, 'No recipient specified');
        return;
      }

      if (!text) {
        logger.error('No text/message found in payload:', payload);
        sendFail(outboxId, 'No text specified');
        return;
      }

      const result = await sock.sendMessage(targetJid, { text });

      logger.info('Sent outbox message to WhatsApp:', { outboxId, to: targetJid, id: result.key.id });
      sendAck(outboxId);
    } else {
      sendFail(outboxId, `Unknown outbox capability: ${capability}`);
    }
  } catch (error) {
    logger.error(`Outbox item ${outboxId} failed:`, error);
    sendFail(outboxId, error.message || String(error));
  }
}

/**
 * Handle incoming message from AgentZero gateway.
 */
async function handleMessage(msg) {
  try {
    const { type } = msg;

    switch (type) {
      case 'hello_ack':
        logger.info('Connected to gateway:', { serverTime: msg.server_time, heartbeat: msg.heartbeat_seconds });
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
        logger.error('Gateway error:', msg.message);
        break;

      default:
        logger.warn('Unknown message type:', type);
    }
  } catch (error) {
    logger.error('Failed to handle message:', error);
  }
}

/**
 * Main entry point.
 */
async function main() {
  console.error(`Starting WhatsApp plugin... (logging: ${LOGGING_ENABLED ? 'enabled' : 'disabled'})`);
  if (LOGGING_ENABLED) {
    logger.info('Starting WhatsApp plugin...');
    logger.info('Log file:', LOG_FILE);
    logger.info('Auth directory:', AUTH_DIR);
  }

  // Set up readline interface for stdin (AgentZero gateway)
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false
  });

  // Handle incoming lines from gateway
  rl.on('line', (line) => {
    if (!line.trim()) return;

    try {
      const msg = JSON.parse(line);
      logger.debug('Received from gateway:', msg.type || 'unknown');
      handleMessage(msg);
    } catch (error) {
      logger.error('Failed to parse message from gateway:', { line: line.substring(0, 100), error: error.message });
    }
  });

  // Handle stdin close (gateway disconnected)
  rl.on('close', () => {
    console.error('Gateway disconnected, exiting...');
    logger.info('Gateway disconnected, exiting...');
    isRunning = false;
    if (sock) {
      sock.end();
    }
    process.exit(0);
  });

  // Initialize WhatsApp connection
  try {
    await connectWhatsApp();
  } catch (error) {
    logger.error('Failed to connect to WhatsApp:', error);
    console.error('Failed to connect to WhatsApp:', error.message);
  }

  // Send hello to register with gateway
  sendHello();

  console.error('WhatsApp plugin ready, waiting for QR code scan...');
  logger.info('WhatsApp plugin ready, waiting for QR code scan...');

  // Handle graceful shutdown
  process.on('SIGTERM', async () => {
    console.error('Received SIGTERM, shutting down...');
    logger.info('Received SIGTERM, shutting down...');
    if (sock) {
      sock.end();
    }
    process.exit(0);
  });

  process.on('SIGINT', async () => {
    console.error('Received SIGINT, shutting down...');
    logger.info('Received SIGINT, shutting down...');
    if (sock) {
      sock.end();
    }
    process.exit(0);
  });
}

// Run main
main().catch((error) => {
  logger.error('Fatal error:', error);
  process.exit(1);
});

