/**
 * AgentZero Slack Notifier
 *
 * A connector that forwards agent responses to Slack channels via webhooks.
 *
 * Setup:
 *   1. Create a Slack Incoming Webhook: https://api.slack.com/messaging/webhooks
 *   2. Set SLACK_WEBHOOK_URL environment variable
 *   3. npm install && npm start
 *   4. Register as connector in AgentZero
 *
 * Environment Variables:
 *   SLACK_WEBHOOK_URL - Your Slack webhook URL (required)
 *   PORT             - Server port (default: 8081)
 *   SLACK_CHANNEL    - Override channel (optional)
 *   SLACK_USERNAME   - Bot username (default: "AgentZero")
 *   SLACK_ICON_EMOJI - Bot emoji (default: ":robot_face:")
 */

const express = require('express');
const https = require('https');
const http = require('http');
const { URL } = require('url');

const app = express();

const PORT = process.env.PORT || 8081;
const SLACK_WEBHOOK_URL = process.env.SLACK_WEBHOOK_URL;
const SLACK_CHANNEL = process.env.SLACK_CHANNEL;
const SLACK_USERNAME = process.env.SLACK_USERNAME || 'AgentZero';
const SLACK_ICON_EMOJI = process.env.SLACK_ICON_EMOJI || ':robot_face:';

app.use(express.json());

// Health check
app.get('/health', (req, res) => {
  res.json({
    status: 'ok',
    slack_configured: !!SLACK_WEBHOOK_URL,
    timestamp: new Date().toISOString()
  });
});

// HEAD request for connectivity testing (used by AgentZero connector test)
app.head('/webhook', (req, res) => {
  res.status(200).end();
});

/**
 * Send message to Slack
 */
async function sendToSlack(text, context = {}) {
  if (!SLACK_WEBHOOK_URL) {
    throw new Error('SLACK_WEBHOOK_URL not configured');
  }

  const payload = {
    username: SLACK_USERNAME,
    icon_emoji: SLACK_ICON_EMOJI,
    blocks: [
      {
        type: 'header',
        text: {
          type: 'plain_text',
          text: `Agent Response`,
          emoji: true
        }
      },
      {
        type: 'section',
        text: {
          type: 'mrkdwn',
          text: text.length > 3000 ? text.substring(0, 2997) + '...' : text
        }
      },
      {
        type: 'context',
        elements: [
          {
            type: 'mrkdwn',
            text: `*Source:* ${context.source || 'unknown'} | *Session:* \`${context.session_id || 'N/A'}\``
          }
        ]
      }
    ]
  };

  if (SLACK_CHANNEL) {
    payload.channel = SLACK_CHANNEL;
  }

  return new Promise((resolve, reject) => {
    const url = new URL(SLACK_WEBHOOK_URL);
    const client = url.protocol === 'https:' ? https : http;

    const data = JSON.stringify(payload);

    const options = {
      hostname: url.hostname,
      port: url.port || (url.protocol === 'https:' ? 443 : 80),
      path: url.pathname,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(data)
      }
    };

    const req = client.request(options, (res) => {
      let body = '';
      res.on('data', chunk => body += chunk);
      res.on('end', () => {
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve({ status: res.statusCode, body });
        } else {
          reject(new Error(`Slack returned ${res.statusCode}: ${body}`));
        }
      });
    });

    req.on('error', reject);
    req.write(data);
    req.end();
  });
}

/**
 * Main webhook endpoint
 */
app.post('/webhook', async (req, res) => {
  const { context, capability, payload } = req.body;

  // Extract message from nested payload
  const message = payload?.message;

  console.log(`[${new Date().toISOString()}] Received webhook: capability=${capability}`);

  if (!message) {
    return res.status(400).json({
      success: false,
      error: 'No message in payload'
    });
  }

  try {
    await sendToSlack(message, context);
    console.log(`[${new Date().toISOString()}] Sent to Slack successfully`);

    res.json({
      success: true,
      message: 'Sent to Slack',
      timestamp: new Date().toISOString()
    });
  } catch (error) {
    console.error(`[${new Date().toISOString()}] Slack error:`, error.message);

    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

// Start server
app.listen(PORT, () => {
  console.log(`
╔══════════════════════════════════════════════════════════╗
║        AgentZero Slack Notifier                          ║
╠══════════════════════════════════════════════════════════╣
║  Server running on http://localhost:${PORT}                 ║
║                                                          ║
║  Slack Webhook: ${SLACK_WEBHOOK_URL ? 'Configured ✓' : 'NOT CONFIGURED ✗'}                      ║
${!SLACK_WEBHOOK_URL ? '║                                                          ║\n║  Set SLACK_WEBHOOK_URL environment variable!             ║' : ''}
║                                                          ║
║  Register this connector:                                ║
║    curl -X POST http://localhost:18791/api/connectors \\ ║
║      -H "Content-Type: application/json" \\               ║
║      -d '{                                               ║
║        "id": "slack-notifier",                           ║
║        "name": "Slack Notifier",                         ║
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
