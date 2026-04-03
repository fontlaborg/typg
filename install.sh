#!/bin/bash
# install.sh - install the typg CLI on the current machine
# made by FontLab https://www.fontlab.com/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Install the typg CLI binary on the current machine.

OPTIONS:
    --debug         Install debug build (faster compile, slower runtime)
    --hpindex       Enable high-performance LMDB index feature
    --path DIR      Install to DIR instead of ~/.cargo/bin
    -h, --help      Show this help

Examples:
    $0                        # Release install to ~/.cargo/bin
    $0 --hpindex              # With LMDB index support
    $0 --path /usr/local/bin  # Install to custom location
    $0 --debug                # Debug build (faster compile)

EOF
}

INSTALL_DIR=""
BUILD_FLAGS="--release"
FEATURE_FLAGS=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            BUILD_FLAGS=""
            shift
            ;;
        --hpindex)
            FEATURE_FLAGS="--features hpindex"
            shift
            ;;
        --path)
            INSTALL_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Check for cargo
if ! command -v cargo >/dev/null 2>&1; then
    log_error "cargo not found. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

if [[ -n "$INSTALL_DIR" ]]; then
    # Build and copy manually
    log_info "Building typg CLI..."
    cargo build -p typg-cli $BUILD_FLAGS $FEATURE_FLAGS --manifest-path "$SCRIPT_DIR/Cargo.toml"

    local_target="$SCRIPT_DIR/target"
    if [[ -n "$BUILD_FLAGS" ]]; then
        bin_path="$local_target/release/typg"
    else
        bin_path="$local_target/debug/typg"
    fi

    if [[ ! -f "$bin_path" ]]; then
        log_error "Build succeeded but binary not found at $bin_path"
        exit 1
    fi

    mkdir -p "$INSTALL_DIR"
    cp "$bin_path" "$INSTALL_DIR/typg"
    chmod +x "$INSTALL_DIR/typg"
    log_success "Installed typg to $INSTALL_DIR/typg"
else
    # Use cargo install (installs to ~/.cargo/bin by default)
    log_info "Installing typg CLI via cargo install..."
    cargo install --path "$SCRIPT_DIR/cli" $FEATURE_FLAGS
    log_success "Installed typg to $(which typg 2>/dev/null || echo '~/.cargo/bin/typg')"
fi

# Verify
if command -v typg >/dev/null 2>&1; then
    log_success "typg is ready: $(typg --version 2>/dev/null || echo 'installed')"
else
    log_warning "typg was installed but is not on PATH. Add the install directory to your PATH."
fi
