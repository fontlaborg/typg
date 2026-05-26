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

Install the typg CLI binary and Python bindings on the current machine.

OPTIONS:
    --rust-only     Install only the Rust CLI binary (typg)
    --python-only   Install only the Python bindings package (typg)
    --python-version VER Specify Python version (e.g. 3.12, 3.13) (default: 3.13)
    --debug         Install debug build (faster compile, slower runtime)
    --hpindex       Enable high-performance LMDB index feature
    --path DIR      Install Rust CLI to DIR instead of ~/.cargo/bin
    -h, --help      Show this help

Examples:
    $0                        # Install Rust CLI and Python bindings
    $0 --rust-only            # Only install Rust CLI
    $0 --hpindex              # With LMDB index support
    $0 --path /usr/local/bin  # Install CLI to custom location

EOF
}

INSTALL_DIR=""
BUILD_FLAGS="--release"
FEATURE_FLAGS=""
INSTALL_RUST=true
INSTALL_PYTHON=true
PYTHON_VERSION="3.13"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --rust-only)
            INSTALL_PYTHON=false
            shift
            ;;
        --python-only)
            INSTALL_RUST=false
            shift
            ;;
        --python-version|--python)
            PYTHON_VERSION="$2"
            shift 2
            ;;
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

if [[ "$INSTALL_RUST" == "true" ]]; then
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
        log_success "Installed typg CLI to $INSTALL_DIR/typg"
    else
        # Use cargo install (installs to ~/.cargo/bin by default)
        log_info "Installing typg CLI via cargo install..."
        cargo install --path "$SCRIPT_DIR/cli" $FEATURE_FLAGS
        log_success "Installed typg CLI to $(which typg 2>/dev/null || echo '~/.cargo/bin/typg')"
    fi

    # Verify Rust CLI
    if command -v typg >/dev/null 2>&1; then
        log_success "typg CLI is ready: $(typg --version 2>/dev/null || echo 'installed')"
    else
        log_warning "typg CLI was installed but is not on PATH. Add the install directory to your PATH."
    fi
fi

if [[ "$INSTALL_PYTHON" == "true" ]]; then
    log_info "Installing Python bindings..."
    
    # Check for python
    if ! command -v "python${PYTHON_VERSION}" >/dev/null 2>&1; then
        log_warning "python${PYTHON_VERSION} not found — skipping Python bindings installation"
    elif ! command -v maturin >/dev/null 2>&1; then
        log_warning "maturin not found — skipping Python bindings installation"
        log_info "  Install with: pip${PYTHON_VERSION} install maturin"
    else
        log_info "Building Python wheel with maturin..."
        # Build using maturin build
        arch=$(uname -m)
        target=""
        case "$arch" in
            arm64) target="aarch64-apple-darwin" ;;
            x86_64) target="x86_64-apple-darwin" ;;
            *) target="unknown" ;;
        esac
        
        maturin_release_flag=""
        if [[ -n "$BUILD_FLAGS" ]]; then
            maturin_release_flag="--release"
        fi
        
        if (cd "$SCRIPT_DIR/py/typg-python" && maturin build $maturin_release_flag --target "$target" --interpreter "python${PYTHON_VERSION}" --features extension-module); then
            # Find built wheel
            wheel_file=$(find "$SCRIPT_DIR/target/wheels" -name "typg-*.whl" -type f | head -n1)
            if [[ -n "$wheel_file" ]]; then
                log_info "Installing Python wheel: $wheel_file"
                if command -v uv >/dev/null 2>&1; then
                    if uv pip install --system "$wheel_file" 2>/dev/null || uv pip install "$wheel_file" 2>/dev/null; then
                        log_success "Python bindings installed successfully via uv"
                    else
                        "pip${PYTHON_VERSION}" install "$wheel_file" --user || pip install "$wheel_file" --user
                        log_success "Python bindings installed successfully via pip"
                    fi
                else
                    "pip${PYTHON_VERSION}" install "$wheel_file" --user || pip install "$wheel_file" --user
                    log_success "Python bindings installed successfully via pip"
                fi
            else
                log_error "Could not find built Python wheel"
            fi
        else
            log_error "Failed to build Python bindings wheel"
        fi
    fi
fi

