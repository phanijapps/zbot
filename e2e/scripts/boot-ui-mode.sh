#!/usr/bin/env bash
# Bring up mock-gateway + UI dev server for Mode UI. Prints a JSON
# summary to stdout with the URLs, or exits non-zero on failure.
set -euo pipefail

FIXTURE="${1:-}"
if [[ -z "$FIXTURE" ]]; then
  echo "usage: boot-ui-mode.sh <fixture-name>" >&2
  exit 64
fi

REPO="$(git rev-parse --show-toplevel)"
FIXTURE_DIR="$REPO/e2e/fixtures/$FIXTURE"
if [[ ! -d "$FIXTURE_DIR" ]]; then
  echo "fixture not found: $FIXTURE_DIR" >&2
  exit 65
fi

# One shared venv for all mock servers. Gitignored via e2e/.gitignore.
VENV="$REPO/e2e/.venv"
if [[ ! -x "$VENV/bin/python" ]]; then
  python3 -m venv "$VENV" >&2
  "$VENV/bin/pip" install --quiet --upgrade pip >&2
  "$VENV/bin/pip" install --quiet \
    -r "$REPO/e2e/mock_gateway/requirements.txt" \
    -r "$REPO/e2e/mock_llm/requirements.txt" >&2
fi
PY="$VENV/bin/python"

RUN_DIR="$(mktemp -d -t zbot-e2e-ui-XXXXXXXX)"
echo "$RUN_DIR" > /tmp/zbot-e2e-latest-run-dir

pick_port() { "$PY" -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1])"; }

GATEWAY_PORT=$(pick_port)
UI_PORT=$(pick_port)

(
  cd "$REPO"
  PYTHONPATH=. "$PY" -m e2e.mock_gateway \
    --fixture "$FIXTURE_DIR" \
    --port "$GATEWAY_PORT" \
    --cadence compressed \
    > "$RUN_DIR/mock-gateway.log" 2>&1
) &
echo $! > "$RUN_DIR/mock-gateway.pid"

for _ in $(seq 1 20); do
  if curl -sf "http://127.0.0.1:$GATEWAY_PORT/api/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done
if ! curl -sf "http://127.0.0.1:$GATEWAY_PORT/api/health" >/dev/null; then
  echo "mock-gateway failed to start; log:" >&2
  cat "$RUN_DIR/mock-gateway.log" >&2
  bash "$(dirname "$0")/teardown.sh" "$RUN_DIR" || true
  exit 70
fi

if [[ ! -f "$REPO/apps/ui/dist/index.html" ]]; then
  (cd "$REPO/apps/ui" && npm run build > "$RUN_DIR/ui-build.log" 2>&1) || {
    echo "UI build failed; log tail:" >&2
    tail -30 "$RUN_DIR/ui-build.log" >&2
    bash "$(dirname "$0")/teardown.sh" "$RUN_DIR" || true
    exit 71
  }
fi
(
  cd "$REPO/apps/ui"
  npx vite preview --port "$UI_PORT" --strictPort \
    > "$RUN_DIR/ui.log" 2>&1
) &
echo $! > "$RUN_DIR/ui.pid"

for _ in $(seq 1 40); do
  if curl -sf "http://127.0.0.1:$UI_PORT/" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done
if ! curl -sf "http://127.0.0.1:$UI_PORT/" >/dev/null; then
  echo "UI preview server failed to start; log tail:" >&2
  tail -30 "$RUN_DIR/ui.log" >&2
  bash "$(dirname "$0")/teardown.sh" "$RUN_DIR" || true
  exit 71
fi

cat <<EOF
{"run_dir":"$RUN_DIR","mock_gateway_url":"http://127.0.0.1:$GATEWAY_PORT","ui_url":"http://127.0.0.1:$UI_PORT","fixture":"$FIXTURE"}
EOF
