#!/usr/bin/env bash
set -euo pipefail

# ── Autoresearch Installer ────────────────────────────────────────────
# Builds the Rust binary and optionally installs for Claude Code / Codex.
#
# Usage:
#   ./install.sh                # Interactive guided install
#   ./install.sh --help         # Show usage
# ──────────────────────────────────────────────────────────────────────

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_INSTALL_DIR="$HOME/.local/bin"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()    { echo -e "${GREEN}▸${NC} $1"; }
warn()    { echo -e "${YELLOW}▸${NC} $1"; }
err()     { echo -e "${RED}✗${NC} $1" >&2; }
header()  { echo -e "\n${BOLD}${CYAN}$1${NC}"; }
success() { echo -e "${GREEN}✓${NC} $1"; }

# ── OS Detection ──────────────────────────────────────────────────────

detect_os() {
    case "$(uname -s)" in
        Linux*)  OS="linux" ;;
        Darwin*) OS="macos" ;;
        *)
            err "Unsupported operating system: $(uname -s)"
            err "Autoresearch supports Linux and macOS."
            exit 1
            ;;
    esac
    info "Detected OS: $OS ($(uname -m))"
}

# ── Rust Toolchain ────────────────────────────────────────────────────

check_rust() {
    if command -v cargo &>/dev/null; then
        local version
        version=$(rustc --version 2>/dev/null || echo "unknown")
        success "Rust toolchain found: $version"
        return 0
    fi

    warn "Rust toolchain not found."
    echo ""
    echo "  Rust is required to build autoresearch."
    echo "  Install it from https://rustup.rs with:"
    echo ""
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""

    read -rp "  Install Rust now via rustup? [Y/n] " answer
    case "${answer:-Y}" in
        [Yy]*)
            info "Installing Rust via rustup..."
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            # shellcheck source=/dev/null
            source "$HOME/.cargo/env" 2>/dev/null || true
            if command -v cargo &>/dev/null; then
                success "Rust installed successfully."
            else
                err "Rust installation completed but cargo not found in PATH."
                err "Run: source \$HOME/.cargo/env"
                exit 1
            fi
            ;;
        *)
            err "Rust is required. Install from https://rustup.rs and re-run."
            exit 1
            ;;
    esac
}

# ── Build ─────────────────────────────────────────────────────────────

build_binary() {
    header "Building autoresearch (release)..."
    cd "$REPO_DIR"

    cargo build --release 2>&1

    local binary="$REPO_DIR/target/release/autoresearch"
    if [[ ! -f "$binary" ]]; then
        err "Build failed: binary not found at $binary"
        exit 1
    fi

    # Also copy to bin/ for the plugin system
    mkdir -p "$REPO_DIR/bin"
    cp "$binary" "$REPO_DIR/bin/autoresearch"
    chmod +x "$REPO_DIR/bin/autoresearch"

    local size
    if [[ "$OS" == "macos" ]]; then
        size=$(stat -f%z "$binary" 2>/dev/null || echo "0")
    else
        size=$(stat -c%s "$binary" 2>/dev/null || echo "0")
    fi
    local size_mb=$((size / 1024 / 1024))

    success "Built: $binary (${size_mb}MB)"
}

# ── Install Binary ────────────────────────────────────────────────────

install_binary() {
    header "Install binary to PATH"

    echo "  Default location: $DEFAULT_INSTALL_DIR"
    read -rp "  Install path [$DEFAULT_INSTALL_DIR]: " install_dir
    install_dir="${install_dir:-$DEFAULT_INSTALL_DIR}"

    mkdir -p "$install_dir"
    cp "$REPO_DIR/target/release/autoresearch" "$install_dir/autoresearch"
    chmod +x "$install_dir/autoresearch"

    success "Installed to $install_dir/autoresearch"

    # Check if directory is in PATH
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
        warn "$install_dir is not in your PATH."
        echo "  Add it with:"
        echo ""
        echo "    export PATH=\"$install_dir:\$PATH\""
        echo ""
        echo "  Add that line to your ~/.bashrc or ~/.zshrc for persistence."
    fi
}

# ── Claude Code Plugin ────────────────────────────────────────────────

install_claude_plugin() {
    header "Claude Code Plugin"

    # Verify plugin structure
    if [[ ! -f "$REPO_DIR/.claude-plugin/plugin.json" ]]; then
        err "Missing .claude-plugin/plugin.json — cannot install plugin."
        return 1
    fi
    if [[ ! -f "$REPO_DIR/hooks/hooks.json" ]]; then
        err "Missing hooks/hooks.json — cannot install plugin."
        return 1
    fi

    read -rp "  Install Claude Code plugin? [Y/n] " answer
    case "${answer:-Y}" in
        [Yy]*)
            if command -v claude &>/dev/null; then
                info "Installing via claude CLI..."
                claude plugin add coder-company/agent-autoresearch || {
                    warn "Remote install failed. You can install from local path instead:"
                    echo "    claude plugin add $REPO_DIR"
                }
            else
                info "Claude CLI not found. Install the plugin manually:"
                echo ""
                echo "    claude plugin add coder-company/agent-autoresearch"
                echo ""
                echo "  Or from local path:"
                echo ""
                echo "    claude plugin add $REPO_DIR"
            fi
            echo ""
            echo "  Restart your Claude Code session after installing."
            echo "  Commands available as /autoresearch and /autoresearch:<mode>"
            ;;
        *)
            info "Skipping Claude Code plugin install."
            ;;
    esac
}

# ── Codex Skill ───────────────────────────────────────────────────────

install_codex_skill() {
    header "Codex Skill"

    read -rp "  Install Codex skill? [Y/n] " answer
    case "${answer:-Y}" in
        [Yy]*)
            local target_dir
            if [[ -d ".agents/skills" ]]; then
                target_dir=".agents/skills/autoresearch"
            else
                target_dir="$HOME/.agents/skills/autoresearch"
            fi

            read -rp "  Skill install path [$target_dir]: " skill_dir
            skill_dir="${skill_dir:-$target_dir}"

            mkdir -p "$skill_dir"

            # Copy skill files
            cp "$REPO_DIR/SKILL.md" "$skill_dir/SKILL.md"
            if [[ -d "$REPO_DIR/skills" ]]; then
                cp -r "$REPO_DIR/skills/"* "$skill_dir/" 2>/dev/null || true
            fi
            if [[ -d "$REPO_DIR/references" ]]; then
                mkdir -p "$skill_dir/references"
                cp -r "$REPO_DIR/references/"* "$skill_dir/references/" 2>/dev/null || true
            fi

            success "Codex skill installed to $skill_dir"
            echo '  Use: $autoresearch'
            ;;
        *)
            info "Skipping Codex skill install."
            ;;
    esac
}

# ── Help ──────────────────────────────────────────────────────────────

show_help() {
    echo "autoresearch installer"
    echo ""
    echo "Usage: ./install.sh [--help]"
    echo ""
    echo "Interactive guided installer that:"
    echo "  1. Detects your OS (Linux, macOS)"
    echo "  2. Checks for Rust toolchain (offers to install via rustup)"
    echo "  3. Builds the release binary"
    echo "  4. Copies binary to ~/.local/bin/ (or custom path)"
    echo "  5. Optionally installs Claude Code plugin"
    echo "  6. Optionally installs Codex skill"
    echo ""
    echo "Requirements: bash, git, curl (for rustup install)"
    exit 0
}

# ── Main ──────────────────────────────────────────────────────────────

main() {
    if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
        show_help
    fi

    echo -e "${BOLD}╭─────────────────────────────────────╮${NC}"
    echo -e "${BOLD}│   autoresearch installer v0.1.0     │${NC}"
    echo -e "${BOLD}╰─────────────────────────────────────╯${NC}"
    echo ""

    detect_os
    check_rust
    build_binary
    install_binary
    install_claude_plugin
    install_codex_skill

    echo ""
    header "Done!"
    echo ""
    echo "  Next steps:"
    echo "    • Run 'autoresearch --help' to see available commands"
    echo "    • In Claude Code: /autoresearch"
    echo "    • In Codex: \$autoresearch"
    echo ""
    echo "  Quick start:"
    echo "    /autoresearch"
    echo "    Goal: Increase test coverage from 72% to 90%"
    echo "    Scope: src/**/*.ts"
    echo "    Verify: npm test -- --coverage | tail -1"
    echo ""
    success "autoresearch is ready."
}

main "$@"
