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
    agents/skill-openai.yaml
    .claude-plugin/marketplace.json
    .claude-plugin/plugin.json
    .agents/plugins/marketplace.json
    .agents/skills/autoresearch/agents/openai.yaml
    commands/autoresearch.md
    hooks/hooks.json
    install.sh
    scripts/run_contributor_gate.sh
    scripts/run_skill_e2e.sh
    scripts/transform.sh
    docs/README.md
    docs/INSTALL.md
    docs/GUIDE.md
    docs/EXAMPLES.md
    docs/system-architecture.md
    docs/project-changelog.md
    guide/README.md
    guide/autoresearch-codex.md
    references/core-principles.md
    references/runtime-protocol.md
    references/results-logging.md
    .agents/skills/autoresearch/SKILL.md
    .opencode/skills/autoresearch/SKILL.md
    plugins/autoresearch/.codex-plugin/plugin.json
    plugins/autoresearch/skills/autoresearch/SKILL.md
)

for path in "${required_paths[@]}"; do
    require_path "$path"
done

require_grep '^name: autoresearch$' SKILL.md
require_grep '^\s*display_name: "Autoresearch"' agents/openai.yaml
require_grep '^\s*allow_implicit_invocation:\s*false\s*$' agents/openai.yaml
require_grep '\$autoresearch' agents/openai.yaml
require_grep 'exec' agents/openai.yaml
require_grep '"\$schema": "https://anthropic.com/claude-code/marketplace.schema.json"' .claude-plugin/marketplace.json
require_grep '"source": "\."' .claude-plugin/marketplace.json
require_grep '"path": "\./plugins/autoresearch"' .agents/plugins/marketplace.json
require_grep '"installation": "AVAILABLE"' .agents/plugins/marketplace.json
cmp -s "$ROOT/agents/skill-openai.yaml" "$ROOT/.agents/skills/autoresearch/agents/openai.yaml" \
    || fail ".agents/skills/autoresearch/agents/openai.yaml drifted from agents/skill-openai.yaml"
if grep -Eq '^(name|description|model|tools):' "$ROOT/.agents/skills/autoresearch/agents/openai.yaml"; then
    fail ".agents/skills/autoresearch/agents/openai.yaml contains full tool-schema fields"
fi
cmp -s "$ROOT/.agents/skills/autoresearch/SKILL.md" "$ROOT/plugins/autoresearch/skills/autoresearch/SKILL.md" \
    || fail "plugins/autoresearch skill entrypoint drifted from .agents skill"
cmp -s "$ROOT/.agents/skills/autoresearch/agents/openai.yaml" "$ROOT/plugins/autoresearch/skills/autoresearch/agents/openai.yaml" \
    || fail "plugins/autoresearch agent metadata drifted from .agents skill"

require_grep '\$autoresearch' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch runtime run' .agents/skills/autoresearch/SKILL.md
require_grep 'exec-workflow\.md' .agents/skills/autoresearch/SKILL.md
require_grep 'runtime-hard-invariants\.md' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch watch' SKILL.md
require_grep 'autoresearch watch' skills/autoresearch/SKILL.md
require_grep 'autoresearch watch' guide/advanced-patterns.md
require_grep 'autoresearch parallel prepare' SKILL.md
require_grep 'autoresearch parallel run' SKILL.md
require_grep 'timeout-seconds' SKILL.md
require_grep 'autoresearch parallel cleanup' SKILL.md
require_grep 'autoresearch parallel prepare' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch parallel run' .agents/skills/autoresearch/SKILL.md
require_grep 'timeout-seconds' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch parallel cleanup' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch parallel prepare' docs/GUIDE.md
require_grep 'timeout-seconds' docs/GUIDE.md
require_grep 'autoresearch parallel run' guide/advanced-patterns.md
require_grep 'timeout-seconds' references/parallel-experiments-protocol.md
require_grep 'autoresearch parallel cleanup' references/parallel-experiments-protocol.md
require_grep '\$autoresearch exec' guide/autoresearch-codex.md
require_grep 'Claude Code, Codex, and OpenCode' README.md
require_grep 'companion-repo-scope' references/results-logging.md
require_grep 'companion-repo-scope' SKILL.md
require_grep 'companion-repo-scope' skills/autoresearch/SKILL.md
require_grep 'companion-repo-scope' .agents/skills/autoresearch/SKILL.md
require_grep 'results\.tsv.*\.prev' references/exec-workflow.md
require_grep 'lessons\.md.*read-only' references/exec-workflow.md
require_grep 'multi-repo-smoke' scripts/run_skill_e2e.sh
require_grep 'environment-summary' references/environment-awareness.md
require_grep 'environment-summary' commands/autoresearch.md
require_grep 'codex plugin marketplace add \.agents/plugins/marketplace\.json' docs/INSTALL.md
require_grep 'codex plugin marketplace add \.agents/plugins/marketplace\.json' guide/autoresearch-codex.md
require_grep '\-\-codex-plugin' docs/INSTALL.md
require_grep '\-\-codex-plugin' install.sh
require_grep '\-\-local' docs/INSTALL.md
require_grep '\-\-local' install.sh
require_grep 'ensure_safe_opencode_dir' install.sh
require_grep 'dangerously-bypass-approvals-and-sandbox' guide/autoresearch-codex.md
require_grep 'danger_full_access' guide/autoresearch-codex.md
require_grep 'plugins/autoresearch' docs/codebase-summary.md
require_grep 'out-of-scope writes' docs/architecture.md
require_grep 'plugins/autoresearch/skills/autoresearch' CONTRIBUTING.md
require_grep 'Codex plugin package' docs/system-architecture.md
require_grep 'Codex plugin package \+ local marketplace entry' docs/development-roadmap.md
require_grep 'local Codex plugin marketplace' COMPARISON.md
require_grep 'Structured metrics' COMPARISON.md

check_synced_reference_package "$ROOT/.agents/skills/autoresearch"
check_synced_reference_package "$ROOT/plugins/autoresearch/skills/autoresearch"
check_synced_reference_package "$ROOT/.opencode/skills/autoresearch"

echo "Distribution validation passed."
