#!/usr/bin/env bash
# ===========================================================================
# GitHub Actions Runner Entrypoint
# Configures and starts the runner. Handles graceful shutdown.
# ===========================================================================
set -e

RUNNER_DIR="/home/runner/actions-runner"
cd "$RUNNER_DIR"

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

if [ -z "$RUNNER_TOKEN" ]; then
    echo "ERROR: RUNNER_TOKEN is required"
    echo "Get it from: GitHub repo > Settings > Actions > Runners > New self-hosted runner"
    exit 1
fi

if [ -z "$RUNNER_REPOSITORY_URL" ]; then
    RUNNER_REPOSITORY_URL="https://github.com/phanijapps/zbot"
fi

RUNNER_NAME="${RUNNER_NAME:-zbot-local}"
RUNNER_LABELS="${RUNNER_LABELS:-self-hosted,linux,x64,zbot}"
RUNNER_WORKDIR="${RUNNER_WORKDIR:-/home/runner/work}"

# Check if already configured
if [ ! -f ".runner" ]; then
    echo "Configuring runner: ${RUNNER_NAME}"
    ./config.sh \
        --url "${RUNNER_REPOSITORY_URL}" \
        --token "${RUNNER_TOKEN}" \
        --name "${RUNNER_NAME}" \
        --labels "${RUNNER_LABELS}" \
        --work "${RUNNER_WORKDIR}" \
        --unattended \
        --replace
fi

# ---------------------------------------------------------------------------
# Graceful shutdown
# ---------------------------------------------------------------------------

cleanup() {
    echo "Shutting down runner..."
    ./config.sh remove --token "${RUNNER_TOKEN}" 2>/dev/null || true
}

trap cleanup SIGTERM SIGINT

# ---------------------------------------------------------------------------
# Start
# ---------------------------------------------------------------------------

echo "Starting GitHub Actions runner: ${RUNNER_NAME}"
echo "  Repository: ${RUNNER_REPOSITORY_URL}"
echo "  Labels: ${RUNNER_LABELS}"
echo "  Work dir: ${RUNNER_WORKDIR}"
echo ""

./run.sh &
wait $!
