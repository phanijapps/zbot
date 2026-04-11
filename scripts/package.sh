#!/bin/bash
# Package AgentZero for distribution
# Usage: ./scripts/package.sh [version]

set -e

# Configuration
VERSION="${1:-0.1.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DIST_DIR="$ROOT_DIR/dist/release"
FRONTEND_DIR="$ROOT_DIR/apps/ui"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Detect platform
detect_platform() {
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)   echo "linux-x86_64" ;;
        Linux-aarch64)  echo "linux-aarch64" ;;
        Darwin-x86_64)  echo "macos-x86_64" ;;
        Darwin-arm64)   echo "macos-aarch64" ;;
        MINGW*-x86_64|MSYS*-x86_64) echo "windows-x86_64" ;;
        *)              echo "unknown" ;;
    esac
}

# Build frontend
build_frontend() {
    log_info "Building frontend..."

    if [[ ! -d "$FRONTEND_DIR/node_modules" ]]; then
        log_info "Installing frontend dependencies..."
        cd "$FRONTEND_DIR" && npm install
    fi

    cd "$FRONTEND_DIR" && npm run build

    if [[ ! -d "$FRONTEND_DIR/dist" ]]; then
        log_error "Frontend build failed - dist/ not found"
        exit 1
    fi

    log_info "Frontend built successfully"
}

# Build backend
build_backend() {
    log_info "Building backend (release mode)..."

    cd "$ROOT_DIR"
    cargo build --release -p daemon -p cli

    log_info "Backend built successfully"
}

# Create package
create_package() {
    local platform="$1"
    local archive_name="agentzero-${VERSION}-${platform}"
    local package_dir="$DIST_DIR/$archive_name"

    log_info "Creating package: $archive_name"

    mkdir -p "$package_dir"

    # Copy binaries
    if [[ "$platform" == "windows-"* ]]; then
        cp "$ROOT_DIR/target/release/zerod.exe" "$package_dir/"
        cp "$ROOT_DIR/target/release/zero.exe" "$package_dir/"
    else
        cp "$ROOT_DIR/target/release/zerod" "$package_dir/"
        cp "$ROOT_DIR/target/release/zero" "$package_dir/"
        chmod +x "$package_dir/zerod" "$package_dir/zero"
    fi

    # Copy documentation
    cp "$ROOT_DIR/README.md" "$package_dir/" 2>/dev/null || log_warn "README.md not found"
    cp "$ROOT_DIR/LICENSE" "$package_dir/" 2>/dev/null || log_warn "LICENSE not found"
    echo "$VERSION" > "$package_dir/VERSION"

    # Create archive
    mkdir -p "$DIST_DIR"
    cd "$DIST_DIR"

    if [[ "$platform" == "windows-"* ]]; then
        if command -v 7z &> /dev/null; then
            7z a -tzip "${archive_name}.zip" "$archive_name"
        elif command -v zip &> /dev/null; then
            zip -r "${archive_name}.zip" "$archive_name"
        else
            log_error "No zip tool found. Please install 7z or zip."
            exit 1
        fi
    else
        tar -czvf "${archive_name}.tar.gz" "$archive_name"
    fi

    # Cleanup package directory
    rm -rf "$package_dir"

    log_info "Package created: $DIST_DIR/${archive_name}.*"
}

# Generate checksums
generate_checksums() {
    log_info "Generating checksums..."

    cd "$DIST_DIR"

    if command -v sha256sum &> /dev/null; then
        sha256sum agentzero-* > checksums.sha256
    elif command -v shasum &> /dev/null; then
        shasum -a 256 agentzero-* > checksums.sha256
    else
        log_warn "No SHA256 tool found, skipping checksums"
        return
    fi

    log_info "Checksums saved to $DIST_DIR/checksums.sha256"
}

# Main
main() {
    log_info "Packaging AgentZero v$VERSION"

    # Check for required tools
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo not found. Please install Rust."
        exit 1
    fi

    if ! command -v npm &> /dev/null; then
        log_error "npm not found. Please install Node.js."
        exit 1
    fi

    # Build
    build_frontend
    build_backend

    # Detect and package for current platform
    PLATFORM=$(detect_platform)

    if [[ "$PLATFORM" = "unknown" ]]; then
        log_error "Unknown platform: $(uname -s)-$(uname -m)"
        exit 1
    fi

    create_package "$PLATFORM"
    generate_checksums

    log_info "Packaging complete!"
    log_info "Artifacts in: $DIST_DIR"

    # List created files
    ls -la "$DIST_DIR"
}

main "$@"
