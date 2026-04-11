# Self-Hosted GitHub Actions Runner

Run GitHub Actions CI on your local machine via Docker. Pre-installed Rust, Node, and all build dependencies — CI runs in minutes instead of 15-20 on cloud runners.

## Quick Start

### 1. Get a Runner Token

Go to **github.com/phanijapps/zbot** → **Settings** → **Actions** → **Runners** → **New self-hosted runner**

Copy the token from the `./config.sh --token <TOKEN>` command shown on that page.

### 2. Start the Runner

```bash
cd docker/runner
RUNNER_TOKEN=<your-token> docker compose up -d --build
```

First build takes ~10 minutes (Rust toolchain + cargo-llvm-cov). Subsequent starts are instant.

### 3. Verify

Check the runner is registered:
- GitHub repo → Settings → Actions → Runners → should show `zbot-local` as "Idle"

Push a commit or open a PR — the runner picks up the job automatically.

## What's Pre-Installed

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | stable | Compile, test, clippy, fmt |
| cargo-llvm-cov | latest | LCOV coverage reports |
| llvm-tools-preview | stable | Coverage instrumentation |
| Node.js | 20 | Frontend build + tests |
| gcc/g++ | system | jemalloc, oniguruma compilation |
| libssl-dev | system | TLS for HTTP clients |
| libonig-dev | system | Oniguruma regex |
| python3 | system | Python scripts |
| git | system | Checkout |

## Commands

```bash
# Start
RUNNER_TOKEN=<token> docker compose up -d --build

# View logs
docker compose logs -f

# Stop (deregisters from GitHub)
docker compose down

# Rebuild (after Rust version update)
docker compose build --no-cache
```

## Persistent Volumes

| Volume | Purpose |
|--------|---------|
| `runner-work` | Runner workspace (checked out repos, build artifacts) |
| `cargo-cache` | Cargo registry cache (crate downloads) |
| `cargo-git` | Cargo git index cache |

These persist across container restarts. Clear with `docker compose down -v`.

## Token Expiry

The runner token from GitHub is short-lived. If the runner can't connect:

1. Go to GitHub → Settings → Actions → Runners → New self-hosted runner
2. Copy the new token
3. Restart: `docker compose down && RUNNER_TOKEN=<new-token> docker compose up -d`

## Workflow Labels

The runner registers with labels: `self-hosted, linux, x64, zbot`

Workflows targeting this runner use:
```yaml
runs-on: [self-hosted, zbot]
```
