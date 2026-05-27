#!/usr/bin/env bash
set -euo pipefail

# autoresearch installer
# Builds the Rust binary and installs for Claude Code and/or Codex.
#
# Usage:
#   ./install.sh              # auto-detect and install
#   ./install.sh claude       # Claude Code plugin only
#   ./install.sh codex        # Codex skill only
#   ./install.sh both         # both

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$REPO_DIR/bin"
BINARY="$BIN_DIR/autoresearch"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${GREEN}▸${NC} $1"; }
warn()  { echo -e "${YELLOW}▸${NC} $1"; }
err()   { echo -e "${RED}▸${NC} $1" >&2; }
bold()  { echo -e "${BOLD}$1${NC}"; }

# --- Build ---
build_binary() {
    if ! command -v cargo &>/dev/null; then
        err "Rust toolchain not found. Install from https://rustup.rs"
        exit 1
    fi

    info "Building autoresearch binary (release)..."
    cd "$REPO_DIR"
    cargo build --release --quiet 2>&1

    mkdir -p "$BIN_DIR"
    cp "target/release/autoresearch" "$BINARY"
    chmod +x "$BINARY"

    local size
    size=$(du -h "$BINARY" | cut -f1)
    info "Built: $BINARY ($size)"
}

# --- Claude Code ---
install_claude() {
    info "Installing Claude Code plugin..."

    # The repo IS the plugin. Just verify structure.
    if [[ ! -f "$REPO_DIR/.claude-plugin/plugin.json" ]]; then
        err "Missing .claude-plugin/plugin.json"
        exit 1
    fi
    if [[ ! -f "$REPO_DIR/hooks/hooks.json" ]]; then
        err "Missing hooks/hooks.json"
        exit 1
    fi

    bold ""
    bold "Claude Code plugin ready."
    bold ""
    echo "  Install with:"
    echo ""
    echo "    claude plugin add coder-company/agent-autoresearch"
    echo ""
    echo "  Or from local path:"
    echo ""
    echo "    claude plugin add $REPO_DIR"
    echo ""
    echo "  Then restart your session. Commands available as /autoresearch"
    echo ""
    echo "  Pro tip: use /goal for autonomous multi-turn runs:"
    echo "    /autoresearch"
    echo "    Goal: Increase test coverage from 72% to 90%"
    echo ""
}

# --- Codex ---
install_codex() {
    info "Installing Codex skill..."

    # Option 1: skill-installer (if codex is available)
    if command -v codex &>/dev/null; then
        bold ""
        bold "Codex detected. Install with:"
        echo ""
        echo "  In Codex:"
        echo '    $skill-installer install https://github.com/coder-company/agent-autoresearch'
        echo ""
        echo "  Or copy to project:"
        echo "    cp -r $REPO_DIR your-project/.agents/skills/autoresearch"
        echo ""
        echo "  Or user-scope:"
        echo "    cp -r $REPO_DIR ~/.agents/skills/autoresearch"
        echo ""
    else
        bold ""
        bold "Codex skill ready."
        echo ""
        echo "  Copy to project:"
        echo "    cp -r $REPO_DIR your-project/.agents/skills/autoresearch"
        echo ""
        echo "  Or user-scope:"
        echo "    cp -r $REPO_DIR ~/.agents/skills/autoresearch"
        echo ""
    fi

    echo '  Then use: $autoresearch'
    echo ""
}

# --- Main ---
main() {
    bold "autoresearch installer"
    echo ""

    local target="${1:-auto}"

    build_binary

    case "$target" in
        claude)
            install_claude
            ;;
        codex)
            install_codex
            ;;
        both)
            install_claude
            install_codex
            ;;
        auto)
            install_claude
            install_codex
            ;;
        *)
            err "Unknown target: $target"
            echo "Usage: ./install.sh [claude|codex|both|auto]"
            exit 1
            ;;
    esac

    info "Done. Binary at: $BINARY"
}

main "$@"
