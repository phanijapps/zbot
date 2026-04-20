#!/usr/bin/env bash
# Bring up mock-llm + real zerod + UI for Mode Full. Prints a JSON
# summary to stdout with URLs + run_dir.
set -euo pipefail

FIXTURE="${1:-}"
if [[ -z "$FIXTURE" ]]; then
  echo "usage: boot-full-mode.sh <fixture-name>" >&2
  exit 64
fi

REPO="$(git rev-parse --show-toplevel)"
FIXTURE_DIR="$REPO/e2e/fixtures/$FIXTURE"
if [[ ! -d "$FIXTURE_DIR" ]]; then
  echo "fixture not found: $FIXTURE_DIR" >&2
  exit 65
fi

RUN_DIR="$(mktemp -d -t zbot-e2e-full-XXXXXXXX)"
VAULT="$RUN_DIR/vault"
mkdir -p "$VAULT/data"
echo "$RUN_DIR" > /tmp/zbot-e2e-latest-run-dir

pick_port() { python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1])"; }
LLM_PORT=$(pick_port)
GATEWAY_HTTP_PORT=$(pick_port)
GATEWAY_WS_PORT=$(pick_port)
UI_PORT=$(pick_port)

cat > "$VAULT/settings.json" <<EOF
{
  "providers": [
    {
      "id": "mock",
      "name": "Mock",
      "base_url": "http://127.0.0.1:$LLM_PORT/v1",
      "api_key": "sk-mock",
      "models": ["gpt-4"]
    }
  ],
  "orchestrator": {
    "provider": "mock",
    "model": "gpt-4",
    "temperature": 0.2,
    "thinking": false
  },
  "embeddings": { "provider": "bge-small" },
  "mcps": [],
  "chat": {}
}
EOF

(
  cd "$REPO"
  PYTHONPATH=. python3 -m e2e.mock_llm \
    --fixture "$FIXTURE_DIR" \
    --port "$LLM_PORT" \
    > "$RUN_DIR/mock-llm.log" 2>&1
) &
echo $! > "$RUN_DIR/mock-llm.pid"

for _ in $(seq 1 20); do
  curl -sf "http://127.0.0.1:$LLM_PORT/health" >/dev/null 2>&1 && break
  sleep 0.5
done
if ! curl -sf "http://127.0.0.1:$LLM_PORT/health" >/dev/null; then
  echo "mock-llm failed to start; log in $RUN_DIR/mock-llm.log" >&2
  bash "$(dirname "$0")/teardown.sh" "$RUN_DIR" || true
  exit 73
fi

(
  cd "$REPO"
  ZBOT_VAULT="$VAULT" \
  ZBOT_REPLAY_DIR="$FIXTURE_DIR" \
  ZBOT_REPLAY_STRICT=1 \
  ZBOT_HTTP_PORT="$GATEWAY_HTTP_PORT" \
  ZBOT_WS_PORT="$GATEWAY_WS_PORT" \
    cargo run -q -p zerod --bin zerod \
    > "$RUN_DIR/zerod.log" 2>&1
) &
echo $! > "$RUN_DIR/zerod.pid"

for _ in $(seq 1 60); do
  curl -sf "http://127.0.0.1:$GATEWAY_HTTP_PORT/api/health" >/dev/null 2>&1 && break
  sleep 1
done
if ! curl -sf "http://127.0.0.1:$GATEWAY_HTTP_PORT/api/health" >/dev/null; then
  echo "zerod failed to start; log in $RUN_DIR/zerod.log" >&2
  bash "$(dirname "$0")/teardown.sh" "$RUN_DIR" || true
  exit 72
fi

(
  cd "$REPO/apps/ui"
  VITE_HTTP_URL="http://127.0.0.1:$GATEWAY_HTTP_PORT" \
  VITE_WS_URL="ws://127.0.0.1:$GATEWAY_WS_PORT" \
    npm run dev -- --port "$UI_PORT" \
    > "$RUN_DIR/ui.log" 2>&1
) &
echo $! > "$RUN_DIR/ui.pid"

for _ in $(seq 1 40); do
  curl -sf "http://127.0.0.1:$UI_PORT/" >/dev/null 2>&1 && break
  sleep 0.5
done

cat <<EOF
{"run_dir":"$RUN_DIR","mock_llm_url":"http://127.0.0.1:$LLM_PORT","gateway_http_url":"http://127.0.0.1:$GATEWAY_HTTP_PORT","gateway_ws_url":"ws://127.0.0.1:$GATEWAY_WS_PORT","ui_url":"http://127.0.0.1:$UI_PORT","vault":"$VAULT","fixture":"$FIXTURE"}
EOF
