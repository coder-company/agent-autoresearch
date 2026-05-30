#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

fail() {
    echo "distribution validation failed: $*" >&2
    exit 1
}

require_path() {
    local path="$1"
    [[ -e "$ROOT/$path" ]] || fail "missing $path"
}

require_grep() {
    local pattern="$1"
    local path="$2"
    grep -Eq "$pattern" "$ROOT/$path" || fail "$path does not match /$pattern/"
}

check_reference_links() {
    local package_root="$1"
    shift

    local file ref
    for file in "$@"; do
        [[ -f "$file" ]] || continue
        while IFS= read -r ref; do
            [[ -f "$package_root/$ref" ]] || fail "${file#"$ROOT"/} links to missing $ref"
        done < <(grep -Eho 'references/[A-Za-z0-9_.-]+\.md' "$file" | sort -u)
    done
}

check_synced_reference_package() {
    local package_root="$1"
    local packaged_ref canonical

    check_reference_links "$package_root" \
        "$package_root/SKILL.md" \
        "$package_root"/references/*.md

    for packaged_ref in "$package_root"/references/*.md; do
        [[ -f "$packaged_ref" ]] || continue
        canonical="$ROOT/references/$(basename "$packaged_ref")"
        [[ -f "$canonical" ]] || fail "${packaged_ref#"$ROOT"/} has no canonical reference"
        cmp -s "$canonical" "$packaged_ref" || fail "${packaged_ref#"$ROOT"/} drifted from references/$(basename "$packaged_ref")"
    done
}

required_paths=(
    README.md
    CONTRIBUTING.md
    SKILL.md
    agents/openai.yaml
    commands/autoresearch.md
    hooks/hooks.json
    install.sh
    scripts/run_contributor_gate.sh
    scripts/transform.sh
    guide/README.md
    guide/autoresearch-codex.md
    references/core-principles.md
    references/runtime-protocol.md
    references/results-logging.md
    .agents/skills/autoresearch/SKILL.md
    .opencode/skills/autoresearch/SKILL.md
)

for path in "${required_paths[@]}"; do
    require_path "$path"
done

require_grep '^name: autoresearch$' SKILL.md
require_grep '^\s*display_name: "Autoresearch"' agents/openai.yaml
require_grep '^\s*allow_implicit_invocation:\s*false\s*$' agents/openai.yaml
require_grep '\$autoresearch' agents/openai.yaml
require_grep 'exec' agents/openai.yaml

require_grep '\$autoresearch' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch runtime run' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch watch' SKILL.md
require_grep 'autoresearch watch' skills/autoresearch/SKILL.md
require_grep 'autoresearch watch' guide/advanced-patterns.md

check_synced_reference_package "$ROOT/.agents/skills/autoresearch"
check_synced_reference_package "$ROOT/.opencode/skills/autoresearch"

echo "Distribution validation passed."
