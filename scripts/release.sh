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
        sed -i "s/\"version\": \".*\"/\"version\": \"$version\"/g" "$path"
    fi
}

update_skill_version() {
    local path="$1"

    if [[ -f "$path" ]]; then
        sed -i "0,/^version: .*/s//version: $VERSION/" "$path"
    fi
}

# ── 1. Check clean worktree ─────────────────────────────────────────
if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
    echo "Error: working tree is dirty. Commit or stash changes first."
    exit 1
fi

# ── 2. Bump version in Cargo.toml ───────────────────────────────────
echo "[1/8] Bumping Cargo.toml to $VERSION..."
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" "$ROOT/Cargo.toml"

# ── 3. Bump agent package manifests ─────────────────────────────────
echo "[2/8] Bumping agent package manifests..."
update_json_version "$ROOT/.claude-plugin/plugin.json" "$VERSION"
update_json_version "$ROOT/.claude-plugin/marketplace.json" "$VERSION"
update_json_version "$ROOT/plugins/autoresearch/.codex-plugin/plugin.json" "$VERSION-codex.0"
update_skill_version "$ROOT/skills/autoresearch/SKILL.md"
update_skill_version "$ROOT/.agents/skills/autoresearch/SKILL.md"

# ── 4. Sync generated distributions ─────────────────────────────────
echo "[3/8] Syncing generated distributions..."
"$ROOT/scripts/transform.sh"

# ── 5. Run tests ────────────────────────────────────────────────────
echo "[4/8] Running tests..."
cargo test --manifest-path "$ROOT/Cargo.toml"

# ── 6. Run clippy ───────────────────────────────────────────────────
echo "[5/8] Running clippy..."
cargo clippy --manifest-path "$ROOT/Cargo.toml" -- -D warnings

# ── 7. Build release ────────────────────────────────────────────────
echo "[6/8] Building release binary..."
cargo build --manifest-path "$ROOT/Cargo.toml" --release

BINARY_SIZE=$(du -h "$ROOT/target/release/autoresearch" | cut -f1)
echo "  Binary size: $BINARY_SIZE"

# ── 8. Update changelog ────────────────────────────────────────────
echo "[7/8] Adding changelog entry..."
CHANGELOG="$ROOT/docs/changelog.md"
if [ -f "$CHANGELOG" ]; then
    DATE=$(date +%Y-%m-%d)
    # Insert new version header after the top-level header block
    sed -i "/^## \[/i \\
## [$VERSION] — $DATE\\
\\
### Changed\\
\\
- TODO: Fill in changes for this release\\
" "$CHANGELOG"
    echo "  Added placeholder entry for v$VERSION in docs/changelog.md"
    echo "  ⚠  Edit docs/changelog.md to fill in the actual changes before pushing."
fi

# ── 9. Commit and tag ──────────────────────────────────────────────
echo "[8/8] Committing and tagging..."
git -C "$ROOT" add \
    Cargo.toml \
    Cargo.lock \
    docs/changelog.md \
    .claude-plugin/plugin.json \
    .claude-plugin/marketplace.json \
    skills/autoresearch/SKILL.md \
    .opencode/skills/autoresearch/SKILL.md \
    .agents/skills/autoresearch/SKILL.md \
    plugins/autoresearch/.codex-plugin/plugin.json \
    plugins/autoresearch/skills/autoresearch
git -C "$ROOT" commit -m "release: v$VERSION"
git -C "$ROOT" tag -a "v$VERSION" -m "Release v$VERSION"

echo ""
echo "=== Release v$VERSION prepared ==="
echo ""
echo "Next steps:"
echo "  1. Review docs/changelog.md and fill in changes"
echo "  2. Amend the commit if needed: git commit --amend"
echo "  3. Push: git push origin main --tags"
echo "  4. Create GitHub release: gh release create v$VERSION --generate-notes"
echo "  5. Upload binary: gh release upload v$VERSION target/release/autoresearch"
