#!/usr/bin/env bash
set -euo pipefail

# ── Autoresearch Installer ────────────────────────────────────────────
# Builds the Rust binary and optionally installs for Claude Code, OpenCode,
# or Codex.
#
# Usage:
#   ./install.sh                # Interactive guided install
#   ./install.sh --help         # Show usage
# ──────────────────────────────────────────────────────────────────────

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_INSTALL_DIR="$HOME/.local/bin"

ASSUME_YES=0
INSTALL_BINARY=1
INSTALL_DIR="$DEFAULT_INSTALL_DIR"
COMPONENT_FLAGS_SET=0
INSTALL_CLAUDE=0
INSTALL_OPENCODE=0
INSTALL_CODEX=0
OPENCODE_DIR=""
CODEX_SKILL_DIR=""

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

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                show_help
                ;;
            -y|--yes)
                ASSUME_YES=1
                ;;
            --install-dir)
                shift
                [[ $# -gt 0 ]] || { err "--install-dir requires a path"; exit 1; }
                INSTALL_DIR="${1/#\~/$HOME}"
                ;;
            --install-dir=*)
                INSTALL_DIR="${1#*=}"
                INSTALL_DIR="${INSTALL_DIR/#\~/$HOME}"
                ;;
            --no-binary)
                INSTALL_BINARY=0
                ;;
            --claude)
                COMPONENT_FLAGS_SET=1
                INSTALL_CLAUDE=1
                ;;
            --opencode)
                COMPONENT_FLAGS_SET=1
                INSTALL_OPENCODE=1
                ;;
            --codex)
                COMPONENT_FLAGS_SET=1
                INSTALL_CODEX=1
                ;;
            --all)
                COMPONENT_FLAGS_SET=1
                INSTALL_CLAUDE=1
                INSTALL_OPENCODE=1
                INSTALL_CODEX=1
                ;;
            --opencode-dir)
                shift
                [[ $# -gt 0 ]] || { err "--opencode-dir requires a path"; exit 1; }
                OPENCODE_DIR="${1/#\~/$HOME}"
                ;;
            --opencode-dir=*)
                OPENCODE_DIR="${1#*=}"
                OPENCODE_DIR="${OPENCODE_DIR/#\~/$HOME}"
                ;;
            --codex-dir)
                shift
                [[ $# -gt 0 ]] || { err "--codex-dir requires a path"; exit 1; }
                CODEX_SKILL_DIR="${1/#\~/$HOME}"
                ;;
            --codex-dir=*)
                CODEX_SKILL_DIR="${1#*=}"
                CODEX_SKILL_DIR="${CODEX_SKILL_DIR/#\~/$HOME}"
                ;;
            *)
                err "Unknown argument: $1"
                echo "Run ./install.sh --help for usage."
                exit 1
                ;;
        esac
        shift
    done
}

component_enabled() {
    local component="$1"
    local prompt="$2"
    local flag_var="INSTALL_${component}"

    if [[ "$COMPONENT_FLAGS_SET" -eq 1 ]]; then
        [[ "${!flag_var}" -eq 1 ]]
        return
    fi

    if [[ "$ASSUME_YES" -eq 1 ]]; then
        return 0
    fi

    local answer
    read -rp "$prompt" answer
    case "${answer:-Y}" in
        [Yy]*) return 0 ;;
        *) return 1 ;;
    esac
}

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
    if [[ "$INSTALL_BINARY" -eq 0 ]]; then
        info "Skipping binary install."
        return 0
    fi

    header "Install binary to PATH"

    local install_dir="$INSTALL_DIR"
    if [[ "$ASSUME_YES" -eq 0 ]]; then
        echo "  Default location: $INSTALL_DIR"
        read -rp "  Install path [$INSTALL_DIR]: " install_dir
        install_dir="${install_dir:-$INSTALL_DIR}"
    fi

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

    if component_enabled "CLAUDE" "  Install Claude Code plugin? [Y/n] "; then
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
    else
        info "Skipping Claude Code plugin install."
    fi
}

# ── OpenCode Assets ───────────────────────────────────────────────────

install_opencode_assets() {
    header "OpenCode Assets"

    if [[ ! -d "$REPO_DIR/.opencode/skills/autoresearch" ]]; then
        err "Missing .opencode/skills/autoresearch — cannot install OpenCode assets."
        return 1
    fi

    if component_enabled "OPENCODE" "  Install OpenCode assets? [Y/n] "; then
            local target_root
            if [[ -n "$OPENCODE_DIR" ]]; then
                target_root="$OPENCODE_DIR"
            elif [[ -n "${OPENCODE_CONFIG_DIR:-}" ]]; then
                target_root="$OPENCODE_CONFIG_DIR"
            elif [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
                target_root="$XDG_CONFIG_HOME/opencode"
            else
                target_root="$HOME/.config/opencode"
            fi

            local opencode_dir="$target_root"
            if [[ "$ASSUME_YES" -eq 0 ]]; then
                read -rp "  OpenCode config path [$target_root]: " opencode_dir
                opencode_dir="${opencode_dir:-$target_root}"
            fi

            mkdir -p "$opencode_dir/skills" "$opencode_dir/commands" "$opencode_dir/agents"
            rm -rf "$opencode_dir/skills/autoresearch"
            cp -R "$REPO_DIR/.opencode/skills/autoresearch" "$opencode_dir/skills/autoresearch"
            cp "$REPO_DIR"/.opencode/commands/autoresearch*.md "$opencode_dir/commands/"
            if [[ -d "$REPO_DIR/.opencode/agents" ]]; then
                cp "$REPO_DIR"/.opencode/agents/*.md "$opencode_dir/agents/" 2>/dev/null || true
            fi

            success "OpenCode assets installed to $opencode_dir"
            echo "  Use: /autoresearch or /autoresearch_debug"
    else
        info "Skipping OpenCode assets install."
    fi
}

# ── Codex Skill ───────────────────────────────────────────────────────

install_codex_skill() {
    header "Codex Skill"

    if component_enabled "CODEX" "  Install Codex skill? [Y/n] "; then
            local target_dir
            if [[ -n "$CODEX_SKILL_DIR" ]]; then
                target_dir="$CODEX_SKILL_DIR"
            elif [[ -n "${CODEX_HOME:-}" ]]; then
                target_dir="$CODEX_HOME/skills/autoresearch"
            else
                target_dir="$HOME/.codex/skills/autoresearch"
            fi

            local skill_dir="$target_dir"
            if [[ "$ASSUME_YES" -eq 0 ]]; then
                read -rp "  Skill install path [$target_dir]: " skill_dir
                skill_dir="${skill_dir:-$target_dir}"
            fi

            mkdir -p "$skill_dir"
            rm -rf "$skill_dir/autoresearch"

            # Copy skill files
            cp "$REPO_DIR/SKILL.md" "$skill_dir/SKILL.md"
            if [[ -d "$REPO_DIR/references" ]]; then
                mkdir -p "$skill_dir/references"
                cp -r "$REPO_DIR/references/"* "$skill_dir/references/" 2>/dev/null || true
            fi

            success "Codex skill installed to $skill_dir"
            echo '  Use: $autoresearch'
    else
        info "Skipping Codex skill install."
    fi
}

# ── Help ──────────────────────────────────────────────────────────────

show_help() {
    echo "autoresearch installer"
    echo ""
    echo "Usage: ./install.sh [options]"
    echo ""
    echo "Options:"
    echo "  -y, --yes                 Accept default prompts"
    echo "  --install-dir PATH        Binary install directory (default: ~/.local/bin)"
    echo "  --no-binary               Build but skip copying binary to PATH"
    echo "  --claude                  Install Claude Code plugin assets"
    echo "  --opencode                Install OpenCode assets"
    echo "  --codex                   Install Codex skill"
    echo "  --all                     Install all optional agent assets"
    echo "  --opencode-dir PATH       Override OpenCode config directory"
    echo "  --codex-dir PATH          Override Codex skill target directory"
    echo "  -h, --help                Show this help"
    echo ""
    echo "Without component flags, the installer runs as an interactive guided installer that:"
    echo "  1. Detects your OS (Linux, macOS)"
    echo "  2. Checks for Rust toolchain (offers to install via rustup)"
    echo "  3. Builds the release binary"
    echo "  4. Copies binary to ~/.local/bin/ (or custom path)"
    echo "  5. Optionally installs Claude Code plugin"
    echo "  6. Optionally installs OpenCode assets"
    echo "  7. Optionally installs Codex skill"
    echo ""
    echo "Requirements: bash, git, curl (for rustup install)"
    exit 0
}

# ── Main ──────────────────────────────────────────────────────────────

main() {
    parse_args "$@"

    echo -e "${BOLD}╭─────────────────────────────────────╮${NC}"
    echo -e "${BOLD}│   autoresearch installer v0.1.0     │${NC}"
    echo -e "${BOLD}╰─────────────────────────────────────╯${NC}"
    echo ""

    detect_os
    check_rust
    build_binary
    install_binary
    install_claude_plugin
    install_opencode_assets
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
