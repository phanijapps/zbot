# Retail and Consumer Daily Use Cases

AI agents for retail and daily consumer applications using AgentZero's connector architecture.

## Overview

AgentZero enables personalized AI-powered experiences for retail and daily consumer applications. By leveraging the connector architecture, you can build intelligent agents that:

- Proactively notify users about relevant events (price drops, deliveries, health insights)
- Respond to natural language queries about shopping, recipes, finances, and more
- Integrate with consumer platforms (SMS, Email, WhatsApp, Smart speakers)
- Schedule automated tasks for regular updates and summaries

### Architecture Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Consumer       │────▶│   AgentZero     │────▶│   Connectors    │
│  Trigger        │     │   Gateway       │     │   (SMS/Email/   │
│  (API/Cron/Web) │     │   :18791        │     │    WhatsApp)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              │ Agent processes request
                              │ with context from MCP tools
                              ▼
                        ┌─────────────────┐
                        │  Response to    │
                        │  respond_to[]   │
                        │  connectors     │
                        └─────────────────┘
```

---

## Use Case 1: Personal Shopping Assistant

AI-powered product recommendations based on user preferences, browsing history, and seasonal trends.

### Trigger: On-Demand Request

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "shopping-assistant",
    "message": "I need a waterproof jacket for hiking, budget under $200",
    "respond_to": ["whatsapp-bridge"],
    "metadata": {
      "user_id": "user-12345",
      "preferences": {
        "brands": ["Patagonia", "North Face", "Columbia"],
        "size": "M",
        "color_preference": "earth tones"
      }
    },
    "thread_id": "shopping-session-001",
    "external_ref": "whatsapp-msg-789"
  }'
```

### Response (to WhatsApp Bridge Connector)

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": "shopping-session-001",
    "agent_id": "shopping-assistant",
    "timestamp": "2024-01-15T14:30:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Based on your preferences, here are my top 3 recommendations:\n\n1. **Patagonia Torrentshell 3L** - $179\n   Excellent waterproofing, eco-friendly materials\n   \n2. **North Face Venture 2** - $149\n   Great value, lightweight and packable\n   \n3. **Columbia Watertight II** - $89\n   Budget-friendly, reliable performance\n\nWould you like me to check availability or compare any of these?",
    "execution_id": "exec-xyz789",
    "conversation_id": "conv-shopping-001"
  }
}
```

### Connector Configuration

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "whatsapp-bridge",
    "name": "WhatsApp Business Bridge",
    "transport": {
      "type": "http",
      "callback_url": "https://api.your-service.com/whatsapp/send",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${WHATSAPP_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'
```

---

## Use Case 2: Price Drop Alerts and Deal Notifications

Automated monitoring of product prices with notifications when targets are met.

### Trigger: Scheduled Price Check (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "price-monitor-morning",
    "name": "Morning Price Check",
    "schedule": "0 0 8 * * *",
    "message": "Check price drops for user wishlists and notify about deals over 20% off",
    "respond_to": ["push-notification-service", "email-bridge"],
    "enabled": true
  }'
```

**Note**: Schedule uses 6-field format: `sec min hour day month weekday`

### Trigger: Manual Price Alert Setup

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "price-tracker",
    "message": "Alert me when the Sony WH-1000XM5 headphones drop below $280",
    "respond_to": ["sms-gateway"],
    "metadata": {
      "user_id": "user-12345",
      "product_url": "https://amazon.com/dp/B09XS7JWHH",
      "target_price": 280,
      "currency": "USD"
    },
    "external_ref": "price-alert-setup-001"
  }'
```

### Response (Price Drop Notification)

```json
{
  "context": {
    "session_id": "sess-price-001",
    "thread_id": null,
    "agent_id": "price-tracker",
    "timestamp": "2024-01-15T08:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Price Alert! Sony WH-1000XM5 is now $249 (was $349) - 29% off! This is below your target of $280. Deal expires in 2 hours. Buy now: https://amzn.to/xyz",
    "execution_id": "exec-price-789",
    "conversation_id": "conv-price-001"
  }
}
```

### Connector Configuration (SMS Gateway)

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "sms-gateway",
    "name": "Twilio SMS Gateway",
    "transport": {
      "type": "http",
      "callback_url": "https://api.twilio.com/2010-04-01/Accounts/${TWILIO_SID}/Messages.json",
      "method": "POST",
      "headers": {
        "Authorization": "Basic ${TWILIO_AUTH}",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      "timeout_ms": 15000
    },
    "enabled": true
  }'
```

---

## Use Case 3: Order Tracking and Delivery Updates

Real-time order status updates and proactive delivery notifications.

### Trigger: Order Status Check

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "order-tracker",
    "message": "What is the status of my order #ORD-2024-78901?",
    "respond_to": ["sms-gateway"],
    "metadata": {
      "user_id": "user-12345",
      "order_id": "ORD-2024-78901",
      "carrier": "FedEx",
      "tracking_number": "794644790299"
    },
    "external_ref": "sms-inquiry-001"
  }'
```

### Trigger: Scheduled Delivery Updates (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "delivery-morning-update",
    "name": "Morning Delivery Updates",
    "schedule": "0 0 7 * * *",
    "message": "Check all pending deliveries for today and notify users of expected arrival times",
    "respond_to": ["push-notification-service"],
    "enabled": true
  }'
```

### Response (Delivery Update)

```json
{
  "context": {
    "session_id": "sess-delivery-001",
    "thread_id": null,
    "agent_id": "order-tracker",
    "timestamp": "2024-01-15T07:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Your package is out for delivery! Order #ORD-2024-78901 (Nike Air Max) will arrive today between 2-6 PM. Track live: https://track.fedex.com/794644790299",
    "execution_id": "exec-delivery-789",
    "conversation_id": "conv-delivery-001"
  }
}
```

---

## Use Case 4: Recipe Suggestions Based on Pantry Inventory

AI-powered meal planning based on available ingredients and dietary preferences.

### Trigger: Recipe Request

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "recipe-assistant",
    "message": "What can I make for dinner with chicken, broccoli, and rice?",
    "respond_to": ["alexa-skill-bridge"],
    "metadata": {
      "user_id": "user-12345",
      "dietary_restrictions": ["gluten-free"],
      "cooking_time_max": 45,
      "skill_level": "intermediate",
      "servings": 4
    },
    "external_ref": "alexa-request-001"
  }'
```

### Trigger: Weekly Meal Planning (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-meal-plan",
    "name": "Sunday Meal Planning",
    "schedule": "0 0 10 * * 0",
    "message": "Generate a weekly meal plan based on user preferences and current grocery inventory",
    "respond_to": ["email-bridge", "push-notification-service"],
    "enabled": true
  }'
```

### Response (to Alexa Skill)

```json
{
  "context": {
    "session_id": "sess-recipe-001",
    "thread_id": null,
    "agent_id": "recipe-assistant",
    "timestamp": "2024-01-15T17:30:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "I found 3 recipes you can make:\n\n1. **Teriyaki Chicken Stir-Fry** (25 min)\n   Quick and healthy with your ingredients\n   \n2. **Chicken Broccoli Rice Casserole** (40 min)\n   Comfort food, perfect for 4 servings\n   \n3. **Asian Chicken Rice Bowls** (30 min)\n   Fresh and flavorful\n\nWould you like the full recipe for any of these?",
    "execution_id": "exec-recipe-789",
    "conversation_id": "conv-recipe-001"
  }
}
```

### Connector Configuration (Alexa Skill)

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "alexa-skill-bridge",
    "name": "Alexa Smart Home Bridge",
    "transport": {
      "type": "http",
      "callback_url": "https://api.your-service.com/alexa/response",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${ALEXA_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 5000
    },
    "enabled": true
  }'
```

---

## Use Case 5: Daily News/Content Digest Personalization

Personalized content curation delivered at preferred times.

### Trigger: Morning Digest (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "morning-news-digest",
    "name": "Personalized Morning Digest",
    "schedule": "0 0 6 * * 1-5",
    "message": "Compile a personalized news digest with tech, finance, and local news. Keep it under 5 minutes reading time.",
    "respond_to": ["email-bridge"],
    "enabled": true
  }'
```

### Trigger: On-Demand Topic Deep Dive

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "content-curator",
    "message": "Give me a summary of the latest AI developments this week",
    "respond_to": ["push-notification-service"],
    "metadata": {
      "user_id": "user-12345",
      "interests": ["artificial-intelligence", "machine-learning", "startups"],
      "preferred_sources": ["TechCrunch", "Ars Technica", "MIT Technology Review"],
      "summary_length": "brief"
    },
    "external_ref": "news-request-001"
  }'
```

### Response (Email Digest)

```json
{
  "context": {
    "session_id": "sess-news-001",
    "thread_id": null,
    "agent_id": "content-curator",
    "timestamp": "2024-01-15T06:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Good morning! Here is your personalized digest:\n\n**Tech Headlines**\n- OpenAI announces GPT-5 preview\n- Apple Vision Pro ships next month\n\n**Finance**\n- Markets rally on Fed rate pause\n- Bitcoin crosses $50k milestone\n\n**Local**\n- City council approves new transit plan\n- Weekend weather: Sunny, high of 72F\n\nRead time: 4 min | Full digest: https://digest.app/u/12345",
    "execution_id": "exec-news-789",
    "conversation_id": "conv-news-001"
  }
}
```

### Connector Configuration (Email Bridge)

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "email-bridge",
    "name": "SendGrid Email Bridge",
    "transport": {
      "type": "http",
      "callback_url": "https://api.sendgrid.com/v3/mail/send",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${SENDGRID_API_KEY}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'
```

---

## Use Case 6: Fitness and Health Tracking Summaries

Daily and weekly health insights from fitness trackers and health apps.

### Trigger: Daily Health Summary (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-health-summary",
    "name": "Daily Health Summary",
    "schedule": "0 0 21 * * *",
    "message": "Generate a daily health summary including steps, sleep quality, heart rate trends, and personalized recommendations",
    "respond_to": ["push-notification-service"],
    "enabled": true
  }'
```

### Trigger: Weekly Wellness Report (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-wellness-report",
    "name": "Weekly Wellness Report",
    "schedule": "0 0 9 * * 0",
    "message": "Create a comprehensive weekly wellness report with trends, achievements, and goals for next week",
    "respond_to": ["email-bridge"],
    "enabled": true
  }'
```

### Trigger: On-Demand Health Query

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "health-assistant",
    "message": "How did I sleep this week compared to last week?",
    "respond_to": ["sms-gateway"],
    "metadata": {
      "user_id": "user-12345",
      "data_sources": ["fitbit", "apple-health"],
      "metrics": ["sleep_duration", "sleep_quality", "rem_sleep"]
    },
    "external_ref": "health-query-001"
  }'
```

### Response (Daily Health Summary)

```json
{
  "context": {
    "session_id": "sess-health-001",
    "thread_id": null,
    "agent_id": "health-assistant",
    "timestamp": "2024-01-15T21:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Daily Health Summary - Jan 15\n\nSteps: 8,247 (Goal: 10,000)\nActive Minutes: 45 min\nSleep: 7h 23m (Quality: Good)\nResting HR: 62 bpm\n\nInsight: Your sleep improved 12% vs last week. Great job on the evening walks - they seem to help!\n\nTip: Try a 15-min walk after lunch to hit your step goal.",
    "execution_id": "exec-health-789",
    "conversation_id": "conv-health-001"
  }
}
```

---

## Use Case 7: Personal Finance Alerts and Budget Tracking

Proactive financial insights and budget notifications.

### Trigger: Weekly Budget Summary (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-budget-summary",
    "name": "Weekly Budget Summary",
    "schedule": "0 0 9 * * 1",
    "message": "Analyze spending patterns from last week and provide a budget status update with category breakdowns",
    "respond_to": ["push-notification-service"],
    "enabled": true
  }'
```

### Trigger: Real-Time Budget Alert

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "finance-assistant",
    "message": "Alert: Dining budget exceeded. Analyze and notify.",
    "respond_to": ["sms-gateway"],
    "metadata": {
      "user_id": "user-12345",
      "alert_type": "budget_exceeded",
      "category": "dining",
      "budget_limit": 400,
      "current_spend": 425,
      "days_remaining": 8
    },
    "external_ref": "budget-alert-001"
  }'
```

### Trigger: Bill Reminder (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-bill-reminder",
    "name": "Daily Bill Reminder",
    "schedule": "0 0 8 * * *",
    "message": "Check for upcoming bills in the next 3 days and remind users",
    "respond_to": ["push-notification-service", "email-bridge"],
    "enabled": true
  }'
```

### Response (Budget Alert)

```json
{
  "context": {
    "session_id": "sess-finance-001",
    "thread_id": null,
    "agent_id": "finance-assistant",
    "timestamp": "2024-01-15T12:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Budget Alert: You have exceeded your $400 dining budget by $25 with 8 days left in the month.\n\nBreakdown:\n- Restaurants: $285\n- Coffee shops: $95\n- Food delivery: $45\n\nTip: Cooking at home for the next week could save you ~$150. Would you like some quick recipe ideas?",
    "execution_id": "exec-finance-789",
    "conversation_id": "conv-finance-001"
  }
}
```

---

## Use Case 8: Smart Home Automation Triggers

AI-powered smart home routines and voice-controlled automation.

### Trigger: Morning Routine (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "morning-routine",
    "name": "Morning Smart Home Routine",
    "schedule": "0 30 6 * * 1-5",
    "message": "Start morning routine: adjust thermostat, turn on lights gradually, start coffee maker, and provide weather briefing",
    "respond_to": ["home-assistant-bridge", "alexa-skill-bridge"],
    "enabled": true
  }'
```

### Trigger: Voice Command (via Smart Speaker)

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "home-assistant",
    "message": "Set the house to movie mode",
    "respond_to": ["home-assistant-bridge", "alexa-skill-bridge"],
    "metadata": {
      "user_id": "user-12345",
      "location": "living_room",
      "time_of_day": "evening",
      "available_devices": ["lights", "thermostat", "tv", "blinds", "speakers"]
    },
    "external_ref": "voice-cmd-001"
  }'
```

### Trigger: Energy Optimization Alert

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "home-assistant",
    "message": "Optimize home energy usage - peak pricing starting in 30 minutes",
    "respond_to": ["push-notification-service", "home-assistant-bridge"],
    "metadata": {
      "user_id": "user-12345",
      "current_rate": 0.12,
      "peak_rate": 0.28,
      "peak_start": "16:00",
      "peak_end": "21:00"
    },
    "external_ref": "energy-alert-001"
  }'
```

### Response (Smart Home Action)

```json
{
  "context": {
    "session_id": "sess-home-001",
    "thread_id": null,
    "agent_id": "home-assistant",
    "timestamp": "2024-01-15T19:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Movie mode activated:\n\n- Living room lights dimmed to 15%\n- Blinds closed\n- Thermostat set to 72F\n- TV powered on, switching to streaming input\n- Surround sound enabled\n\nEnjoy your movie! Say 'pause movie mode' to return to normal.",
    "execution_id": "exec-home-789",
    "conversation_id": "conv-home-001"
  }
}
```

### Connector Configuration (Home Assistant Bridge)

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "home-assistant-bridge",
    "name": "Home Assistant Bridge",
    "transport": {
      "type": "http",
      "callback_url": "http://homeassistant.local:8123/api/webhook/agentzero",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${HA_LONG_LIVED_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 10000
    },
    "enabled": true
  }'
```

---

## Integration Patterns

### Pattern 1: SMS Integration (Twilio)

Receive user messages via SMS webhook and respond through AgentZero.

**Incoming SMS Handler (Your Service)**

```javascript
// Express.js webhook handler for incoming SMS
app.post('/sms/incoming', async (req, res) => {
  const { From, Body } = req.body;

  // Forward to AgentZero
  await fetch('http://localhost:18791/api/gateway/submit', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      agent_id: 'consumer-assistant',
      message: Body,
      respond_to: ['sms-gateway'],
      metadata: { phone_number: From },
      external_ref: `sms-${Date.now()}`
    })
  });

  res.status(200).send('OK');
});
```

**SMS Connector Response Handler**

```javascript
// Handle AgentZero response and send SMS
app.post('/sms/send', async (req, res) => {
  const { context, payload } = req.body;
  const phoneNumber = context.metadata?.phone_number;

  await twilioClient.messages.create({
    body: payload.message,
    to: phoneNumber,
    from: process.env.TWILIO_NUMBER
  });

  res.json({ success: true });
});
```

### Pattern 2: Email Integration (SendGrid)

**Email Connector Adapter**

```javascript
// Transform AgentZero response to SendGrid format
app.post('/email/send', async (req, res) => {
  const { context, payload } = req.body;

  await fetch('https://api.sendgrid.com/v3/mail/send', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${process.env.SENDGRID_API_KEY}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      personalizations: [{
        to: [{ email: context.metadata?.user_email }]
      }],
      from: { email: 'agent@your-app.com', name: 'AgentZero' },
      subject: `Update from ${context.agent_id}`,
      content: [{
        type: 'text/plain',
        value: payload.message
      }]
    })
  });

  res.json({ success: true });
});
```

### Pattern 3: WhatsApp Business API

**WhatsApp Connector**

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "whatsapp-business",
    "name": "WhatsApp Business API",
    "transport": {
      "type": "http",
      "callback_url": "https://graph.facebook.com/v18.0/${PHONE_NUMBER_ID}/messages",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${WHATSAPP_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'
```

**WhatsApp Message Adapter**

```javascript
app.post('/whatsapp/send', async (req, res) => {
  const { context, payload } = req.body;

  await fetch(`https://graph.facebook.com/v18.0/${process.env.PHONE_NUMBER_ID}/messages`, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${process.env.WHATSAPP_TOKEN}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      messaging_product: 'whatsapp',
      recipient_type: 'individual',
      to: context.metadata?.whatsapp_number,
      type: 'text',
      text: { body: payload.message }
    })
  });

  res.json({ success: true });
});
```

### Pattern 4: Smart Speaker Integration (Alexa/Google Home)

**Alexa Skill Backend**

```javascript
// Alexa skill intent handler
const LaunchRequestHandler = {
  canHandle(handlerInput) {
    return handlerInput.requestEnvelope.request.type === 'LaunchRequest';
  },
  async handle(handlerInput) {
    // Get user query from Alexa
    const userQuery = handlerInput.requestEnvelope.request.intent?.slots?.query?.value;

    // Forward to AgentZero
    const response = await fetch('http://localhost:18791/api/gateway/submit', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        agent_id: 'voice-assistant',
        message: userQuery,
        respond_to: [], // Synchronous response
        metadata: { alexa_user_id: handlerInput.requestEnvelope.session.user.userId }
      })
    });

    // Return speech response
    return handlerInput.responseBuilder
      .speak(response.payload.message)
      .getResponse();
  }
};
```

---

## Complete Connector Configurations

### All Connectors Setup Script

```bash
#!/bin/bash
# setup-consumer-connectors.sh

AGENTZERO_URL="http://localhost:18791"

# 1. SMS Gateway (Twilio)
curl -X POST ${AGENTZERO_URL}/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "sms-gateway",
    "name": "Twilio SMS Gateway",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/sms/send",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${SMS_SERVICE_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 15000
    },
    "enabled": true
  }'

# 2. Email Bridge (SendGrid)
curl -X POST ${AGENTZERO_URL}/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "email-bridge",
    "name": "SendGrid Email Bridge",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/email/send",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${EMAIL_SERVICE_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'

# 3. WhatsApp Bridge
curl -X POST ${AGENTZERO_URL}/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "whatsapp-bridge",
    "name": "WhatsApp Business Bridge",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/whatsapp/send",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${WHATSAPP_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'

# 4. Push Notification Service
curl -X POST ${AGENTZERO_URL}/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "push-notification-service",
    "name": "Firebase Push Notifications",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/push/send",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${FCM_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 10000
    },
    "enabled": true
  }'

# 5. Alexa Skill Bridge
curl -X POST ${AGENTZERO_URL}/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "alexa-skill-bridge",
    "name": "Alexa Smart Home Bridge",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/alexa/response",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${ALEXA_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 5000
    },
    "enabled": true
  }'

# 6. Home Assistant Bridge
curl -X POST ${AGENTZERO_URL}/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "home-assistant-bridge",
    "name": "Home Assistant Bridge",
    "transport": {
      "type": "http",
      "callback_url": "http://homeassistant.local:8123/api/webhook/agentzero",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${HA_TOKEN}",
        "Content-Type": "application/json"
      },
      "timeout_ms": 10000
    },
    "enabled": true
  }'

echo "All connectors configured!"
```

---

## Cron Schedule Quick Reference

| Schedule | Cron Expression | Description |
|----------|-----------------|-------------|
| Every minute | `0 * * * * *` | Testing/debugging |
| Every hour | `0 0 * * * *` | Price checks, status updates |
| Daily 6 AM | `0 0 6 * * *` | Morning routines |
| Daily 9 PM | `0 0 21 * * *` | Evening summaries |
| Weekdays 8 AM | `0 0 8 * * 1-5` | Workday notifications |
| Sunday 10 AM | `0 0 10 * * 0` | Weekly planning |
| Monthly 1st at noon | `0 0 12 1 * *` | Monthly reports |

**Note**: AgentZero uses 6-field cron format: `sec min hour day month weekday`

---

## Best Practices

### 1. Personalization

- Always include `user_id` in metadata for personalized responses
- Store user preferences and use them in agent context
- Track conversation threads with `thread_id` for continuity

### 2. Response Optimization

- Keep messages concise for SMS (160 char limit)
- Use rich formatting for email/WhatsApp
- Provide actionable links when relevant

### 3. Error Handling

- Configure appropriate timeouts per connector type
- Implement retry logic in your connector services
- Log all AgentZero responses for debugging

### 4. Rate Limiting

- Respect platform rate limits (SMS, WhatsApp, etc.)
- Use batching for bulk notifications
- Implement queuing for high-volume scenarios

### 5. Security

- Always use HTTPS in production
- Rotate API tokens regularly
- Validate incoming webhook payloads
- Never log sensitive user data

---

## Troubleshooting

### Common Issues

**Agent not responding?**
```bash
# Check session status
curl http://localhost:18791/api/gateway/status/{session_id}
```

**Connector not receiving responses?**
```bash
# Test connector connectivity
curl -X POST http://localhost:18791/api/connectors/{connector_id}/test
```

**Cron job not triggering?**
```bash
# List all cron jobs and check status
curl http://localhost:18791/api/cron

# Manually trigger for testing
curl -X POST http://localhost:18791/api/cron/{job_id}/trigger
```

**Check connector status?**
```bash
# Get connector details
curl http://localhost:18791/api/connectors/{connector_id}
```
