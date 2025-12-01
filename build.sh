#!/bin/bash
# build.sh - robust build script for typg on macOS
# made by FontLab https://www.fontlab.com/

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
TARGET_DIR="$PROJECT_ROOT/target"
BUILD_TYPE="${1:-release}"  # Default to release build
PYTHON_VERSION="${2:-3.12}" # Default Python version for bindings

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Usage information
usage() {
    cat << EOF
Usage: $0 [BUILD_TYPE] [PYTHON_VERSION]

Build script for typg - ultra-fast font search/discovery toolkit

BUILD_TYPE:
    debug      Build with debugging information (faster compile)
    release    Optimized release build (default)
    check      Run checks without building
    clean      Clean build artifacts

PYTHON_VERSION:
    3.10, 3.11, 3.12, or 3.13 (default: 3.12)
    Used only for Python bindings compilation

Examples:
    $0                    # Release build with Python 3.12
    $0 debug             # Debug build with Python 3.12
    $0 release 3.13      # Release build with Python 3.13
    $0 clean             # Clean build artifacts

EOF
}

# Check dependencies
check_dependencies() {
    log_info "Checking dependencies..."
    
    local missing_deps=()
    
    # Check for Rust toolchain
    if ! command -v cargo >/dev/null 2>&1; then
        missing_deps+=("cargo")
    fi
    
    if ! command -v rustc >/dev/null 2>&1; then
        missing_deps+=("rustc")
    fi
    
    # Check for Python
    if ! command -v "python${PYTHON_VERSION}" >/dev/null 2>&1; then
        missing_deps+=("python${PYTHON_VERSION}")
    fi
    
    # Check for maturin (needed for Python bindings)
    if ! command -v maturin >/dev/null 2>&1; then
        missing_deps+=("maturin")
    fi
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        log_info "Install missing dependencies:"
        
        if [[ " ${missing_deps[*]} " =~ " cargo " ]] || [[ " ${missing_deps[*]} " =~ " rustc " ]]; then
            log_info "  Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        fi
        
        if [[ " ${missing_deps[*]} " =~ " python${PYTHON_VERSION} " ]]; then
            log_info "  Python: Install via Homebrew or python.org"
        fi
        
        if [[ " ${missing_deps[*]} " =~ " maturin " ]]; then
            log_info "  Maturin: pip${PYTHON_VERSION} install maturin"
        fi
        
        return 1
    fi
    
    log_success "All dependencies found"
    return 0
}

# Detect macOS architecture
detect_arch() {
    local arch=$(uname -m)
    case "$arch" in
        arm64)
            echo "aarch64-apple-darwin"
            ;;
        x86_64)
            echo "x86_64-apple-darwin"
            ;;
        *)
            log_error "Unsupported architecture: $arch"
            return 1
            ;;
    esac
}

# Clean build artifacts
clean_build() {
    log_info "Cleaning build artifacts..."
    
    # Clean Rust workspace
    cargo clean
    
    # Clean Python build artifacts if they exist
    if [[ -f "$PROJECT_ROOT/typg-python/pyproject.toml" ]]; then
        (cd "$PROJECT_ROOT/typg-python" && rm -rf build/ dist/ *.egg-info/)
    fi
    
    log_success "Build artifacts cleaned"
}

# Run checks without building
run_checks() {
    log_info "Running code quality checks..."
    
    local failed_checks=0
    
    # Rust formatting check (only check workspace members, exclude linked)
    log_info "Checking Rust formatting..."
    if cargo fmt -p typg-core -p typg-cli -p typg-python -- --check; then
        log_success "Formatting check passed"
    else
        log_error "Formatting check failed"
        ((failed_checks++))
    fi
    
    # Rust linting (only workspace members, exclude benchmarks and linked)
    log_info "Running Rust linter..."
    if cargo clippy -p typg-core -p typg-cli -p typg-python --all-targets --all-features -- -W warnings; then
        log_success "Linting check passed"
    else
        log_error "Linting check failed (warnings found)"
        ((failed_checks++))
    fi
    
    if [[ $failed_checks -eq 0 ]]; then
        log_success "All checks passed"
        return 0
    else
        log_warning "$failed_checks check(s) failed. Fix issues before proceeding with release build."
        return 1
    fi
}

# Build Rust components
build_rust() {
    local build_flags=""
    if [[ "$BUILD_TYPE" == "release" ]]; then
        build_flags="--release"
        log_info "Building Rust workspace in release mode..."
    else
        log_info "Building Rust workspace in debug mode..."
    fi
    
    # Build workspace
    cargo build $build_flags --workspace
    
    log_success "Rust components built successfully"
}

# Build Python bindings
build_python() {
    log_info "Building Python bindings for Python $PYTHON_VERSION..."
    
    cd "$PROJECT_ROOT/typg-python"
    
    # Verify we're targeting the right Python version
    local python_exe="python${PYTHON_VERSION}"
    local python_lib_dir=$("$python_exe" -c "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))")
    
    if [[ -z "$python_lib_dir" ]]; then
        log_error "Could not determine Python library directory for Python $PYTHON_VERSION"
        return 1
    fi
    
    log_info "Using Python library directory: $python_lib_dir"
    
    # Build Python extension
    local target=$(detect_arch)
    log_info "Targeting architecture: $target"
    
    # Determine if we're building release or debug
    local maturin_release_flag=""
    if [[ "$BUILD_TYPE" == "release" ]]; then
        maturin_release_flag="--release"
    fi
    
    # Build using maturin
    maturin build $maturin_release_flag \
        --target $target \
        --interpreter "python${PYTHON_VERSION}" \
        --features extension-module
    
    cd "$PROJECT_ROOT"
    
    log_success "Python bindings built successfully"
}

# Install Python bindings locally (development)
install_python_dev() {
    log_info "Installing Python bindings in development mode..."
    
    cd "$PROJECT_ROOT/typg-python"
    
    # Try to install in development mode using maturin develop
    if maturin develop --features extension-module 2>/dev/null; then
        log_success "Python bindings installed in development mode via maturin develop"
    else
        # If maturin develop fails, build wheel and install with pip
        log_warning "No virtualenv detected, building wheel and installing with pip..."
        
        # Build wheel
        local target=$(detect_arch)
        local maturin_release_flag=""
        if [[ "$BUILD_TYPE" == "release" ]]; then
            maturin_release_flag="--release"
        fi
        
        maturin build $maturin_release_flag \
            --target $target \
            --interpreter "python${PYTHON_VERSION}" \
            --features extension-module
            
        # Find the built wheel and install it
        local wheel_file=$(find "$PROJECT_ROOT/target/wheels" -name "typg-*.whl" -type f | head -n1)
        if [[ -n "$wheel_file" ]]; then
            "pip${PYTHON_VERSION}" install "$wheel_file" --user
            log_success "Python bindings installed via pip install"
        else
            log_error "Could not find built wheel to install"
            cd "$PROJECT_ROOT"
            return 1
        fi
    fi
    
    cd "$PROJECT_ROOT"
}

# Verify build
verify_build() {
    log_info "Verifying build..."
    
    # Check if core library was built
    local target_path="$TARGET_DIR"
    if [[ "$BUILD_TYPE" == "release" ]]; then
        target_path="$target_path/release"
    else
        target_path="$target_path/debug"
    fi
    
    if [[ ! -f "$target_path/libtypg_core.a" ]] && [[ ! -f "$target_path/libtypg_core.rlib" ]]; then
        log_error "Core library not found at $target_path"
        return 1
    fi
    
    # Check CLI binary
    if [[ ! -f "$target_path/typg" ]]; then
        log_error "CLI binary not found at $target_path/typg"
        return 1
    fi
    
    log_success "Build verification passed"
}

# Main build function
main() {
    log_info "Starting typg build process..."
    log_info "Project root: $PROJECT_ROOT"
    log_info "Build type: $BUILD_TYPE"
    
    # Handle special commands
    case "$BUILD_TYPE" in
        help|--help|-h)
            usage
            exit 0
            ;;
        check)
            check_dependencies
            run_checks
            exit 0
            ;;
        clean)
            check_dependencies
            clean_build
            exit 0
            ;;
        debug|release)
            # Valid build types, continue
            ;;
        *)
            log_error "Unknown build type: $BUILD_TYPE"
            usage
            exit 1
            ;;
    esac
    
    # Dependencies check
    check_dependencies
    
    # Build components
    build_rust
    build_python
    
    # Verify
    verify_build
    
    log_success "Build completed successfully!"
    
    # Show artifact locations
    local target_path="$TARGET_DIR"
    if [[ "$BUILD_TYPE" == "release" ]]; then
        target_path="$target_path/release"
    else
        target_path="$target_path/debug"
    fi
    
    echo
    log_info "Build artifacts:"
    echo "  Core library: $target_path/libtypg_core.rlib"
    echo "  CLI binary: $target_path/typg"
    echo "  Python wheel: $TARGET_DIR/wheels/"
    echo
    
    log_info "To install Python bindings in development mode, run:"
    echo "  $0 dev-install $BUILD_TYPE $PYTHON_VERSION"
}

# Additional commands
case "${1:-}" in
    dev-install)
        BUILD_TYPE="${2:-release}"
        PYTHON_VERSION="${3:-3.12}"
        check_dependencies
        install_python_dev
        exit 0
        ;;
    *)
        main
        ;;
esac
