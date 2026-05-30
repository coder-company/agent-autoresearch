#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT"

echo "==> cargo fmt -- --check"
cargo fmt -- --check

echo "==> cargo test"
cargo test

echo "==> bash -n install.sh"
bash -n install.sh

echo "==> scripts/transform.sh"
./scripts/transform.sh

echo "==> git diff --check"
git diff --check

if ! git diff --quiet -- .opencode .agents; then
    echo "Generated distribution files are out of sync. Run ./scripts/transform.sh and commit the result." >&2
    git status --short -- .opencode .agents >&2
    exit 1
fi

echo "Contributor gate passed."
