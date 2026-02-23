#!/usr/bin/env node

/**
 * Slack Plugin for AgentZero
 *
 * Receives messages from Slack channels and forwards them to AgentZero.
 * Responds back to Slack when the agent sends replies.
 *
 * Required Environment Variables:
 * - SLACK_BOT_TOKEN: Slack bot user OAuth token (xoxb-...)
 * - SLACK_APP_TOKEN: Slack app-level token for Socket Mode (xapp-...)
 *
 * Setup:
 * 1. Create a Slack App at https://api.slack.com/apps
 * 2. Enable Socket Mode (for secure connection without public endpoint)
 * 3. Subscribe to events: message.channels, app_mention
 * 4. Add bot token scopes: chat:write, channels:history, groups:history, im:history
 * 5. Install app to workspace
 * 6. Copy Bot User OAuth Token and App-Level Token
 */

const readline = require('readline');
const fs = require('fs');
const path = require('path');
const { WebClient } = require('@slack/web-api');
const { SocketModeClient } = require('@slack/socket-mode');

// Configuration from environment
const PLUGIN_ID = 'slack';
const BOT_TOKEN = process.env.SLACK_BOT_TOKEN;
const APP_TOKEN = process.env.SLACK_APP_TOKEN;

// Rolling log configuration
const LOG_DIR = __dirname;
const LOG_FILE = path.join(LOG_DIR, 'plugin.log');
const MAX_LOG_SIZE = 5 * 1024 * 1024; // 5MB
const MAX_LOG_FILES = 3;

/**
 * Rolling file logger
 */
class Logger {
  constructor(file, maxSize, maxFiles) {
    this.file = file;
    this.maxSize = maxSize;
    this.maxFiles = maxFiles;
  }

  _rotate() {
    try {
      // Delete oldest log
      const oldest = `${this.file}.${this.maxFiles}`;
      if (fs.existsSync(oldest)) {
        fs.unlinkSync(oldest);
      }

      // Rotate existing logs
      for (let i = this.maxFiles - 1; i >= 1; i--) {
        const oldFile = `${this.file}.${i}`;
        const newFile = `${this.file}.${i + 1}`;
        if (fs.existsSync(oldFile)) {
          fs.renameSync(oldFile, newFile);
        }
      }

      // Rotate current log
      if (fs.existsSync(this.file)) {
        fs.renameSync(this.file, `${this.file}.1`);
      }
    } catch (e) {
      // Ignore rotation errors
    }
  }

  _checkRotate() {
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
      // Fallback to stderr if file write fails
      console.error(line.trim());
    }

    // Also output to stderr for daemon logs
    console.error(`[${level}] ${message}`);
  }

  info(...args) { this.log('INFO', ...args); }
  warn(...args) { this.log('WARN', ...args); }
  error(...args) { this.log('ERROR', ...args); }
  debug(...args) { this.log('DEBUG', ...args); }
}

const logger = new Logger(LOG_FILE, MAX_LOG_SIZE, MAX_LOG_FILES);

// Validate required tokens
if (!BOT_TOKEN) {
  logger.error('SLACK_BOT_TOKEN is required');
  process.exit(1);
}
if (!APP_TOKEN) {
  logger.error('SLACK_APP_TOKEN is required');
  process.exit(1);
}

// Capabilities this plugin provides
const CAPABILITIES = [
  {
    name: 'send_message',
    description: 'Send a message to a Slack channel or user',
    schema: {
      type: 'object',
      properties: {
        channel: { type: 'string', description: 'Channel ID or name' },
        text: { type: 'string', description: 'Message text' },
        thread_ts: { type: 'string', description: 'Thread timestamp to reply in thread' },
        blocks: { type: 'array', description: 'Slack blocks for rich formatting' }
      },
      required: ['channel', 'text']
    }
  },
  {
    name: 'send_ephemeral',
    description: 'Send an ephemeral message visible only to a specific user',
    schema: {
      type: 'object',
      properties: {
        channel: { type: 'string', description: 'Channel ID' },
        user: { type: 'string', description: 'User ID' },
        text: { type: 'string', description: 'Message text' }
      },
      required: ['channel', 'user', 'text']
    }
  },
  {
    name: 'add_reaction',
    description: 'Add a reaction emoji to a message',
    schema: {
      type: 'object',
      properties: {
        channel: { type: 'string', description: 'Channel ID' },
        timestamp: { type: 'string', description: 'Message timestamp' },
        name: { type: 'string', description: 'Emoji name (without :)' }
      },
      required: ['channel', 'timestamp', 'name']
    }
  }
];

// Resources this plugin exposes
const RESOURCES = [
  {
    name: 'channels',
    description: 'List channels the bot is a member of'
  },
  {
    name: 'users',
    description: 'List users in the workspace'
  },
  {
    name: 'team',
    description: 'Get team/workspace info'
  }
];

// State
let isRunning = true;
let messageCount = 0;
let slackClient = null;
let socketClient = null;
let botUserId = null;
let pendingResponses = new Map(); // thread_ts -> { channel, resolve }
let recentlyProcessed = new Map(); // ts -> timestamp (for deduplication)

// Caches
const channelCache = new Map();
const userCache = new Map();

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
function sendInbound(text, slackMessage) {
  const channelId = slackMessage.channel;
  const user = slackMessage.user;
  const threadTs = slackMessage.thread_ts || slackMessage.ts;
  const ts = slackMessage.ts;

  // Build thread ID that combines channel and thread
  const threadId = `${channelId}:${threadTs}`;

  logger.info('Forwarding inbound message to gateway:', {
    channel: channelId,
    user: user,
    threadId: threadId,
    textLength: text.length
  });

  sendMessage({
    type: 'inbound',
    text,
    thread_id: threadId,
    sender: {
      id: user,
      name: userCache.get(user) || user
    },
    agent_id: null,
    metadata: {
      slack_channel: channelId,
      slack_ts: ts,
      slack_thread_ts: slackMessage.thread_ts || null,
      slack_user: user
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
 * Initialize Slack clients.
 */
async function initSlack() {
  logger.info('Initializing Slack client...');

  slackClient = new WebClient(BOT_TOKEN);

  // Get bot user ID
  const authResult = await slackClient.auth.test();
  botUserId = authResult.user_id;
  logger.info(`Connected as @${authResult.user} (${botUserId})`);

  // Cache common channels and users
  await cacheUsers();
  await cacheChannels();

  // Initialize Socket Mode client for receiving events
  socketClient = new SocketModeClient({ appToken: APP_TOKEN });

  // Handle incoming messages
  socketClient.on('message', async ({ event, body }) => {
    logger.debug('Socket message event received:', event.type);
    await handleSlackMessage(event);
  });

  // Handle app mentions
  socketClient.on('app_mention', async ({ event, body }) => {
    logger.debug('App mention event received');
    await handleSlackMessage(event, true);
  });

  // Handle errors
  socketClient.on('error', (error) => {
    logger.error('Socket Mode error:', error);
  });

  // Handle connection events
  socketClient.on('connected', () => {
    logger.info('Socket Mode connected');
  });

  socketClient.on('disconnect', () => {
    logger.warn('Socket Mode disconnected');
  });

  // Start Socket Mode
  await socketClient.start();
  logger.info('Socket Mode started, listening for events...');
}

/**
 * Cache workspace users.
 */
async function cacheUsers() {
  try {
    const result = await slackClient.users.list();
    if (result.members) {
      for (const member of result.members) {
        if (!member.is_bot && !member.deleted) {
          userCache.set(member.id, member.real_name || member.name);
        }
      }
      logger.info(`Cached ${userCache.size} users`);
    }
  } catch (error) {
    logger.warn('Failed to cache users:', error.message);
  }
}

/**
 * Cache channels bot is a member of.
 */
async function cacheChannels() {
  try {
    const result = await slackClient.conversations.list({
      types: 'public_channel,private_channel,mpim,im'
    });
    if (result.channels) {
      for (const channel of result.channels) {
        if (channel.is_member || channel.is_im || channel.is_mpim) {
          channelCache.set(channel.id, channel.name || channel.id);
        }
      }
      logger.info(`Cached ${channelCache.size} channels`);
    }
  } catch (error) {
    logger.warn('Failed to cache channels:', error.message);
  }
}

/**
 * Handle incoming Slack message.
 */
async function handleSlackMessage(event, isMention = false) {
  try {
    logger.debug('Processing Slack event:', {
      type: event.type,
      channel: event.channel,
      user: event.user,
      subtype: event.subtype,
      bot_id: event.bot_id
    });

    // Skip messages from bots (including ourselves)
    if (event.bot_id || event.subtype || event.user === botUserId) {
      logger.debug('Skipping bot/subtype message');
      return;
    }

    // Skip messages without text
    if (!event.text) {
      logger.debug('Skipping message without text');
      return;
    }

    // Deduplicate: Slack sends both 'message' and 'app_mention' for @mentions
    // Skip if we've already processed this message timestamp recently
    const msgKey = `${event.channel}:${event.ts}`;
    const now = Date.now();
    if (recentlyProcessed.has(msgKey)) {
      logger.debug('Skipping duplicate message:', msgKey);
      return;
    }
    recentlyProcessed.set(msgKey, now);

    // Clean up old entries (older than 10 seconds)
    for (const [key, timestamp] of recentlyProcessed) {
      if (now - timestamp > 10000) {
        recentlyProcessed.delete(key);
      }
    }

    // Determine if we should respond
    // - Always respond to mentions and DMs
    // - In channels, only respond if mentioned or in a thread we're participating in
    const isDirectMessage = event.channel_type === 'im' || event.channel.startsWith('D');
    const threadId = `${event.channel}:${event.thread_ts || event.ts}`;

    if (!isMention && !isDirectMessage && !pendingResponses.has(threadId)) {
      // Not a message we should respond to
      logger.debug('Skipping non-mention/non-DM without pending response');
      return;
    }

    messageCount++;
    logger.info(`Processing message #${messageCount} from ${event.user} in ${event.channel}`);

    // Track this thread for future responses
    pendingResponses.set(threadId, {
      channel: event.channel,
      thread_ts: event.thread_ts || event.ts
    });

    // Clean up old pending responses (keep last 100)
    if (pendingResponses.size > 100) {
      const firstKey = pendingResponses.keys().next().value;
      pendingResponses.delete(firstKey);
    }

    // Extract text - remove bot mention if present
    let text = event.text;
    if (isMention) {
      text = text.replace(/<@[A-Z0-9]+>/g, '').trim();
    }

    logger.info('Forwarding message to AgentZero:', {
      textPreview: text.substring(0, 50) + (text.length > 50 ? '...' : ''),
      channel: event.channel,
      threadId: threadId
    });

    // Forward to AgentZero
    sendInbound(text, event);

  } catch (error) {
    logger.error('Failed to handle Slack message:', error);
  }
}

/**
 * Handle capability invocation from AgentZero.
 */
async function handleCapabilityInvoke(requestId, capability, payload) {
  logger.debug('Capability invoke:', capability, payload);

  try {
    switch (capability) {
      case 'send_message': {
        const { channel, text, thread_ts, blocks } = payload;

        // Resolve channel name to ID if needed
        let channelId = channel;
        if (!channel.startsWith('C') && !channel.startsWith('D') && !channel.startsWith('G')) {
          for (const [id, name] of channelCache) {
            if (name === channel || `#${name}` === channel) {
              channelId = id;
              break;
            }
          }
        }

        const result = await slackClient.chat.postMessage({
          channel: channelId,
          text,
          thread_ts,
          blocks,
          mrkdwn: true
        });

        logger.info('Sent message to Slack:', { channel: channelId, ts: result.ts });
        sendCapabilityResponse(requestId, {
          success: true,
          ts: result.ts,
          channel: result.channel
        });
        break;
      }

      case 'send_ephemeral': {
        const { channel, user, text } = payload;

        const result = await slackClient.chat.postEphemeral({
          channel,
          user,
          text
        });

        sendCapabilityResponse(requestId, {
          success: true,
          message_ts: result.message_ts
        });
        break;
      }

      case 'add_reaction': {
        const { channel, timestamp, name } = payload;

        await slackClient.reactions.add({
          channel,
          timestamp,
          name
        });

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

  try {
    switch (resource) {
      case 'channels': {
        const result = await slackClient.conversations.list({
          types: 'public_channel,private_channel',
          limit: params?.limit || 100
        });

        sendResourceResponse(requestId, {
          channels: result.channels?.map(c => ({
            id: c.id,
            name: c.name,
            is_member: c.is_member,
            num_members: c.num_members
          })) || []
        });
        break;
      }

      case 'users': {
        const result = await slackClient.users.list();

        sendResourceResponse(requestId, {
          users: result.members
            ?.filter(m => !m.is_bot && !m.deleted)
            .map(m => ({
              id: m.id,
              name: m.name,
              real_name: m.real_name,
              display_name: m.profile?.display_name
            })) || []
        });
        break;
      }

      case 'team': {
        const result = await slackClient.team.info();

        sendResourceResponse(requestId, {
          id: result.team?.id,
          name: result.team?.name,
          domain: result.team?.domain
        });
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
 * Handle outbox item (message to send to Slack).
 */
async function handleOutboxItem(outboxId, capability, payload) {
  logger.info('Outbox item:', outboxId, capability, JSON.stringify(payload));

  try {
    // Most outbound messages use send_message, reply, or respond capability
    if (capability === 'send_message' || capability === 'reply' || capability === 'respond') {
      // Accept both 'text' and 'message' field names
      const text = payload.text || payload.message;
      const { channel, thread_ts, blocks } = payload;

      // If thread_id is provided (from inbound metadata), use it
      let targetChannel = channel;
      let targetThreadTs = thread_ts;

      // Parse thread_id format "channel:ts" if present
      if (payload.thread_id && payload.thread_id.includes(':')) {
        const [ch, ts] = payload.thread_id.split(':');
        targetChannel = ch;
        targetThreadTs = ts;
      }

      if (!targetChannel) {
        logger.error('No channel found in payload:', payload);
        sendFail(outboxId, 'No channel specified');
        return;
      }

      if (!text) {
        logger.error('No text/message found in payload:', payload);
        sendFail(outboxId, 'No text specified');
        return;
      }

      const result = await slackClient.chat.postMessage({
        channel: targetChannel,
        text,
        thread_ts: targetThreadTs,
        blocks,
        mrkdwn: true
      });

      logger.info('Sent outbox message to Slack:', { outboxId, channel: targetChannel, ts: result.ts });
      sendAck(outboxId);
    } else {
      // Unknown capability for outbox
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
  logger.info('Starting Slack plugin...');
  logger.info('Log file:', LOG_FILE);

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
    logger.info('Gateway disconnected, exiting...');
    isRunning = false;
    if (socketClient) {
      socketClient.disconnect();
    }
    process.exit(0);
  });

  // Initialize Slack connection
  await initSlack();

  // Send hello to register with gateway
  sendHello();

  logger.info('Slack plugin ready, listening for messages...');

  // Handle graceful shutdown
  process.on('SIGTERM', async () => {
    logger.info('Received SIGTERM, shutting down...');
    if (socketClient) {
      await socketClient.disconnect();
    }
    process.exit(0);
  });

  process.on('SIGINT', async () => {
    logger.info('Received SIGINT, shutting down...');
    if (socketClient) {
      await socketClient.disconnect();
    }
    process.exit(0);
  });
}

// Run main
main().catch((error) => {
  logger.error('Fatal error:', error);
  process.exit(1);
});
