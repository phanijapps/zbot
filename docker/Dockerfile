# ===========================================================================
# z-Bot Docker Build — Multi-Stage
# Stage 1: Build frontend (Node)
# Stage 2: Build daemon binary (Rust)
# Stage 3: Runtime with tools (Debian)
# ===========================================================================

# ---------------------------------------------------------------------------
# Stage 1: Frontend Build
# ---------------------------------------------------------------------------
FROM node:20-slim AS frontend

WORKDIR /build

# Copy package files first for layer caching
COPY apps/ui/package.json apps/ui/package-lock.json apps/ui/
COPY package.json ./

# Install dependencies
RUN cd apps/ui && npm ci --ignore-scripts

# Copy UI source and build
COPY apps/ui/ apps/ui/
RUN cd apps/ui && npm run build

# ---------------------------------------------------------------------------
# Stage 2: Rust Build
# ---------------------------------------------------------------------------
FROM rust:1.88-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy Cargo files first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY apps/daemon/Cargo.toml apps/daemon/Cargo.toml
COPY apps/cli/Cargo.toml apps/cli/Cargo.toml
COPY runtime/agent-runtime/Cargo.toml runtime/agent-runtime/Cargo.toml
COPY runtime/agent-tools/Cargo.toml runtime/agent-tools/Cargo.toml
COPY runtime/zero-core/Cargo.toml runtime/zero-core/Cargo.toml
COPY services/execution-state/Cargo.toml services/execution-state/Cargo.toml
COPY services/api-logs/Cargo.toml services/api-logs/Cargo.toml
COPY gateway/Cargo.toml gateway/Cargo.toml
COPY gateway/gateway-bus/Cargo.toml gateway/gateway-bus/Cargo.toml
COPY gateway/gateway-bridge/Cargo.toml gateway/gateway-bridge/Cargo.toml
COPY gateway/gateway-connectors/Cargo.toml gateway/gateway-connectors/Cargo.toml
COPY gateway/gateway-database/Cargo.toml gateway/gateway-database/Cargo.toml
COPY gateway/gateway-events/Cargo.toml gateway/gateway-events/Cargo.toml
COPY gateway/gateway-execution/Cargo.toml gateway/gateway-execution/Cargo.toml
COPY gateway/gateway-services/Cargo.toml gateway/gateway-services/Cargo.toml
COPY gateway/gateway-ws-protocol/Cargo.toml gateway/gateway-ws-protocol/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p apps/daemon/src apps/cli/src \
    runtime/agent-runtime/src runtime/agent-tools/src runtime/zero-core/src \
    services/execution-state/src services/api-logs/src \
    gateway/src gateway/gateway-bus/src gateway/gateway-bridge/src \
    gateway/gateway-connectors/src gateway/gateway-database/src \
    gateway/gateway-events/src gateway/gateway-execution/src \
    gateway/gateway-services/src gateway/gateway-ws-protocol/src \
    && for d in apps/daemon apps/cli \
       runtime/agent-runtime runtime/agent-tools runtime/zero-core \
       services/execution-state services/api-logs \
       gateway gateway/gateway-bus gateway/gateway-bridge \
       gateway/gateway-connectors gateway/gateway-database \
       gateway/gateway-events gateway/gateway-execution \
       gateway/gateway-services gateway/gateway-ws-protocol; do \
         echo "fn main() {}" > "$d/src/main.rs" 2>/dev/null; \
         echo "" > "$d/src/lib.rs" 2>/dev/null; \
       done

# Build dependencies only (cached layer)
RUN cargo build -p daemon --release 2>/dev/null || true

# Copy actual source code
COPY . .

# Build the real binary
RUN cargo build -p daemon --release

# ---------------------------------------------------------------------------
# Stage 3: Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Core tools
    bash \
    curl \
    wget \
    ca-certificates \
    git \
    # Python
    python3 \
    python3-pip \
    python3-venv \
    # LibreOffice (document conversion)
    libreoffice-core \
    libreoffice-writer \
    # SSL for HTTPS API calls
    libssl3 \
    # Cleanup
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install pnpm via corepack
RUN corepack enable && corepack prepare pnpm@latest --activate

# Install uv (fast Python package manager)
RUN pip3 install --break-system-packages uv

# Copy daemon binary
COPY --from=builder /build/target/release/zerod /usr/local/bin/zerod

# Copy built frontend
COPY --from=frontend /build/dist /opt/zbot/ui

# Create non-root user (optional, can run as root for simplicity)
RUN mkdir -p /data/zbot /root/.cache/fastembed

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD curl -f http://localhost:18791/api/health || exit 1

EXPOSE 18791 18790

ENTRYPOINT ["/usr/local/bin/zerod"]
CMD ["--host", "0.0.0.0", "--data-dir", "/data/zbot", "--static-dir", "/opt/zbot/ui", "--log-no-stdout"]
