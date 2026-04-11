# ===========================================================================
# GitHub Actions Self-Hosted Runner
# Pre-installed: Rust, Node 20, cargo-llvm-cov, build deps
# ===========================================================================

FROM ubuntu:22.04

ARG RUNNER_VERSION=2.322.0
ARG TARGETARCH=x64

ENV DEBIAN_FRONTEND=noninteractive
ENV RUNNER_ALLOW_RUNASROOT=false

# ---------------------------------------------------------------------------
# System dependencies
# ---------------------------------------------------------------------------
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    wget \
    git \
    jq \
    ca-certificates \
    gnupg \
    lsb-release \
    unzip \
    zip \
    # Rust build deps
    pkg-config \
    libssl-dev \
    make \
    gcc \
    g++ \
    libc6-dev \
    libstdc++-12-dev \
    libonig-dev \
    # Python (for scripts)
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Node.js 20
# ---------------------------------------------------------------------------
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Create runner user
# ---------------------------------------------------------------------------
RUN useradd -m -s /bin/bash runner \
    && mkdir -p /home/runner/work \
    && chown -R runner:runner /home/runner

# ---------------------------------------------------------------------------
# Install Rust (as runner user)
# ---------------------------------------------------------------------------
USER runner
WORKDIR /home/runner

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- \
    --default-toolchain stable \
    --component llvm-tools-preview \
    --profile minimal \
    -y

ENV PATH="/home/runner/.cargo/bin:${PATH}"

# Install cargo-llvm-cov for coverage reports
RUN cargo install cargo-llvm-cov --locked

# ---------------------------------------------------------------------------
# Install GitHub Actions Runner
# ---------------------------------------------------------------------------
RUN mkdir -p /home/runner/actions-runner \
    && cd /home/runner/actions-runner \
    && curl -o actions-runner.tar.gz -L \
       "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-${TARGETARCH}-${RUNNER_VERSION}.tar.gz" \
    && tar xzf actions-runner.tar.gz \
    && rm actions-runner.tar.gz

# Install runner dependencies (as root, then switch back)
USER root
RUN cd /home/runner/actions-runner && ./bin/installdependencies.sh
USER runner

# ---------------------------------------------------------------------------
# Entrypoint
# ---------------------------------------------------------------------------
COPY --chown=runner:runner start.sh /home/runner/start.sh
RUN chmod +x /home/runner/start.sh

WORKDIR /home/runner/actions-runner
ENTRYPOINT ["/home/runner/start.sh"]
