#!/usr/bin/env bash
# transform.sh — Generate .opencode/ and .agents/ distributions from canonical sources.
#
# Copies commands/ and skills/ into distribution-specific directories with
# the required naming conventions.
#
# Usage: ./scripts/transform.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Cleanup previous builds ─────────────────────────────────────────
rm -rf "$ROOT/.opencode" "$ROOT/.agents"

# ── .opencode distribution ──────────────────────────────────────────

echo "Building .opencode/ distribution..."

# Commands: colon → underscore rename
mkdir -p "$ROOT/.opencode/commands"
cp "$ROOT/commands/autoresearch.md" "$ROOT/.opencode/commands/autoresearch.md"

mkdir -p "$ROOT/.opencode/commands/autoresearch"
for f in "$ROOT/commands/autoresearch/"*.md; do
    basename="$(basename "$f")"
    # Convert autoresearch:debug.md style references to autoresearch_debug.md
    # (the source files are already plain names, just copy)
    cp "$f" "$ROOT/.opencode/commands/autoresearch/$basename"
done

# Skills
mkdir -p "$ROOT/.opencode/skills"
cp -r "$ROOT/skills/autoresearch" "$ROOT/.opencode/skills/"

# References
mkdir -p "$ROOT/.opencode/references"
for ref in core-principles.md runtime-protocol.md runtime-hard-invariants.md \
           results-logging.md escalation.md modes.md structured-output-spec.md; do
    if [ -f "$ROOT/references/$ref" ]; then
        cp "$ROOT/references/$ref" "$ROOT/.opencode/references/"
    fi
done

opencode_count=$(find "$ROOT/.opencode" -type f | wc -l)

# ── .agents distribution ────────────────────────────────────────────

echo "Building .agents/ distribution..."

# Skills
mkdir -p "$ROOT/.agents/skills"
cp -r "$ROOT/skills/autoresearch" "$ROOT/.agents/skills/"

# References
mkdir -p "$ROOT/.agents/references"
for ref in core-principles.md runtime-protocol.md runtime-hard-invariants.md \
           results-logging.md escalation.md modes.md structured-output-spec.md; do
    if [ -f "$ROOT/references/$ref" ]; then
        cp "$ROOT/references/$ref" "$ROOT/.agents/references/"
    fi
done

agents_count=$(find "$ROOT/.agents" -type f | wc -l)

# ── Summary ─────────────────────────────────────────────────────────

echo ""
echo "=== Transform Complete ==="
echo ".opencode/  : $opencode_count files"
echo ".agents/    : $agents_count files"
echo ""
echo "Distributions:"
echo "  .opencode/commands/    — OpenCode command surface"
echo "  .opencode/skills/      — OpenCode skill definitions"
echo "  .opencode/references/  — Core protocol references"
echo "  .agents/skills/        — Generic agent skills"
echo "  .agents/references/    — Core protocol references"
