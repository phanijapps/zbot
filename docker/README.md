# z-Bot Docker

Run z-Bot in Docker with a single command. The agent runs in a sandboxed container with access only to your vault directory.

## Quick Start

```bash
cd docker/
./start.sh
```

First build takes ~10 minutes (Rust compilation). Subsequent starts are instant.

Open **http://localhost:18791** and add your LLM provider in Settings.

## Manual Setup

```bash
cd docker/
cp .env.example .env
# Edit .env — set VAULT_PATH to your preferred location
docker compose up -d --build
```

## Configuration

Edit `docker/.env`:

| Variable | Default | Description |
|----------|---------|-------------|
| `VAULT_PATH` | `~/Documents/zbot` | Host directory for all z-Bot data |
| `HTTP_PORT` | `18791` | Web UI + HTTP API + `/ws` WebSocket upgrade |
| `WS_PORT` | `18790` | Legacy standalone WebSocket bind, off by default — flip `legacy_ws_port_enabled: true` only if you have integrations that hardcode `ws://host:18790` and haven't migrated to the unified `/ws` route |

## What's in the Container

| Tool | Version | Purpose |
|------|---------|---------|
| python3 | System | Agent scripts, Python wards |
| uv | Latest | Fast Python package management |
| node | 20 | Agent scripts, Node wards |
| pnpm | Latest | Fast Node package management |
| libreoffice | System | Document conversion (soffice) |
| git | System | Version control in wards |

## Vault Directory

The vault (`VAULT_PATH`) is bind-mounted into the container. The daemon creates the full directory structure on first run:

```
zbot/
├── config/          # providers.json, settings.json, mcps.json, cron_jobs.json, …
├── data/            # conversations.db, knowledge.db
├── agents/          # Agent configurations
├── agents_data/     # Per-agent runtime data + shared memory
├── skills/          # Skill definitions
├── wards/           # Persistent project directories
├── plugins/         # Bridge workers
├── logs/            # Execution logs
└── temp/            # Ephemeral scratch (auto-wiped)
```

## Embedding Models

ONNX embedding models (~100MB) are downloaded on first use and cached in a Docker volume (`fastembed-cache`). They persist across container restarts.

## Sandbox

The container can only access:
- The mounted vault directory
- The fastembed cache volume
- Network (for LLM API calls)

It **cannot** access any other host files, processes, or resources.

## Commands

```bash
# Start
docker compose up -d

# Stop
docker compose down

# View logs
docker compose logs -f

# Rebuild after code changes
docker compose up -d --build

# Full reset (removes embedding cache)
docker compose down -v
```
