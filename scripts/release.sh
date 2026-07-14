#!/usr/bin/env bash
# release.sh — Automate version bump, build, test, tag, and changelog update.
#
# Usage: ./scripts/release.sh <version>
#   e.g. ./scripts/release.sh 0.2.0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ $# -ne 1 ]; then
    echo "Usage: $0 <version>"
    echo "  e.g. $0 0.2.0"
    exit 1
fi

VERSION="$1"

# Validate version format
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "Error: version must be semver (e.g., 0.2.0)"
    exit 1
fi

echo "=== Releasing v$VERSION ==="
echo ""

update_json_version() {
    local path="$1"
    local version="$2"

    if [[ -f "$path" ]]; then
        local tmp
        tmp="$(mktemp)"
        awk -v version="$version" '{ gsub(/"version": "[^"]*"/, "\"version\": \"" version "\""); print }' "$path" > "$tmp"
        mv "$tmp" "$path"
    fi
}

update_skill_version() {
    local path="$1"

    if [[ -f "$path" ]]; then
        local tmp
        tmp="$(mktemp)"
        awk -v version="$VERSION" '
            BEGIN { replaced = 0 }
            !replaced && /^version: / {
                print "version: " version
                replaced = 1
                next
            }
            { print }
        ' "$path" > "$tmp"
        mv "$tmp" "$path"
    fi
}

update_cargo_version() {
    local path="$1"
    local version="$2"
    local tmp

    tmp="$(mktemp)"
    awk -v version="$version" '
        BEGIN { replaced = 0 }
        !replaced && /^version = "/ {
            print "version = \"" version "\""
            replaced = 1
            next
        }
        { print }
    ' "$path" > "$tmp"
    mv "$tmp" "$path"
}

collect_change_lines() {
    local range_args=("$@")
    local line

    CHANGE_LINES=()
    while IFS= read -r line; do
        CHANGE_LINES+=("$line")
    done < <(git -C "$ROOT" log --format='- %s' "${range_args[@]}")
}

# ── 1. Check clean worktree ─────────────────────────────────────────
if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
    echo "Error: working tree is dirty. Commit or stash changes first."
    exit 1
fi

# ── 2. Bump version in Cargo.toml ───────────────────────────────────
echo "[1/10] Bumping Cargo.toml to $VERSION..."
update_cargo_version "$ROOT/Cargo.toml" "$VERSION"

# ── 3. Bump agent package manifests ─────────────────────────────────
echo "[2/10] Bumping agent package manifests..."
update_json_version "$ROOT/.claude-plugin/plugin.json" "$VERSION"
update_json_version "$ROOT/.claude-plugin/marketplace.json" "$VERSION"
update_json_version "$ROOT/plugins/autoresearch/.codex-plugin/plugin.json" "$VERSION-codex.0"
update_json_version "$ROOT/package.json" "$VERSION"
update_skill_version "$ROOT/skills/autoresearch/SKILL.md"
update_skill_version "$ROOT/.agents/skills/autoresearch/SKILL.md"
update_skill_version "$ROOT/integrations/pi/skills/autoresearch/SKILL.md"

# ── 4. Sync generated distributions ─────────────────────────────────
echo "[3/10] Syncing generated distributions..."
"$ROOT/scripts/transform.sh"

# ── 5. Validate distributions ───────────────────────────────────────
echo "[4/10] Validating distributions..."
"$ROOT/scripts/validate_distribution.sh"

# ── 6. Run format check + tests ─────────────────────────────────────
echo "[5/10] Checking formatting..."
cargo fmt --manifest-path "$ROOT/Cargo.toml" -- --check

echo "[6/10] Running tests..."
cargo test --manifest-path "$ROOT/Cargo.toml"

# ── 7. Run clippy ───────────────────────────────────────────────────
echo "[7/10] Running clippy..."
cargo clippy --manifest-path "$ROOT/Cargo.toml" -- -D warnings

# ── 8. Build release ────────────────────────────────────────────────
echo "[8/10] Building release binary..."
cargo build --manifest-path "$ROOT/Cargo.toml" --release

MAX_RELEASE_BINARY_BYTES=$((5 * 1024 * 1024))
RELEASE_BINARY="$ROOT/target/release/autoresearch"
RELEASE_BINARY_BYTES=$(wc -c < "$RELEASE_BINARY" | tr -d '[:space:]')
if [[ "$RELEASE_BINARY_BYTES" -gt "$MAX_RELEASE_BINARY_BYTES" ]]; then
    echo "Error: release binary is too large: ${RELEASE_BINARY_BYTES} bytes (limit: ${MAX_RELEASE_BINARY_BYTES})"
    exit 1
fi

BINARY_SIZE=$(du -h "$ROOT/target/release/autoresearch" | cut -f1)
echo "  Binary size: $BINARY_SIZE"

# ── 9. Update changelog ────────────────────────────────────────────
echo "[9/10] Adding changelog entry..."
CHANGELOG="$ROOT/docs/changelog.md"
if [ -f "$CHANGELOG" ]; then
    DATE=$(date +%Y-%m-%d)

    LATEST_TAG=$(git -C "$ROOT" describe --tags --abbrev=0 2>/dev/null || true)
    if [[ -n "$LATEST_TAG" ]]; then
        collect_change_lines "${LATEST_TAG}..HEAD"
    else
        collect_change_lines --max-count=20
    fi
    if [[ ${#CHANGE_LINES[@]} -eq 0 ]]; then
        CHANGE_LINES=("- Release v$VERSION")
    fi

    TMP_CHANGELOG=$(mktemp)
    INSERTED=0
    while IFS= read -r line; do
        if [[ "$INSERTED" -eq 0 && "$line" == "## ["* ]]; then
            {
                printf '## [%s] — %s\n\n' "$VERSION" "$DATE"
                printf '### Changed\n\n'
                printf '%s\n' "${CHANGE_LINES[@]}"
                printf '\n'
            } >> "$TMP_CHANGELOG"
            INSERTED=1
        fi
        printf '%s\n' "$line" >> "$TMP_CHANGELOG"
    done < "$CHANGELOG"
    if [[ "$INSERTED" -eq 0 ]]; then
        {
            printf '\n## [%s] — %s\n\n' "$VERSION" "$DATE"
            printf '### Changed\n\n'
            printf '%s\n' "${CHANGE_LINES[@]}"
        } >> "$TMP_CHANGELOG"
    fi
    mv "$TMP_CHANGELOG" "$CHANGELOG"
    echo "  Added changelog entry for v$VERSION from recent commit subjects."
fi

# ── 10. Commit and tag ─────────────────────────────────────────────
echo "[10/10] Committing and tagging..."
git -C "$ROOT" add \
    Cargo.toml \
    Cargo.lock \
    docs/changelog.md \
    .claude-plugin/plugin.json \
    .claude-plugin/marketplace.json \
    skills/autoresearch/SKILL.md \
    .opencode/skills/autoresearch/SKILL.md \
    .claude/skills/autoresearch/SKILL.md \
    .agents/skills/autoresearch/SKILL.md \
    plugins/autoresearch/.codex-plugin/plugin.json \
    plugins/autoresearch/skills/autoresearch \
    package.json \
    integrations/pi/skills/autoresearch
git -C "$ROOT" commit -m "release: v$VERSION"
git -C "$ROOT" tag -a "v$VERSION" -m "Release v$VERSION"

echo ""
echo "=== Release v$VERSION prepared ==="
echo ""
echo "Next steps:"
echo "  1. Review generated docs/changelog.md notes"
echo "  2. Amend the commit if needed: git commit --amend"
echo "  3. Push: git push origin main --tags"
echo "  4. Confirm the Release workflow published checksummed binary archives for v$VERSION"
echo "  5. Render packaging/homebrew/autoresearch.rb.template with release SHA-256 values"
