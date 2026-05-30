#!/usr/bin/env bash
# transform.sh — Generate OpenCode distribution assets from canonical sources.
#
# Copies commands/ and skills/ into the OpenCode naming conventions. The
# .agents Codex skill is maintained directly because it uses a different
# invocation model than the Claude/OpenCode command surface.
#
# Usage: ./scripts/transform.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── .opencode distribution ──────────────────────────────────────────

echo "Building .opencode/ distribution..."

adapt_opencode() {
    sed \
        -e 's|/autoresearch:plan|/autoresearch_plan|g' \
        -e 's|/autoresearch:debug|/autoresearch_debug|g' \
        -e 's|/autoresearch:fix|/autoresearch_fix|g' \
        -e 's|/autoresearch:security|/autoresearch_security|g' \
        -e 's|/autoresearch:ship|/autoresearch_ship|g' \
        -e 's|/autoresearch:scenario|/autoresearch_scenario|g' \
        -e 's|/autoresearch:predict|/autoresearch_predict|g' \
        -e 's|/autoresearch:learn|/autoresearch_learn|g' \
        -e 's|/autoresearch:reason|/autoresearch_reason|g' \
        -e 's|/autoresearch:probe|/autoresearch_probe|g' \
        -e 's|/autoresearch:evals|/autoresearch_evals|g' \
        -e 's|/autoresearch:improve|/autoresearch_improve|g' \
        -e 's|name: autoresearch:plan|name: autoresearch_plan|g' \
        -e 's|name: autoresearch:debug|name: autoresearch_debug|g' \
        -e 's|name: autoresearch:fix|name: autoresearch_fix|g' \
        -e 's|name: autoresearch:security|name: autoresearch_security|g' \
        -e 's|name: autoresearch:ship|name: autoresearch_ship|g' \
        -e 's|name: autoresearch:scenario|name: autoresearch_scenario|g' \
        -e 's|name: autoresearch:predict|name: autoresearch_predict|g' \
        -e 's|name: autoresearch:learn|name: autoresearch_learn|g' \
        -e 's|name: autoresearch:reason|name: autoresearch_reason|g' \
        -e 's|name: autoresearch:probe|name: autoresearch_probe|g' \
        -e 's|name: autoresearch:evals|name: autoresearch_evals|g' \
        -e 's|name: autoresearch:improve|name: autoresearch_improve|g' \
        -e 's|Bounded by default (500 iterations)|Bounded by default (25 iterations)|g' \
        -e 's|default: 500|default: 25|g' \
        -e 's|AskUserQuestion|question|g' \
        "$1"
}

# Commands: colon → underscore rename
rm -rf "$ROOT/.opencode/commands"
mkdir -p "$ROOT/.opencode/commands"
adapt_opencode "$ROOT/commands/autoresearch.md" > "$ROOT/.opencode/commands/autoresearch.md"

for f in "$ROOT/commands/autoresearch/"*.md; do
    stem="$(basename "$f" .md)"
    adapt_opencode "$f" > "$ROOT/.opencode/commands/autoresearch_${stem}.md"
done

# Skills
rm -rf "$ROOT/.opencode/skills/autoresearch"
mkdir -p "$ROOT/.opencode/skills"
mkdir -p "$ROOT/.opencode/skills/autoresearch"
adapt_opencode "$ROOT/skills/autoresearch/SKILL.md" > "$ROOT/.opencode/skills/autoresearch/SKILL.md"

opencode_count=$(find "$ROOT/.opencode" -type f | wc -l)

# ── .agents distribution ────────────────────────────────────────────

echo "Checking .agents/ distribution..."

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
echo "  .agents/skills/        — Generic agent skills (maintained directly)"
