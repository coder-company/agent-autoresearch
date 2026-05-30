#!/usr/bin/env bash
# transform.sh — Generate OpenCode distribution assets from canonical sources.
#
# Copies commands/ and skills/ into the OpenCode naming conventions. The
# .opencode/agents package is maintained directly for OpenCode subagents.
# The .agents Codex skill entrypoint is maintained directly because it uses
# a different invocation model, but its reference package is synced from the
# same canonical references/ directory as OpenCode.
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
        -e 's|AskUserQuestion|question|g' \
        "$1"
}

check_reference_links() {
    local base_dir="$1"
    shift

    local missing=0
    local file ref
    for file in "$@"; do
        [[ -f "$file" ]] || continue
        while IFS= read -r ref; do
            [[ -z "$ref" ]] && continue
            if [[ ! -f "$base_dir/$ref" ]]; then
                echo "Missing reference in ${file#"$ROOT"/}: $ref" >&2
                missing=1
            fi
        done < <(grep -Eho 'references/[A-Za-z0-9_.-]+\.md' "$file" | sort -u)
    done

    if [[ "$missing" -ne 0 ]]; then
        exit 1
    fi
}

check_reference_links "$ROOT" \
    "$ROOT/SKILL.md" \
    "$ROOT"/skills/autoresearch/SKILL.md \
    "$ROOT"/commands/autoresearch.md \
    "$ROOT"/commands/autoresearch/*.md \
    "$ROOT"/references/*.md

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
mkdir -p "$ROOT/.opencode/skills/autoresearch/references"
for f in "$ROOT/references/"*.md; do
    adapt_opencode "$f" > "$ROOT/.opencode/skills/autoresearch/references/$(basename "$f")"
done

check_reference_links "$ROOT/.opencode/skills/autoresearch" \
    "$ROOT"/.opencode/skills/autoresearch/SKILL.md \
    "$ROOT"/.opencode/skills/autoresearch/references/*.md

opencode_count=$(find "$ROOT/.opencode" -type f | wc -l)

# ── .agents distribution ────────────────────────────────────────────

echo "Building .agents/ reference package..."

rm -rf "$ROOT/.agents/skills/autoresearch/references"
mkdir -p "$ROOT/.agents/skills/autoresearch/references"
for f in "$ROOT/references/"*.md; do
    cp "$f" "$ROOT/.agents/skills/autoresearch/references/$(basename "$f")"
done
mkdir -p "$ROOT/.agents/skills/autoresearch/agents"
cp "$ROOT/agents/skill-openai.yaml" "$ROOT/.agents/skills/autoresearch/agents/openai.yaml"

check_reference_links "$ROOT/.agents/skills/autoresearch" \
    "$ROOT"/.agents/skills/autoresearch/SKILL.md \
    "$ROOT"/.agents/skills/autoresearch/references/*.md

agents_count=$(find "$ROOT/.agents" -type f | wc -l)

# ── Codex plugin package ─────────────────────────────────────────────

echo "Building plugins/autoresearch skill package..."

rm -rf "$ROOT/plugins/autoresearch/skills/autoresearch"
mkdir -p "$ROOT/plugins/autoresearch/skills"
cp -R "$ROOT/.agents/skills/autoresearch" "$ROOT/plugins/autoresearch/skills/autoresearch"

plugin_count=$(find "$ROOT/plugins/autoresearch" -type f | wc -l)

# ── Summary ─────────────────────────────────────────────────────────

echo ""
echo "=== Transform Complete ==="
echo ".opencode/  : $opencode_count files"
echo ".agents/    : $agents_count files"
echo "plugin      : $plugin_count files"
echo ""
echo "Distributions:"
echo "  .opencode/commands/    — OpenCode command surface"
echo "  .opencode/skills/      — OpenCode skill definitions"
echo "  .opencode/agents/      — OpenCode helper subagents"
echo "  .agents/skills/        — Generic agent skills (maintained directly)"
echo "  plugins/autoresearch/  — Codex plugin package"
