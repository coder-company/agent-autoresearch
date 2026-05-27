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

# ── 1. Check clean worktree ─────────────────────────────────────────
if ! git -C "$ROOT" diff --quiet HEAD 2>/dev/null; then
    echo "Error: working tree is dirty. Commit or stash changes first."
    exit 1
fi

# ── 2. Bump version in Cargo.toml ───────────────────────────────────
echo "[1/6] Bumping Cargo.toml to $VERSION..."
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" "$ROOT/Cargo.toml"

# ── 3. Run tests ────────────────────────────────────────────────────
echo "[2/6] Running tests..."
cargo test --manifest-path "$ROOT/Cargo.toml"

# ── 4. Run clippy ───────────────────────────────────────────────────
echo "[3/6] Running clippy..."
cargo clippy --manifest-path "$ROOT/Cargo.toml" -- -D warnings

# ── 5. Build release ────────────────────────────────────────────────
echo "[4/6] Building release binary..."
cargo build --manifest-path "$ROOT/Cargo.toml" --release

BINARY_SIZE=$(du -h "$ROOT/target/release/autoresearch" | cut -f1)
echo "  Binary size: $BINARY_SIZE"

# ── 6. Update changelog ────────────────────────────────────────────
echo "[5/6] Adding changelog entry..."
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

# ── 7. Commit and tag ──────────────────────────────────────────────
echo "[6/6] Committing and tagging..."
git -C "$ROOT" add Cargo.toml Cargo.lock docs/changelog.md
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
