# Publishing AgentZero

This document describes how to build and publish AgentZero for Windows, macOS, and Linux.

## Overview

AgentZero consists of two distributable components:

| Component | Binary Name | Description |
|-----------|-------------|-------------|
| **Daemon** | `zerod` | HTTP/WebSocket server + static files |
| **CLI** | `zero` | Terminal UI client |

The web UI is embedded in the daemon binary at compile time.

---

## Prerequisites

### All Platforms

- **Rust** 1.75+ (`rustup` recommended)
- **Node.js** 20+ and **npm**
- **Git**

### Platform-Specific

| Platform | Requirements |
|----------|--------------|
| **Windows** | Visual Studio Build Tools (MSVC), Windows SDK |
| **macOS** | Xcode Command Line Tools (`xcode-select --install`) |
| **Linux** | `build-essential`, `pkg-config`, `libssl-dev` |

---

## Build Process

### 1. Build Frontend

```bash
cd apps/ui
npm install
npm run build
```

This creates `apps/ui/dist/` with static files.

### 2. Build Backend (Debug)

```bash
# Build daemon (includes embedded static files)
cargo build -p daemon

# Build CLI
cargo build -p cli
```

### 3. Build Backend (Release)

```bash
# Optimized build
cargo build --release -p daemon -p cli
```

Binaries are output to:
- `target/release/zerod` (or `zerod.exe` on Windows)
- `target/release/zero` (or `zero.exe` on Windows)

---

## Cross-Compilation

For building platform binaries from a single machine, use cross-compilation.

### Linux Targets

```bash
# Add targets
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu

# Build
cargo build --release -p daemon -p cli --target x86_64-unknown-linux-gnu
cargo build --release -p daemon -p cli --target aarch64-unknown-linux-gnu
```

### macOS Targets

```bash
# Add targets (must build on macOS)
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Intel Mac
cargo build --release -p daemon -p cli --target x86_64-apple-darwin

# Apple Silicon
cargo build --release -p daemon -p cli --target aarch64-apple-darwin

# Universal binary (combine both)
lipo -create \
  target/x86_64-apple-darwin/release/zerod \
  target/aarch64-apple-darwin/release/zerod \
  -output target/universal/release/zerod
```

### Windows Targets

From Linux or macOS (requires `mingw`):

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release -p daemon -p cli --target x86_64-pc-windows-gnu
```

For MSVC builds (recommended), you must build on Windows.

---

## Release Artifacts

### Directory Structure

```
dist/
├── agentzero-{version}-linux-x86_64.tar.gz
├── agentzero-{version}-linux-aarch64.tar.gz
├── agentzero-{version}-macos-x86_64.tar.gz
├── agentzero-{version}-macos-aarch64.tar.gz
├── agentzero-{version}-windows-x86_64.zip
└── checksums.sha256
```

### Package Contents

Each archive contains:
```
agentzero-{version}/
├── zerod              # Daemon binary
├── zero               # CLI binary
├── README.md
├── LICENSE
└── VERSION
```

### Manual Packaging Script

```bash
#!/bin/bash
# scripts/package.sh

VERSION=${1:-"0.1.0"}
DIST_DIR="dist/release"
mkdir -p "$DIST_DIR"

# Package for current platform
build_package() {
    local target=$1
    local ext=$2
    local archive_name="agentzero-${VERSION}-${target}"

    mkdir -p "$DIST_DIR/$archive_name"

    # Copy binaries
    cp target/release/zerod "$DIST_DIR/$archive_name/"
    cp target/release/zero "$DIST_DIR/$archive_name/"

    # Copy docs
    cp README.md "$DIST_DIR/$archive_name/"
    cp LICENSE "$DIST_DIR/$archive_name/"
    echo "$VERSION" > "$DIST_DIR/$archive_name/VERSION"

    # Create archive
    if [ "$ext" = "zip" ]; then
        cd "$DIST_DIR" && zip -r "${archive_name}.zip" "$archive_name"
    else
        tar -czf "$DIST_DIR/${archive_name}.tar.gz" -C "$DIST_DIR" "$archive_name"
    fi
}

# Detect platform and package
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)   build_package "linux-x86_64" "tar.gz" ;;
    Linux-aarch64)  build_package "linux-aarch64" "tar.gz" ;;
    Darwin-x86_64)  build_package "macos-x86_64" "tar.gz" ;;
    Darwin-arm64)   build_package "macos-aarch64" "tar.gz" ;;
    MINGW*-x86_64)  build_package "windows-x86_64" "zip" ;;
esac

# Generate checksums
cd "$DIST_DIR" && sha256sum *.tar.gz *.zip > checksums.sha256
```

---

## GitHub Actions CI/CD

### Release Workflow

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:
    inputs:
      version:
        description: 'Version (e.g., 0.1.0)'
        required: true

env:
  CARGO_TERM_COLOR: always

jobs:
  build-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: apps/ui/package-lock.json

      - name: Install dependencies
        run: cd apps/ui && npm ci

      - name: Build
        run: cd apps/ui && npm run build

      - name: Upload dist
        uses: actions/upload-artifact@v4
        with:
          name: frontend-dist
          path: apps/ui/dist/

  build-linux:
    needs: build-frontend
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu]
    steps:
      - uses: actions/checkout@v4

      - name: Download frontend
        uses: actions/download-artifact@v4
        with:
          name: frontend-dist
          path: apps/ui/dist/

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Build
        run: cargo build --release -p daemon -p cli --target ${{ matrix.target }}

      - name: Package
        run: |
          mkdir -p release
          cp target/${{ matrix.target }}/release/zerod release/
          cp target/${{ matrix.target }}/release/zero release/
          chmod +x release/*

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: linux-${{ matrix.target }}
          path: release/

  build-macos:
    needs: build-frontend
    runs-on: macos-latest
    strategy:
      matrix:
        target: [x86_64-apple-darwin, aarch64-apple-darwin]
    steps:
      - uses: actions/checkout@v4

      - name: Download frontend
        uses: actions/download-artifact@v4
        with:
          name: frontend-dist
          path: apps/ui/dist/

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Build
        run: cargo build --release -p daemon -p cli --target ${{ matrix.target }}

      - name: Package
        run: |
          mkdir -p release
          cp target/${{ matrix.target }}/release/zerod release/
          cp target/${{ matrix.target }}/release/zero release/
          chmod +x release/*

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: macos-${{ matrix.target }}
          path: release/

  build-windows:
    needs: build-frontend
    runs-on: windows-latest
    strategy:
      matrix:
        target: [x86_64-pc-windows-msvc]
    steps:
      - uses: actions/checkout@v4

      - name: Download frontend
        uses: actions/download-artifact@v4
        with:
          name: frontend-dist
          path: apps/ui/dist/

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Build
        run: cargo build --release -p daemon -p cli --target ${{ matrix.target }}

      - name: Package
        run: |
          mkdir release
          copy target\${{ matrix.target }}\release\zerod.exe release\
          copy target\${{ matrix.target }}\release\zero.exe release\

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: windows-${{ matrix.target }}
          path: release/

  create-release:
    needs: [build-linux, build-macos, build-windows]
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts/

      - name: Prepare release
        run: |
          VERSION=${{ github.event.inputs.version || github.ref_name }}
          mkdir -p release

          # Linux x86_64
          tar -czvf release/agentzero-${VERSION}-linux-x86_64.tar.gz \
            -C artifacts/linux-x86_64-unknown-linux-gnu .

          # macOS x86_64
          tar -czvf release/agentzero-${VERSION}-macos-x86_64.tar.gz \
            -C artifacts/macos-x86_64-apple-darwin .

          # macOS ARM64
          tar -czvf release/agentzero-${VERSION}-macos-aarch64.tar.gz \
            -C artifacts/macos-aarch64-apple-darwin .

          # Windows
          cd artifacts/windows-x86_64-pc-windows-msvc
          zip -r ../../release/agentzero-${VERSION}-windows-x86_64.zip .

          # Checksums
          cd release && sha256sum * > checksums.sha256

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          name: AgentZero ${{ github.event.inputs.version || github.ref_name }}
          files: release/*
          generate_release_notes: true
```

---

## Code Signing (Optional but Recommended)

### macOS

```bash
# Sign binaries
codesign --sign "Developer ID Application: Your Name" \
  --options runtime \
  --timestamp \
  release/zerod release/zero

# Notarize
xcrun notarytool submit release/agentzero-*.zip \
  --apple-id "your@email.com" \
  --password "@keychain:AC_PASSWORD" \
  --team-id "TEAM_ID" \
  --wait

# Staple
xcrun stapler staple release/agentzero-*.zip
```

### Windows

Requires a code signing certificate (e.g., from DigiCert, Sectigo):

```powershell
# Sign with signtool
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
  /f certificate.pfx /p $PASSWORD `
  release\zerod.exe release\zero.exe
```

---

## Installation Methods

### Direct Download

Users download the archive for their platform, extract, and run:

```bash
# Linux/macOS
tar -xzf agentzero-0.1.0-linux-x86_64.tar.gz
cd agentzero-0.1.0
./zerod --static-dir ./dist
```

### Homebrew (macOS)

Create a Homebrew formula:

```ruby
# Formula/agentzero.rb
class Agentzero < Formula
  desc "Local-first AI agent platform"
  homepage "https://github.com/yourorg/agentzero"
  version "0.1.0"
  sha256 "..." # Calculate from archive

  on_macos do
    on_intel do
      url "https://github.com/yourorg/agentzero/releases/download/v#{version}/agentzero-#{version}-macos-x86_64.tar.gz"
    end
    on_arm do
      url "https://github.com/yourorg/agentzero/releases/download/v#{version}/agentzero-#{version}-macos-aarch64.tar.gz"
    end
  end

  def install
    bin.install "zerod"
    bin.install "zero"
  end

  test do
    assert_match "AgentZero", shell_output("#{bin}/zerod --version")
  end
end
```

### AUR (Arch Linux)

Create PKGBUILD:

```bash
# Maintainer: Your Name <email>
pkgname=agentzero-bin
pkgver=0.1.0
pkgrel=1
pkgdesc="Local-first AI agent platform"
arch=('x86_64' 'aarch64')
url="https://github.com/yourorg/agentzero"
license=('MIT')

source_x86_64=("${url}/releases/download/v${pkgver}/agentzero-${pkgver}-linux-x86_64.tar.gz")
source_aarch64=("${url}/releases/download/v${pkgver}/agentzero-${pkgver}-linux-aarch64.tar.gz")

package() {
    install -Dm755 zerod "$pkgdir/usr/bin/zerod"
    install -Dm755 zero "$pkgdir/usr/bin/zero"
}
```

### Docker (Alternative Distribution)

```dockerfile
# Dockerfile
FROM rust:1.75 AS builder

WORKDIR /app
COPY . .

# Build frontend
RUN apt-get update && apt-get install -y nodejs npm
RUN cd apps/ui && npm ci && npm run build

# Build backend
RUN cargo build --release -p daemon

# Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists

COPY --from=builder /app/target/release/zerod /usr/local/bin/

EXPOSE 18791 18790

CMD ["zerod"]
```

```bash
docker build -t agentzero:latest .
docker run -p 18791:18791 -p 18790:18790 \
  -v ~/Documents/agentzero:/root/Documents/agentzero \
  agentzero:latest
```

---

## Release Checklist

### Pre-Release

- [ ] Update version in `Cargo.toml` (workspace)
- [ ] Update version in `apps/ui/package.json`
- [ ] Update `CHANGELOG.md` with changes
- [ ] Run full test suite: `cargo test --workspace`
- [ ] Run frontend tests: `cd apps/ui && npm test`
- [ ] Build frontend: `cd apps/ui && npm run build`
- [ ] Test daemon locally: `cargo run -p daemon -- --static-dir ./dist`
- [ ] Verify UI works at http://localhost:18791

### Create Release

- [ ] Commit version bump: `git commit -am "chore: release v0.1.0"`
- [ ] Tag release: `git tag v0.1.0`
- [ ] Push: `git push origin main --tags`
- [ ] Wait for GitHub Actions to complete
- [ ] Verify release artifacts on GitHub Releases page
- [ ] Download and test each platform binary

### Post-Release

- [ ] Update Homebrew formula (if applicable)
- [ ] Update AUR package (if applicable)
- [ ] Update Docker image: `docker build -t agentzero:0.1.0 .`
- [ ] Announce release (Discord, Twitter, etc.)
- [ ] Update documentation site (if applicable)

---

## Troubleshooting

### Build Errors

**" linker `cc` not found"** (Linux):
```bash
sudo apt-get install build-essential
```

**"openssl not found"** (Linux):
```bash
sudo apt-get install pkg-config libssl-dev
```

**Windows MSVC errors**:
- Install Visual Studio Build Tools with "C++ build tools" workload

### Runtime Errors

**"Address already in use"**:
```bash
# Check what's using the port
lsof -i :18791
# Kill process or use different port
zerod --port 8080
```

**Permission denied (Linux/macOS)**:
```bash
chmod +x zerod zero
```

### Cross-Compilation

For reliable cross-platform builds, use GitHub Actions runners for each OS rather than cross-compilation.

---

## Version Scheme

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes

Examples:
- `0.1.0` → Initial alpha
- `0.2.0` → New features, may break compatibility
- `1.0.0` → First stable release
- `1.1.0` → New features
- `1.1.1` → Bug fix
