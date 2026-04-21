#!/usr/bin/env bash
# Kill processes whose PID files live under the harness run dir, then
# remove the run dir itself. Safe to call twice.
set -u

RUN_DIR="${1:-}"
if [[ -z "$RUN_DIR" || ! -d "$RUN_DIR" ]]; then
  echo "teardown.sh: run dir missing or already gone: $RUN_DIR" >&2
  exit 0
fi

for pidfile in "$RUN_DIR"/*.pid; do
  [[ -f "$pidfile" ]] || continue
  pid=$(cat "$pidfile" 2>/dev/null || true)
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    for _ in 1 2; do
      sleep 1
      kill -0 "$pid" 2>/dev/null || break
    done
    kill -9 "$pid" 2>/dev/null || true
  fi
  rm -f "$pidfile"
done

rm -rf "$RUN_DIR"
echo "teardown.sh: cleaned $RUN_DIR"
