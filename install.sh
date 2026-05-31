#!/usr/bin/env bash
set -euo pipefail

# ── Autoresearch Installer ────────────────────────────────────────────
# Builds the Rust binary and optionally installs for Claude Code, OpenCode,
# or Codex.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --claude
#   ./install.sh                # Interactive guided install
#   ./install.sh --help         # Show usage
# ──────────────────────────────────────────────────────────────────────

LAUNCH_DIR="$(pwd)"
SCRIPT_SOURCE="${BASH_SOURCE[0]:-$0}"
if [[ -n "$SCRIPT_SOURCE" && -f "$SCRIPT_SOURCE" ]]; then
    REPO_DIR="$(cd "$(dirname "$SCRIPT_SOURCE")" && pwd)"
else
    REPO_DIR=""
fi
DEFAULT_INSTALL_DIR="$HOME/.local/bin"
INSTALL_REPO="${AUTORESEARCH_INSTALL_REPO:-coder-company/agent-autoresearch}"
INSTALL_REF="${AUTORESEARCH_INSTALL_REF:-main}"
INSTALL_ARCHIVE_URL="${AUTORESEARCH_INSTALL_ARCHIVE_URL:-https://github.com/${INSTALL_REPO}/archive/refs/heads/${INSTALL_REF}.tar.gz}"

cleanup_bootstrap_tmp() {
    if [[ -n "${AUTORESEARCH_BOOTSTRAP_TMP_DIR:-}" && -d "$AUTORESEARCH_BOOTSTRAP_TMP_DIR" ]]; then
        rm -rf "$AUTORESEARCH_BOOTSTRAP_TMP_DIR"
    fi
}

if [[ -n "${AUTORESEARCH_BOOTSTRAP_TMP_DIR:-}" ]]; then
    trap cleanup_bootstrap_tmp EXIT
fi

ASSUME_YES=0
INSTALL_BINARY=1
INSTALL_DIR="$DEFAULT_INSTALL_DIR"
INSTALL_SCOPE="global"
INSTALL_SCOPE_SET=0
COMPONENT_FLAGS_SET=0
INSTALL_CLAUDE=0
INSTALL_OPENCODE=0
INSTALL_CODEX=0
INSTALL_CODEX_PLUGIN=0
INSTALL_VSCODE=0
OPENCODE_DIR=""
CODEX_SKILL_DIR=""
VSCODE_EXTENSION_DIR=""

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

is_source_tree() {
    [[ -n "$REPO_DIR" && -f "$REPO_DIR/Cargo.toml" && -f "$REPO_DIR/install.sh" && -d "$REPO_DIR/src" ]]
}

bootstrap_source_tree() {
    if is_source_tree; then
        return 0
    fi

    header "Fetching autoresearch source"
    info "Installer was not launched from a source checkout."
    info "Downloading $INSTALL_ARCHIVE_URL"

    for command_name in curl find mktemp tar; do
        if ! command -v "$command_name" &>/dev/null; then
            err "Remote install requires $command_name."
            err "Install dependencies or clone https://github.com/$INSTALL_REPO and run ./install.sh."
            exit 1
        fi
    done

    local tmp_dir archive source_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/autoresearch-install.XXXXXX")"
    archive="$tmp_dir/source.tar.gz"

    curl -fsSL "$INSTALL_ARCHIVE_URL" -o "$archive"
    tar -xzf "$archive" -C "$tmp_dir"

    source_dir="$(find "$tmp_dir" -mindepth 1 -maxdepth 2 -type f -name Cargo.toml -print -quit)"
    if [[ -z "$source_dir" ]]; then
        err "Downloaded archive did not contain Cargo.toml."
        exit 1
    fi
    source_dir="$(dirname "$source_dir")"
    if [[ ! -f "$source_dir/install.sh" ]]; then
        err "Downloaded archive did not contain install.sh."
        exit 1
    fi

    success "Source ready in $source_dir"
    export AUTORESEARCH_BOOTSTRAP_TMP_DIR="$tmp_dir"
    exec bash "$source_dir/install.sh" "$@"
}

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
            -g|--global)
                if [[ "$INSTALL_SCOPE_SET" -eq 1 && "$INSTALL_SCOPE" != "global" ]]; then
                    err "Choose either --global or --local, not both."
                    exit 1
                fi
                INSTALL_SCOPE_SET=1
                INSTALL_SCOPE="global"
                ;;
            -l|--local)
                if [[ "$INSTALL_SCOPE_SET" -eq 1 && "$INSTALL_SCOPE" != "local" ]]; then
                    err "Choose either --global or --local, not both."
                    exit 1
                fi
                INSTALL_SCOPE_SET=1
                INSTALL_SCOPE="local"
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
            --codex-plugin)
                COMPONENT_FLAGS_SET=1
                INSTALL_CODEX_PLUGIN=1
                ;;
            --vscode)
                COMPONENT_FLAGS_SET=1
                INSTALL_VSCODE=1
                ;;
            --all)
                COMPONENT_FLAGS_SET=1
                INSTALL_CLAUDE=1
                INSTALL_OPENCODE=1
                INSTALL_CODEX=1
                INSTALL_CODEX_PLUGIN=1
                INSTALL_VSCODE=1
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
            --vscode-dir)
                shift
                [[ $# -gt 0 ]] || { err "--vscode-dir requires a path"; exit 1; }
                VSCODE_EXTENSION_DIR="${1/#\~/$HOME}"
                ;;
            --vscode-dir=*)
                VSCODE_EXTENSION_DIR="${1#*=}"
                VSCODE_EXTENSION_DIR="${VSCODE_EXTENSION_DIR/#\~/$HOME}"
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

ensure_safe_codex_skill_dir() {
    local dir="${1%/}"

    if [[ -z "$dir" ]]; then
        err "Refusing empty Codex skill path."
        return 1
    fi

    case "$dir" in
        "/"|"$HOME"|"$HOME/.codex"|"$HOME/.codex/skills")
            err "Refusing unsafe Codex skill path: $dir"
            return 1
            ;;
    esac

    if [[ "${dir##*/}" != "autoresearch" ]]; then
        err "Refusing Codex skill path that does not end in autoresearch: $dir"
        return 1
    fi
}

ensure_safe_opencode_dir() {
    local dir="${1%/}"

    if [[ -z "$dir" ]]; then
        err "Refusing empty OpenCode config path."
        return 1
    fi

    case "$dir" in
        "/"|"$HOME"|"$HOME/.config"|"$HOME/.opencode/skills"|"$HOME/.config/opencode/skills")
            err "Refusing unsafe OpenCode config path: $dir"
            return 1
            ;;
    esac
}

ensure_safe_vscode_extension_dir() {
    local dir="${1%/}"

    if [[ -z "$dir" ]]; then
        err "Refusing empty VS Code extensions path."
        return 1
    fi

    case "$dir" in
        "/"|"$HOME"|"$HOME/.vscode"|"$HOME/.vscode-insiders"|"$HOME/.config"|"$HOME/Library")
            err "Refusing unsafe VS Code extensions path: $dir"
            return 1
            ;;
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

    local answer="Y"
    if [[ "$ASSUME_YES" -eq 0 ]]; then
        read -rp "  Install Rust now via rustup? [Y/n] " answer
    fi
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

    cargo build --manifest-path "$REPO_DIR/Cargo.toml" --release 2>&1

    local binary="$REPO_DIR/target/release/autoresearch"
    if [[ ! -f "$binary" ]]; then
        err "Build failed: binary not found at $binary"
        exit 1
    fi

    # The Claude hook config calls bin/autoresearch. Keep that file as a
    # tracked wrapper so plugin installs are portable across platforms.
    if [[ -f "$REPO_DIR/bin/autoresearch" ]]; then
        chmod +x "$REPO_DIR/bin/autoresearch"
    fi

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
    if [[ ! -x "$REPO_DIR/bin/autoresearch" ]]; then
        err "Missing executable bin/autoresearch wrapper — cannot install plugin hooks."
        return 1
    fi

    if component_enabled "CLAUDE" "  Install Claude Code plugin? [Y/n] "; then
            if command -v claude &>/dev/null; then
                info "Installing local plugin via claude CLI..."
                claude plugin add "$REPO_DIR" || {
                    warn "Local install failed. If the binary is on PATH, you can install the remote plugin instead:"
                    echo "    claude plugin add coder-company/agent-autoresearch"
                }
            else
                info "Claude CLI not found. Install the plugin manually:"
                echo ""
                echo "    claude plugin add $REPO_DIR"
                echo ""
                echo "  Or, if the binary is already on PATH, from GitHub:"
                echo ""
                echo "    claude plugin add coder-company/agent-autoresearch"
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
            elif [[ "$INSTALL_SCOPE" == "local" ]]; then
                target_root="$LAUNCH_DIR/.opencode"
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

            ensure_safe_opencode_dir "$opencode_dir"
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
            elif [[ "$INSTALL_SCOPE" == "local" ]]; then
                target_dir="$LAUNCH_DIR/.codex/skills/autoresearch"
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

            ensure_safe_codex_skill_dir "$skill_dir"
            rm -rf "$skill_dir"
            mkdir -p "$skill_dir"

            # Copy the maintained Codex skill distribution when available.
            if [[ -d "$REPO_DIR/.agents/skills/autoresearch" ]]; then
                cp -R "$REPO_DIR/.agents/skills/autoresearch/." "$skill_dir/"
            else
                cp "$REPO_DIR/SKILL.md" "$skill_dir/SKILL.md"
                if [[ -d "$REPO_DIR/references" ]]; then
                    mkdir -p "$skill_dir/references"
                    cp -r "$REPO_DIR/references/"* "$skill_dir/references/" 2>/dev/null || true
                fi
            fi

            success "Codex skill installed to $skill_dir"
            echo '  Use: $autoresearch'
    else
        info "Skipping Codex skill install."
    fi
}

install_codex_plugin_package() {
    header "Codex Plugin Package"

    if [[ ! -f "$REPO_DIR/.agents/plugins/marketplace.json" ]]; then
        err "Missing .agents/plugins/marketplace.json — cannot install Codex plugin package."
        return 1
    fi
    if [[ ! -f "$REPO_DIR/plugins/autoresearch/.codex-plugin/plugin.json" ]]; then
        err "Missing plugins/autoresearch/.codex-plugin/plugin.json — cannot install Codex plugin package."
        return 1
    fi

    if component_enabled "CODEX_PLUGIN" "  Install Codex plugin package? [Y/n] "; then
            if command -v codex &>/dev/null; then
                info "Registering local Codex marketplace..."
                codex plugin marketplace add "$REPO_DIR/.agents/plugins/marketplace.json" || {
                    warn "Marketplace add failed. You can run it manually:"
                    echo "    codex plugin marketplace add $REPO_DIR/.agents/plugins/marketplace.json"
                }
                info "Installing autoresearch plugin..."
                codex plugin install autoresearch@autoresearch-local || {
                    warn "Plugin install failed. You can run it manually after adding the marketplace:"
                    echo "    codex plugin install autoresearch@autoresearch-local"
                }
            else
                info "Codex CLI not found. Install the plugin manually:"
                echo ""
                echo "    codex plugin marketplace add $REPO_DIR/.agents/plugins/marketplace.json"
                echo "    codex plugin install autoresearch@autoresearch-local"
            fi
            echo ""
            echo '  Use: $autoresearch'
    else
        info "Skipping Codex plugin package install."
    fi
}

# ── VS Code Extension ─────────────────────────────────────────────────

install_vscode_extension() {
    header "VS Code Extension"

    if [[ ! -f "$REPO_DIR/integrations/vscode/package.json" ]]; then
        err "Missing integrations/vscode/package.json — cannot install VS Code extension."
        return 1
    fi
    if [[ ! -f "$REPO_DIR/integrations/vscode/extension.js" ]]; then
        err "Missing integrations/vscode/extension.js — cannot install VS Code extension."
        return 1
    fi

    if component_enabled "VSCODE" "  Install VS Code extension? [Y/n] "; then
            local target_root
            if [[ -n "$VSCODE_EXTENSION_DIR" ]]; then
                target_root="$VSCODE_EXTENSION_DIR"
            elif [[ -n "${VSCODE_EXTENSIONS:-}" ]]; then
                target_root="$VSCODE_EXTENSIONS"
            else
                target_root="$HOME/.vscode/extensions"
            fi

            local extension_root="$target_root"
            if [[ "$ASSUME_YES" -eq 0 ]]; then
                read -rp "  VS Code extensions path [$target_root]: " extension_root
                extension_root="${extension_root:-$target_root}"
            fi

            ensure_safe_vscode_extension_dir "$extension_root"

            local package_json="$REPO_DIR/integrations/vscode/package.json"
            local publisher name version
            publisher=$(sed -n 's/^[[:space:]]*"publisher"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$package_json" | head -n1)
            name=$(sed -n 's/^[[:space:]]*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$package_json" | head -n1)
            version=$(sed -n 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$package_json" | head -n1)
            publisher="${publisher:-coder-company}"
            name="${name:-autoresearch}"
            version="${version:-0.0.0}"

            local extension_dir="$extension_root/$publisher.$name-$version"
            mkdir -p "$extension_root"
            rm -rf "$extension_dir"
            mkdir -p "$extension_dir"
            cp -R "$REPO_DIR/integrations/vscode/." "$extension_dir/"

            success "VS Code extension installed to $extension_dir"
            echo "  Reload VS Code, then run: Autoresearch: Show Status"
    else
        info "Skipping VS Code extension install."
    fi
}

# ── Help ──────────────────────────────────────────────────────────────

show_help() {
    echo "autoresearch installer"
    echo ""
    echo "Usage: ./install.sh [options]"
    echo ""
    echo "Remote one-liner:"
    echo "  curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --claude"
    echo ""
    echo "Remote install environment:"
    echo "  AUTORESEARCH_INSTALL_REF=main        Branch name to download (default: main)"
    echo "  AUTORESEARCH_INSTALL_REPO=owner/repo GitHub repository to download"
    echo "  AUTORESEARCH_INSTALL_ARCHIVE_URL=URL Full source archive URL override"
    echo ""
    echo "Options:"
    echo "  -y, --yes                 Accept default prompts"
    echo "  --install-dir PATH        Binary install directory (default: ~/.local/bin)"
    echo "  --no-binary               Build but skip copying binary to PATH"
    echo "  -g, --global              Install copy-based agent assets globally (default)"
    echo "  -l, --local               Install OpenCode/Codex assets into the current project"
    echo "  --claude                  Install Claude Code plugin assets"
    echo "  --opencode                Install OpenCode assets"
    echo "  --codex                   Install Codex skill"
    echo "  --codex-plugin            Install local Codex plugin package"
    echo "  --vscode                  Install VS Code extension package"
    echo "  --all                     Install all optional agent assets"
    echo "  --opencode-dir PATH       Override OpenCode config directory"
    echo "  --codex-dir PATH          Override Codex skill target directory"
    echo "  --vscode-dir PATH         Override VS Code extensions directory"
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
    echo "  8. Optionally installs Codex plugin package"
    echo "  9. Optionally installs VS Code extension"
    echo ""
    echo "Requirements: bash, git, curl (for rustup install)"
    exit 0
}

# ── Main ──────────────────────────────────────────────────────────────

main() {
    bootstrap_source_tree "$@"
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
    install_codex_plugin_package
    install_vscode_extension

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
