#!/usr/bin/env bash
# ===========================================================================
# GitHub Actions Runner — Setup & Start
# ===========================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo ""
echo -e "${GREEN}  ╔══════════════════════════════════════╗${NC}"
echo -e "${GREEN}  ║  GitHub Actions Self-Hosted Runner    ║${NC}"
echo -e "${GREEN}  ║  z-Bot CI/CD                          ║${NC}"
echo -e "${GREEN}  ╚══════════════════════════════════════╝${NC}"
echo ""

# Check Docker
if ! command -v docker &>/dev/null; then
    echo -e "${RED}Error: Docker is not installed.${NC}" >&2
    exit 1
fi

if ! docker compose version &>/dev/null; then
    echo -e "${RED}Error: docker compose is not available.${NC}" >&2
    exit 1
fi

# Setup .env on first run
if [[ ! -f .env ]]; then
    echo -e "${YELLOW}First run — setting up configuration...${NC}"
    echo ""

    if [[ -z "$RUNNER_TOKEN" ]]; then
        echo "Get your runner token from:"
        echo "  GitHub > phanijapps/zbot > Settings > Actions > Runners > New self-hosted runner"
        echo ""
        read -rp "Runner token: " RUNNER_TOKEN
    fi

    if [[ -z "$RUNNER_TOKEN" ]]; then
        echo -e "${RED}Error: Runner token is required.${NC}" >&2
        exit 1
    fi

    cp .env.example .env
    sed -i "s|^RUNNER_TOKEN=.*|RUNNER_TOKEN=${RUNNER_TOKEN}|" .env

    echo ""
    echo -e "${GREEN}Configuration saved to .env${NC}"
fi

# Source .env
source .env

if [[ -z "$RUNNER_TOKEN" ]]; then
    echo -e "${RED}Error: RUNNER_TOKEN is empty in .env${NC}" >&2
    echo "Edit docker/runner/.env and set your token."
    exit 1
fi

# Build and start
echo -e "${YELLOW}Building runner image (first time takes ~10 minutes)...${NC}"
echo ""

docker compose up -d --build

echo ""
echo -e "${GREEN}Runner is running!${NC}"
echo ""
echo -e "  Name:    ${GREEN}${RUNNER_NAME:-zbot-local}${NC}"
echo -e "  Labels:  ${RUNNER_LABELS:-self-hosted,linux,x64,zbot}"
echo -e "  Repo:    ${RUNNER_REPOSITORY_URL:-https://github.com/phanijapps/zbot}"
echo ""
echo -e "  Verify:  GitHub > Settings > Actions > Runners"
echo ""
echo -e "  Logs:    ${YELLOW}docker compose logs -f${NC}"
echo -e "  Stop:    ${YELLOW}docker compose down${NC}"
echo -e "  Rebuild: ${YELLOW}docker compose build --no-cache${NC}"
echo ""
