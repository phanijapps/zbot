# Docker Packaging — Design Spec

## Problem

z-Bot currently requires Rust, Node.js, and Python installed on the host to build and run. There's no simple way for a layman to get it running. Docker provides both easy deployment and sandboxing — the agent can only access the mounted vault directory, nothing else on the host.

## Solution

A `docker/` directory with Dockerfile, docker-compose.yml, .env.example, and a start.sh script. One command to build + run. Vault lives on host filesystem (bind-mounted). Embedding model cache persists via named volume.

## Directory Structure

```
docker/
├── Dockerfile          # Multi-stage: build binary + frontend, runtime with tools
├── docker-compose.yml  # One-command launch
├── .env.example        # Configurable vault path and ports
├── start.sh            # Interactive launcher for first-time users
└── README.md           # Quick start instructions
```

## Dockerfile — Multi-Stage Build

### Stage 1: Frontend Build (`node:20-slim`)
- Install pnpm
- Copy `apps/ui/` and `package.json`
- Run `pnpm install && pnpm run build`
- Output: `dist/` directory

### Stage 2: Rust Build (`rust:1.88-slim-bookworm`)
- Install build dependencies (pkg-config, libssl-dev, etc.)
- Copy full workspace
- Run `cargo build -p daemon --release`
- Output: `target/release/zerod`

### Stage 3: Runtime (`debian:bookworm-slim`)
- Install runtime tools:
  - python3, python3-pip, python3-venv
  - uv (via pip or install script)
  - node 20, pnpm (via corepack)
  - libreoffice-core (soffice)
  - git, curl, bash, ca-certificates
- Copy `zerod` binary from Stage 2
- Copy `dist/` from Stage 1 to `/opt/zbot/ui/`
- Set entrypoint to `zerod`

### Layer Caching Strategy
- `Cargo.toml` and `Cargo.lock` copied before source code — dependency layer cached
- `package.json` and `pnpm-lock.yaml` copied before UI source — npm dependency layer cached
- Source code changes only rebuild the final compilation step

## docker-compose.yml

```yaml
services:
  zbot:
    build:
      context: ..
      dockerfile: docker/Dockerfile
    ports:
      - "${HTTP_PORT:-8080}:18791"
      - "${WS_PORT:-8090}:18790"
    volumes:
      - ${VAULT_PATH:-~/Documents/zbot}:/data/zbot
      - fastembed-cache:/root/.cache/fastembed
    environment:
      - RUST_LOG=warn
    command: >
      /usr/local/bin/zerod
        --host 0.0.0.0
        --data-dir /data/zbot
        --static-dir /opt/zbot/ui
        --log-no-stdout
    restart: unless-stopped

volumes:
  fastembed-cache:
```

### Key Config

| Setting | Default | Description |
|---------|---------|-------------|
| `VAULT_PATH` | `~/Documents/zbot` | Host path to vault (agents, wards, config, data) |
| `HTTP_PORT` | `8080` | Host port for web UI + HTTP API |
| `WS_PORT` | `8090` | Host port for WebSocket streaming |

### Volumes
- **Vault:** Bind-mount from host → `/data/zbot` in container. Daemon auto-creates subdirectories on first run.
- **fastembed-cache:** Named Docker volume for ONNX embedding models. Persists across container restarts. ~100MB for default model, downloaded on first embedding call.

### Logging
- `--log-no-stdout` suppresses console output (quiet mode)
- `RUST_LOG=warn` filters Rust tracing to warnings only
- File logs written to `{vault}/logs/` (accessible from host via bind mount)

## .env.example

```env
# Host path to your z-Bot vault (agents, wards, config, data)
# This directory is created automatically on first run
VAULT_PATH=~/Documents/zbot

# Ports exposed on host
HTTP_PORT=8080
WS_PORT=8090
```

## start.sh

Interactive launcher for first-time users:

1. Check Docker and docker compose are installed
2. If `.env` doesn't exist, copy from `.env.example`
3. Prompt user for vault path (or accept default)
4. Write vault path to `.env`
5. Run `docker compose up -d --build`
6. Wait for container to be healthy
7. Print the URL: `z-Bot is running at http://localhost:8080`

On subsequent runs, skips the prompt and just starts.

## docker/README.md

Short quick-start:
```
## Quick Start

1. cd docker/
2. ./start.sh

Or manually:
1. cp .env.example .env
2. Edit .env (set VAULT_PATH if needed)
3. docker compose up -d --build
4. Open http://localhost:8080

First build takes ~10 minutes (Rust compilation).
Subsequent starts are instant.
```

## Embedding Models in Docker

ONNX models (fastembed) are downloaded to `/root/.cache/fastembed/` inside the container on first embedding call. The `fastembed-cache` named volume ensures:
- Models persist across container restarts
- No re-download after `docker compose down && up`
- Volume can be pruned with `docker volume rm` if needed

The default model (`all-MiniLM-L6-v2`) is ~100MB and requires internet on first run. No special configuration needed — it just works.

## Sandbox Properties

The Docker container can only access:
- The mounted vault directory (`VAULT_PATH` → `/data/zbot`)
- The fastembed cache volume
- Network access (for LLM API calls and model downloads)

It cannot access any other host filesystem, processes, or resources. This is intentional — Docker acts as a sandbox for agent execution.

## Runtime Tools in Container

| Tool | Purpose | Install Method |
|------|---------|---------------|
| python3 | Agent scripts, Python wards | apt |
| uv | Fast Python package management | pip/install script |
| node 20 | Agent scripts, Node wards | nodesource apt |
| pnpm | Fast Node package management | corepack |
| libreoffice-core | Document conversion (soffice) | apt |
| git | Version control in wards | apt |
| curl | Web requests | apt |
| bash | Shell tool execution | apt |

## Files to Create

| File | Description |
|------|-------------|
| `docker/Dockerfile` | Multi-stage build (node → rust → runtime) |
| `docker/docker-compose.yml` | Service definition with volumes and ports |
| `docker/.env.example` | Default configuration |
| `docker/start.sh` | Interactive launcher script |
| `docker/README.md` | Quick start for Docker users |
| `.dockerignore` | Exclude target/, node_modules/, .git, etc. |
