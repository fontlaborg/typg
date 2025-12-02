#!/bin/bash
# publish.sh - robust publishing script for typg workspace
# made by FontLab https://www.fontlab.com/

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
DRY_RUN="${DRY_RUN:-false}"
BUILD_TYPE="${BUILD_TYPE:-release}"
PYTHON_VERSION="${PYTHON_VERSION:-3.12}"

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

log_dry() {
    echo -e "${YELLOW}[DRY-RUN]${NC} $1"
}

# Usage information
usage() {
    cat << EOF
Usage: $0 [OPTIONS] [COMMAND]

Publish script for typg workspace - handles Rust crates and PyPI publishing

COMMANDS:
    publish          Publish all packages (default)
    rust-only        Publish only Rust crates
    python-only      Publish only Python package
    sync             Sync Cargo crate versions to git tag (no publish)
    check            Check if packages need publishing
    clean            Clean build artifacts

OPTIONS:
    --dry-run       Show what would be published without actually publishing
    --build-type    Build type: debug or release (default: release)
    --python-version Python version for bindings (default: 3.12)
    --help, -h      Show this help message

ENVIRONMENT VARIABLES:
    DRY_RUN=true    Enable dry-run mode
    BUILD_TYPE      Set build type
    PYTHON_VERSION  Set Python version

EXAMPLES:
    $0                           # Publish all packages
    $0 --dry-run                # Show what would be published
    $0 rust-only                 # Publish only Rust crates
    $0 python-only               # Publish only Python package
    $0 check                     # Check publishing status

EOF
}

# Check dependencies
check_dependencies() {
    log_info "Checking publishing dependencies..."
    
    local missing_deps=()
    
    # Check for Rust toolchain
    if ! command -v cargo >/dev/null 2>&1; then
        missing_deps+=("cargo")
    fi
    
    # Check for cargo-publish utilities
    if ! cargo publish --help >/dev/null 2>&1; then
        log_warning "cargo publish may not be available"
    fi
    
    # Check for Python and uv
    if ! command -v "python${PYTHON_VERSION}" >/dev/null 2>&1; then
        missing_deps+=("python${PYTHON_VERSION}")
    fi
    
    if ! command -v uv >/dev/null 2>&1; then
        missing_deps+=("uv")
    fi

    # Check for maturin
    if ! command -v maturin >/dev/null 2>&1; then
        missing_deps+=("maturin")
    fi

    # Check for hatch (hatch-vcs supplies the version)
    if ! command -v hatch >/dev/null 2>&1; then
        missing_deps+=("hatch")
    fi
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        log_info "Install missing dependencies:"
        
        if [[ " ${missing_deps[*]} " =~ " maturin " ]]; then
            log_info "  Maturin: pip${PYTHON_VERSION} install maturin"
        fi
        
        if [[ " ${missing_deps[*]} " =~ " uv " ]]; then
            log_info "  UV: pip install uv"
        fi

        if [[ " ${missing_deps[*]} " =~ " hatch " ]]; then
            log_info "  Hatch + hatch-vcs: pip install hatch hatch-vcs"
        fi

        if [[ " ${missing_deps[*]} " =~ " python${PYTHON_VERSION} " ]]; then
            log_info "  Python: Install via Homebrew or python.org"
        fi
        
        return 1
    fi

    if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
        log_warning "CARGO_REGISTRY_TOKEN not set; crates.io publish will fail"
    fi

    if [[ -z "${UV_PUBLISH_TOKEN:-}" && -z "${PYPI_TOKEN:-}" ]]; then
        log_warning "No PyPI token (UV_PUBLISH_TOKEN or PYPI_TOKEN) detected; PyPI publish may fail"
    fi
    
    log_success "All publishing dependencies found"
    return 0
}

# Get current version from Cargo.toml
get_crate_version() {
    local crate_dir="$1"
    local cargo_toml="$crate_dir/Cargo.toml"
    
    if [[ ! -f "$cargo_toml" ]]; then
        echo "0.0.0"
        return 1
    fi
    
    # Extract version using grep and sed
    grep '^version = ' "$cargo_toml" | sed 's/version = "\(.*\)"/\1/' | head -n1
}

# Get published version from crates.io
get_published_version() {
    local crate_name="$1"
    
    # Check if crate exists on crates.io using cargo search
    if cargo search "$crate_name" --limit 1 2>/dev/null | grep -q "$crate_name"; then
        # Extract current version from crates.io API
        curl -s "https://crates.io/api/v1/crates/$crate_name" 2>/dev/null | \
            jq -r '.crate.max_stable_version' 2>/dev/null || echo "unknown"
    else
        echo "none"
    fi
}

# Get published Python version from PyPI
get_python_published_version() {
    local package_name="$1"
    
    # Check PyPI API
    local response=$(curl -s "https://pypi.org/pypi/$package_name/json" 2>/dev/null || echo "")
    if [[ -n "$response" ]] && echo "$response" | jq -e '.info.version' >/dev/null 2>&1; then
        echo "$response" | jq -r '.info.version' 2>/dev/null || echo "unknown"
    else
        echo "none"
    fi
}

# Derive version from git tags via hatch-vcs (required)
get_semver_version() {
    if ! command -v hatch >/dev/null 2>&1; then
        log_error "hatch is required to compute the version via hatch-vcs"
        return 1
    fi

    local raw_version
    pushd "$PROJECT_ROOT/typg-python" >/dev/null
    if ! raw_version=$(hatch version 2>/dev/null); then
        popd >/dev/null
        log_error "Failed to read version from hatch-vcs; ensure git tags exist"
        return 1
    fi
    popd >/dev/null

    # Strip leading 'v' if present
    raw_version="${raw_version#v}"

    if [[ "$raw_version" == *dev* ]] || [[ "$raw_version" == *+* ]]; then
        log_error "Version $raw_version is not a clean tag; create/checkout a semver tag before publishing"
        return 1
    fi

    echo "$raw_version"
}

# Write the resolved version into Cargo manifests and path deps
sync_versions_to_cargo() {
    local version="$1"

    log_info "Syncing Cargo.toml versions to ${version}"

    perl -0pi -e "s/^version = \"[^\"]*\"/version = \"${version}\"/m" "$PROJECT_ROOT/typg-core/Cargo.toml"
    perl -0pi -e "s/^version = \"[^\"]*\"/version = \"${version}\"/m" "$PROJECT_ROOT/typg-cli/Cargo.toml"
    perl -0pi -e "s/^version = \"[^\"]*\"/version = \"${version}\"/m" "$PROJECT_ROOT/typg-python/Cargo.toml"

    perl -0pi -e "s|typg-core = \{[^}]*path = \"\.\./typg-core\"[^}]*\}|typg-core = { version = \"=${version}\", path = \"../typg-core\" }|" "$PROJECT_ROOT/typg-cli/Cargo.toml"
    perl -0pi -e "s|typg-core = \{[^}]*path = \"\.\./typg-core\"[^}]*\}|typg-core = { version = \"=${version}\", path = \"../typg-core\" }|" "$PROJECT_ROOT/typg-python/Cargo.toml"
}

sync_only() {
    log_info "Syncing versions from git tag via hatch-vcs (no publish)"

    if ! command -v hatch >/dev/null 2>&1; then
        log_error "hatch is required to resolve version from git tags"
        return 1
    fi

    local resolved_version
    resolved_version=$(get_semver_version)
    log_info "Resolved version: ${resolved_version}"

    sync_versions_to_cargo "$resolved_version"

    log_success "Cargo manifest versions updated to ${resolved_version}"
}

# Check if version needs publishing
check_version_needs_publish() {
    local current="$1"
    local published="$2"
    local package_name="$3"
    
    case "$published" in
        "none")
            log_info "$package_name has not been published yet"
            return 0
            ;;
        "unknown")
            log_warning "Could not determine published version for $package_name"
            return 2
            ;;
        *)
            if [[ "$current" == "$published" ]]; then
                log_info "$package_name version $current is already published"
                return 1
            else
                log_info "$package_name needs publishing (current: $current, published: $published)"
                return 0
            fi
            ;;
    esac
}

# Check if repository is clean for publishing
check_repo_clean() {
    log_info "Checking repository state..."
    
    # Check git status
    if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
        log_error "Repository has uncommitted changes"
        log_info "Commit or stash changes before publishing"
        return 1
    fi
    
    # Check if we're on main branch
    local current_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
    if [[ "$current_branch" != "main" ]] && [[ "$current_branch" != "master" ]]; then
        log_warning "Not on main/master branch (current: $current_branch)"
    fi
    
    log_success "Repository state is clean"
    return 0
}

# Build packages before publishing
build_packages() {
    log_info "Building packages for publishing..."
    
    # Use existing build script
    if [[ -f "$PROJECT_ROOT/build.sh" ]]; then
        if ! bash "$PROJECT_ROOT/build.sh" "$BUILD_TYPE" "$PYTHON_VERSION"; then
            log_error "Build failed"
            return 1
        fi
    else
        log_error "build.sh not found"
        return 1
    fi
    
    log_success "Build completed successfully"
}

# Publish Rust crate
publish_rust_crate() {
    local crate_dir="$1"
    local crate_name="$2"
    
    log_info "Publishing Rust crate: $crate_name"
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_dry "Would publish: $crate_name from $crate_dir"
        return 0
    fi
    
    cd "$crate_dir"
    
    # Ensure the crate builds
    if ! cargo build --release; then
        log_error "Failed to build $crate_name"
        cd "$PROJECT_ROOT"
        return 1
    fi
    
    # Publish to crates.io
    if cargo publish; then
        log_success "Published $crate_name to crates.io"
        cd "$PROJECT_ROOT"
        return 0
    else
        log_error "Failed to publish $crate_name"
        cd "$PROJECT_ROOT"
        return 1
    fi
}

# Publish Python package
publish_python_package() {
    local package_dir="$1"
    local package_name="$2"
    
    log_info "Publishing Python package: $package_name"
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_dry "Would publish: $package_name from $package_dir"
        return 0
    fi
    
    cd "$package_dir"
    
    # Build Python package
    if ! maturin build --release --features extension-module; then
        log_error "Failed to build Python package"
        cd "$PROJECT_ROOT"
        return 1
    fi
    
    # Find the built wheel
    local wheel_file=$(find "$PROJECT_ROOT/target/wheels" -name "typg-*.whl" -type f | head -n1)
    if [[ -z "$wheel_file" ]]; then
        log_error "Could not find built Python wheel"
        cd "$PROJECT_ROOT"
        return 1
    fi

    # Publish to PyPI using uv
    local publish_token="${PYPI_TOKEN:-${UV_PUBLISH_TOKEN:-}}"
    local publish_args=()
    if [[ -n "$publish_token" ]]; then
        publish_args+=(--token "$publish_token")
    fi

    if uv publish "${publish_args[@]}" "$wheel_file"; then
        log_success "Published $package_name to PyPI"
        cd "$PROJECT_ROOT"
        return 0
    else
        log_error "Failed to publish $package_name"
        cd "$PROJECT_ROOT"
        return 1
    fi
}

# Main publish function
publish_all() {
    local rust_only="${1:-false}"
    local python_only="${2:-false}"
    
    log_info "Starting publishing process..."
    log_info "Dry run: $DRY_RUN"
    log_info "Build type: $BUILD_TYPE"
    
    # Check repository state
    check_repo_clean
    
    # Check dependencies
    check_dependencies

    # Resolve version from git tags via hatch-vcs and sync manifests
    local resolved_version
    resolved_version=$(get_semver_version)
    log_info "Using version: ${resolved_version} (from git tags via hatch-vcs)"
    sync_versions_to_cargo "$resolved_version"
    
    # Check what needs publishing
    local needs_publishing=false
    local rust_needs=false
    local python_needs=false
    
    # Check Rust crates
    if [[ "$python_only" != "true" ]]; then
        log_info "Checking Rust crate versions..."
        
        for crate in typg-core typg-cli typg-python; do
            local current_version=$(get_crate_version "$PROJECT_ROOT/$crate")
            local published_version=$(get_published_version "$crate")
            
            if check_version_needs_publish "$current_version" "$published_version" "$crate"; then
                needs_publishing=true
                rust_needs=true
                break
            fi
        done
    fi
    
    # Check Python package
    if [[ "$rust_only" != "true" ]]; then
        log_info "Checking Python package version..."
        
        local current_python_version=$(get_crate_version "$PROJECT_ROOT/typg-python")
        local published_python_version=$(get_python_published_version "typg")
        
        if check_version_needs_publish "$current_python_version" "$published_python_version" "typg-python"; then
            needs_publishing=true
            python_needs=true
        fi
    fi
    
    if [[ "$needs_publishing" != "true" ]]; then
        log_success "All packages are up to date - nothing to publish"
        return 0
    fi
    
    # Build packages
    build_packages
    
    # Publish in correct order
    local failed_packages=()
    
    # First publish typg-core (dependency of others)
    if [[ "$python_only" != "true" ]] && [[ "$rust_needs" == "true" ]]; then
        local core_version=$(get_crate_version "$PROJECT_ROOT/typg-core")
        local core_published=$(get_published_version "typg-core")
        
        if check_version_needs_publish "$core_version" "$core_published" "typg-core"; then
            if ! publish_rust_crate "$PROJECT_ROOT/typg-core" "typg-core"; then
                failed_packages+=("typg-core")
            fi
        fi
        
        # Wait a bit for crates.io to update
        sleep 3
    fi
    
    # Then publish other Rust crates
    for crate in typg-cli typg-python; do
        if [[ "$python_only" != "true" ]] && [[ "$rust_needs" == "true" ]]; then
            local current_version=$(get_crate_version "$PROJECT_ROOT/$crate")
            local published_version=$(get_published_version "$crate")

            if check_version_needs_publish "$current_version" "$published_version" "$crate"; then
                if ! publish_rust_crate "$PROJECT_ROOT/$crate" "$crate"; then
                    failed_packages+=("$crate")
                fi
            fi

            sleep 2
        fi
    done
    
    # Finally publish Python package
    if [[ "$rust_only" != "true" ]] && [[ "$python_needs" == "true" ]]; then
        local current_python_version=$(get_crate_version "$PROJECT_ROOT/typg-python")
        local published_python_version=$(get_python_published_version "typg")
        
        if check_version_needs_publish "$current_python_version" "$published_python_version" "typg (PyPI)"; then
            if ! publish_python_package "$PROJECT_ROOT/typg-python" "typg"; then
                failed_packages+=("typg (PyPI)")
            fi
        fi
    fi
    
    # Report results
    if [[ ${#failed_packages[@]} -eq 0 ]]; then
        log_success "All packages published successfully!"
        return 0
    else
        log_error "Failed to publish: ${failed_packages[*]}"
        log_error "Publishing aborted - check logs above for details"
        return 1
    fi
}

# Check publishing status
check_status() {
    log_info "Checking publishing status..."
    
    log_info "Rust crates:"
    for crate in typg-core typg-cli typg-python; do
        local current_version=$(get_crate_version "$PROJECT_ROOT/$crate")
        local published_version=$(get_published_version "$crate")
        local status=""
        
        if check_version_needs_publish "$current_version" "$published_version" "$crate"; then
            status="${YELLOW}NEEDS PUBLISH${NC}"
        else
            status="${GREEN}UP TO DATE${NC}"
        fi
        
        echo -e "  $crate: $current_version (published: $published_version) [$status]"
    done
    
    log_info "Python package:"
    local current_python_version=$(get_crate_version "$PROJECT_ROOT/typg-python")
    local published_python_version=$(get_python_published_version "typg")
    local python_status=""
    
    if check_version_needs_publish "$current_python_version" "$published_python_version" "typg-python"; then
        python_status="${YELLOW}NEEDS PUBLISH${NC}"
    else
        python_status="${GREEN}UP TO DATE${NC}"
    fi
    
    echo -e "  typg (PyPI): $current_python_version (published: $published_python_version) [$python_status]"
}

# Main execution
main() {
    local command="${1:-publish}"
    local rust_only=false
    local python_only=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)
                export DRY_RUN=true
                shift
                ;;
            --build-type)
                export BUILD_TYPE="$2"
                shift 2
                ;;
            --python-version)
                export PYTHON_VERSION="$2"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            publish|rust-only|python-only|check|clean|sync)
                command="$1"
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
    
    # Set flags for specific commands
    case "$command" in
        rust-only)
            rust_only=true
            ;;
        python-only)
            python_only=true
            ;;
        sync)
            command="sync"
            ;;
    esac
    
    # Execute command
    case "$command" in
        publish|rust-only|python-only)
            publish_all "$rust_only" "$python_only"
            ;;
        sync)
            sync_only
            ;;
        check)
            check_status
            ;;
        clean)
            log_info "Cleaning build artifacts..."
            bash "$PROJECT_ROOT/build.sh" clean
            ;;
        *)
            log_error "Unknown command: $command"
            usage
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
