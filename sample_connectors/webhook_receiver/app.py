"""
AgentZero Webhook Receiver (Python/Flask)

A simple HTTP server that receives webhook callbacks from AgentZero connectors.
Use this as a starting point for building your own connectors.

Usage:
    pip install -r requirements.txt
    python app.py

The server runs on port 8080 by default (configurable via PORT env var).
"""

import os
import json
from datetime import datetime
from flask import Flask, request, jsonify

app = Flask(__name__)

PORT = int(os.environ.get('PORT', 8080))


@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({
        'status': 'ok',
        'timestamp': datetime.utcnow().isoformat() + 'Z'
    })


@app.route('/webhook', methods=['HEAD'])
def webhook_head():
    """HEAD request for connectivity testing (used by AgentZero connector test)."""
    return '', 200


@app.route('/webhook', methods=['POST'])
def webhook():
    """
    Main webhook endpoint.

    AgentZero sends POST requests here with the following payload:
    {
        "context": {
            "session_id": "sess-xxx",
            "thread_id": null,
            "agent_id": "root",
            "timestamp": "2024-01-15T09:00:00Z"
        },
        "capability": "respond",
        "payload": {
            "message": "The agent's response",
            "execution_id": "exec-xxx",
            "conversation_id": "conv-xxx"
        }
    }
    """
    data = request.get_json() or {}

    context = data.get('context', {})
    capability = data.get('capability', 'unknown')
    payload = data.get('payload', {})

    # Extract fields from nested structure
    message = payload.get('message', '')
    execution_id = payload.get('execution_id', 'N/A')
    conversation_id = payload.get('conversation_id', 'N/A')

    print('\n' + '=' * 60)
    print('WEBHOOK RECEIVED')
    print('=' * 60)
    print(f"Capability: {capability}")
    print(f"Session: {context.get('session_id', 'N/A')}")
    print(f"Execution: {execution_id}")
    print(f"Conversation: {conversation_id}")
    print(f"Agent: {context.get('agent_id', 'N/A')}")
    print(f"Timestamp: {context.get('timestamp', 'N/A')}")
    print('-' * 60)
    print('MESSAGE:')
    print(message or '(empty)')
    print('=' * 60 + '\n')

    # Process the webhook payload here
    # Examples:
    # - Store in database
    # - Forward to another service
    # - Send notification
    # - Trigger workflow

    return jsonify({
        'success': True,
        'message': 'Webhook processed successfully',
        'received_at': datetime.utcnow().isoformat() + 'Z'
    })


@app.route('/<path:path>', methods=['POST'])
def catch_all(path):
    """Catch-all for testing."""
    print(f'\nReceived POST to: /{path}')
    print(f'Body: {json.dumps(request.get_json(), indent=2)}')
    return jsonify({'success': True, 'path': f'/{path}'})


if __name__ == '__main__':
    print(f'''
╔══════════════════════════════════════════════════════════╗
║        AgentZero Webhook Receiver (Python)               ║
╠══════════════════════════════════════════════════════════╣
║  Server running on http://localhost:{PORT}                 ║
║                                                          ║
║  Endpoints:                                              ║
║    GET  /health   - Health check                         ║
║    POST /webhook  - Webhook receiver                     ║
║                                                          ║
║  Register this connector in AgentZero:                   ║
║    curl -X POST http://localhost:18791/api/connectors \\ ║
║      -H "Content-Type: application/json" \\               ║
║      -d '{{                                               ║
║        "id": "local-webhook",                            ║
║        "name": "Local Webhook",                          ║
║        "transport": {{                                    ║
║          "type": "http",                                 ║
║          "callback_url": "http://localhost:{PORT}/webhook",║
║          "method": "POST",                               ║
║          "headers": {{}}                                   ║
║        }},                                                ║
║        "enabled": true                                   ║
║      }}'                                                 ║
╚══════════════════════════════════════════════════════════╝
    ''')
    app.run(host='0.0.0.0', port=PORT, debug=True)
