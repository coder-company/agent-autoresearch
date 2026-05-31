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
    CONTEXT.md
    SKILL.md
    agents/openai.yaml
    agents/skill-openai.yaml
    .claude-plugin/marketplace.json
    .claude-plugin/plugin.json
    .agents/plugins/marketplace.json
    .agents/skills/autoresearch/agents/openai.yaml
    .claude/commands/autoresearch/evals.md
    .claude/skills/autoresearch/references/core-principles.md
    .claude/skills/autoresearch/references/runtime-protocol.md
    .opencode/agents/docs-manager.md
    commands/autoresearch.md
    hooks/hooks.json
    install.sh
    scripts/run_contributor_gate.sh
    scripts/run_skill_e2e.sh
    scripts/transform.sh
    packaging/homebrew/README.md
    packaging/homebrew/autoresearch.rb.template
    tests/test-hooks.sh
    .github/workflows/ci.yml
    .github/workflows/docs.yml
    .github/workflows/release.yml
    book.toml
    docs/SUMMARY.md
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
cmp -s "$ROOT/skills/autoresearch/SKILL.md" "$ROOT/.claude/skills/autoresearch/SKILL.md" \
    || fail ".claude skill entrypoint drifted from skills/autoresearch/SKILL.md"
for command in "$ROOT"/commands/autoresearch.md "$ROOT"/commands/autoresearch/*.md; do
    relative="${command#"$ROOT"/commands/}"
    cmp -s "$command" "$ROOT/.claude/commands/$relative" \
        || fail ".claude command $relative drifted from commands/$relative"
done

require_grep '\$autoresearch' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch runtime run' .agents/skills/autoresearch/SKILL.md
require_grep 'exec-workflow\.md' .agents/skills/autoresearch/SKILL.md
require_grep 'runtime-hard-invariants\.md' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch health --strict' SKILL.md
require_grep 'autoresearch health --strict' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch status --summary' README.md
require_grep 'autoresearch status --summary' docs/GUIDE.md
require_grep 'autoresearch status --summary' SKILL.md
require_grep 'autoresearch watch --lines 20 --format jsonl' SKILL.md
require_grep 'autoresearch watch --lines 20 --format jsonl' skills/autoresearch/SKILL.md
require_grep 'autoresearch watch --websocket' README.md
require_grep 'autoresearch watch --websocket' docs/GUIDE.md
require_grep 'WebSocket watch streams' .agents/skills/autoresearch/SKILL.md
require_grep 'WebSocket watch streams' plugins/autoresearch/skills/autoresearch/SKILL.md
require_grep 'Progress websocket for real-time monitoring' docs/development-roadmap.md
require_grep 'autoresearch watch' guide/advanced-patterns.md
require_grep 'format jsonl' README.md
require_grep 'format jsonl' docs/GUIDE.md
require_grep 'autoresearch lessons --add' README.md
require_grep 'autoresearch lessons --add' docs/GUIDE.md
require_grep 'autoresearch lessons --add' SKILL.md
require_grep 'autoresearch lessons --add' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch search --from-state --log' README.md
require_grep 'autoresearch search --from-state' docs/GUIDE.md
require_grep 'autoresearch search --from-state --log' SKILL.md
require_grep 'autoresearch search --from-state --log' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch mcp serve' README.md
require_grep 'autoresearch mcp serve' docs/GUIDE.md
require_grep 'autoresearch mcp serve' SKILL.md
require_grep 'autoresearch mcp serve' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch mcp call' README.md
require_grep 'autoresearch mcp call' docs/GUIDE.md
require_grep 'mcp call --server-command' SKILL.md
require_grep 'mcp call --server-command' .agents/skills/autoresearch/SKILL.md
require_grep 'MCP tool server mode' docs/development-roadmap.md
require_grep 'MCP client mode' docs/development-roadmap.md
require_grep 'search.*meta-row' references/web-search-protocol.md
require_grep 'AUTORESEARCH_SEARCH_CMD' references/web-search-protocol.md
require_grep 'auto_search\.status' references/web-search-protocol.md
require_grep '\[x\] Built-in web search escalation \(configurable provider command\)' docs/development-roadmap.md
require_grep '\[x\] Search result caching to avoid redundant queries' docs/development-roadmap.md
require_grep 'autoresearch parallel prepare' SKILL.md
require_grep 'autoresearch parallel run' SKILL.md
require_grep 'timeout-seconds' SKILL.md
require_grep 'merge-strategy' SKILL.md
require_grep 'rebase' SKILL.md
require_grep 'autoresearch parallel cleanup' SKILL.md
require_grep 'autoresearch parallel prepare' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch parallel run' .agents/skills/autoresearch/SKILL.md
require_grep 'timeout-seconds' .agents/skills/autoresearch/SKILL.md
require_grep 'merge-strategy' .agents/skills/autoresearch/SKILL.md
require_grep 'rebase' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch parallel cleanup' .agents/skills/autoresearch/SKILL.md
require_grep 'autoresearch parallel prepare' docs/GUIDE.md
require_grep 'autoresearch health --strict' docs/GUIDE.md
require_grep 'merge-strategy' docs/GUIDE.md
require_grep 'rebase' docs/GUIDE.md
require_grep 'timeout-seconds' docs/GUIDE.md
require_grep 'autoresearch parallel run' guide/advanced-patterns.md
require_grep 'merge-strategy' guide/advanced-patterns.md
require_grep 'merge-strategy' references/parallel-experiments-protocol.md
require_grep '\[x\] Branch merge strategy selection \(fast-forward, squash, rebase\)' docs/development-roadmap.md
require_grep '\[x\] Improved evals: statistical significance testing on parallel results' docs/development-roadmap.md
require_grep 'parallel worker significance' README.md
require_grep 'sign-test summary' docs/GUIDE.md
require_grep 'timeout-seconds' references/parallel-experiments-protocol.md
require_grep 'autoresearch parallel cleanup' references/parallel-experiments-protocol.md
require_grep 'autoresearch completions' README.md
require_grep 'autoresearch completions' docs/GUIDE.md
require_grep 'autoresearch completions zsh' docs/INSTALL.md
require_grep 'Shell completions \(bash, zsh, fish, elvish, PowerShell\)' docs/development-roadmap.md
require_grep 'autoresearch manpages --output-dir' README.md
require_grep 'autoresearch manpages --output-dir' docs/GUIDE.md
require_grep 'autoresearch manpages --output-dir' docs/INSTALL.md
require_grep 'Man pages generation' docs/development-roadmap.md
require_grep 'autoresearch api --format json' README.md
require_grep 'autoresearch api --format json' docs/GUIDE.md
require_grep 'Stable CLI API — semver guarantees' docs/development-roadmap.md
require_grep 'autoresearch scope expand --format json' README.md
require_grep 'autoresearch scope expand --format json' docs/GUIDE.md
require_grep 'Workspace-aware scope expansion \(monorepo package boundaries\)' docs/development-roadmap.md
require_grep 'autoresearch guard-presets --format json' README.md
require_grep 'autoresearch guard-presets --format json' docs/GUIDE.md
require_grep 'Cross-repo guard command presets' docs/development-roadmap.md
require_grep 'autoresearch lessons --workspace-context --last 5' README.md
require_grep 'autoresearch lessons --workspace-context --last 5' docs/GUIDE.md
require_grep 'Shared lessons across repos in a workspace' docs/development-roadmap.md
require_grep 'autoresearch plugin list' README.md
require_grep 'autoresearch plugin validate --path' docs/GUIDE.md
require_grep 'autoresearch plugin marketplace' README.md
require_grep 'autoresearch plugin marketplace' docs/GUIDE.md
require_grep 'Plugin system — loadable mode definitions' docs/development-roadmap.md
require_grep 'Plugin marketplace — community-contributed modes' docs/development-roadmap.md
require_grep '\.autoresearch\.toml' README.md
require_grep '\.autoresearch\.toml' docs/GUIDE.md
require_grep 'autoresearch config template' README.md
require_grep 'autoresearch config template' docs/GUIDE.md
require_grep 'autoresearch config template' docs/INSTALL.md
require_grep 'autoresearch config validate' README.md
require_grep 'autoresearch config validate' docs/GUIDE.md
require_grep 'autoresearch config validate' docs/INSTALL.md
require_grep 'Configuration file \(`\.autoresearch\.toml`\) for project-level defaults' docs/development-roadmap.md
require_grep '\$autoresearch exec' guide/autoresearch-codex.md
require_grep 'Claude Code, Codex, and OpenCode' README.md
require_grep '13 command protocols · 11 native hooks · background runtime · parallel verified closeout' README.md
require_grep 'Companion Repo' CONTEXT.md
require_grep 'Structured Metrics' CONTEXT.md
require_grep 'Runtime Snapshot' CONTEXT.md
require_grep '^name: docs-manager$' .opencode/agents/docs-manager.md
for i18n_readme in "$ROOT"/docs/i18n/README_*.md; do
    grep -q '/autoresearch:improve' "$i18n_readme" \
        || fail "${i18n_readme#"$ROOT"/} is missing /autoresearch:improve"
    grep -q -- '--opencode' "$i18n_readme" \
        || fail "${i18n_readme#"$ROOT"/} is missing OpenCode install instructions"
    ! grep -Eq '12 (comandos|команд|Befehle|commandes)|12個すべてのコマンド|12개 명령어|12 个命令|12 .*command' "$i18n_readme" \
        || fail "${i18n_readme#"$ROOT"/} still advertises 12 commands"
done
require_grep 'companion-repo-scope' references/results-logging.md
require_grep 'companion-repo-scope' SKILL.md
require_grep 'companion-repo-scope' skills/autoresearch/SKILL.md
require_grep 'companion-repo-scope' .agents/skills/autoresearch/SKILL.md
require_grep 'results\.tsv.*\.prev' references/exec-workflow.md
require_grep 'lessons\.md.*read-only' references/exec-workflow.md
require_grep 'multi-repo-smoke' scripts/run_skill_e2e.sh
require_grep 'runtime-smoke' scripts/run_skill_e2e.sh
require_grep 'parallel-smoke' scripts/run_skill_e2e.sh
require_grep 'runtime stop requested' scripts/run_skill_e2e.sh
require_grep 'parallel cleanup' scripts/run_skill_e2e.sh
require_grep 'cargo build --manifest-path "\$ROOT/Cargo.toml" >/dev/null' scripts/run_skill_e2e.sh
require_grep 'environment-summary' references/environment-awareness.md
require_grep 'environment-summary' commands/autoresearch.md
require_grep 'codex plugin marketplace add \.agents/plugins/marketplace\.json' docs/INSTALL.md
require_grep 'codex plugin marketplace add \.agents/plugins/marketplace\.json' guide/autoresearch-codex.md
require_grep '\.claude/commands' README.md
require_grep '\.claude/skills/autoresearch' README.md
require_grep '\.claude/commands' docs/INSTALL.md
require_grep '\.claude/skills/autoresearch' docs/INSTALL.md
require_grep '\-\-codex-plugin' docs/INSTALL.md
require_grep '\-\-codex-plugin' install.sh
require_grep '\-\-local' docs/INSTALL.md
require_grep '\-\-local' install.sh
require_grep 'ensure_safe_opencode_dir' install.sh
require_grep '\.opencode/agents/' scripts/transform.sh
require_grep 'dangerously-bypass-approvals-and-sandbox' guide/autoresearch-codex.md
require_grep 'danger_full_access' guide/autoresearch-codex.md
require_grep 'plugins/autoresearch' docs/codebase-summary.md
require_grep 'docs-manager' docs/codebase-summary.md
require_grep 'runtime, parallel, screen, and hooks' docs/codebase-summary.md
require_grep 'dev_rules_reminder\.rs' docs/codebase-summary.md
require_grep 'out-of-scope writes' docs/architecture.md
require_grep 'tool_name' tests/test-hooks.sh
require_grep '\.decision // "allow"' tests/test-hooks.sh
require_grep '\./tests/test-hooks\.sh' CONTRIBUTING.md
require_grep 'runtime-smoke --clean' CONTRIBUTING.md
require_grep 'parallel-smoke --clean' CONTRIBUTING.md
require_grep 'runtime-smoke --clean' docs/INSTALL.md
require_grep 'parallel-smoke --clean' docs/INSTALL.md
require_grep 'runtime-smoke --clean' docs/code-standards.md
require_grep 'parallel-smoke --clean' docs/code-standards.md
require_grep 'plugins/autoresearch/skills/autoresearch' CONTRIBUTING.md
require_grep '\.claude/commands/' CONTRIBUTING.md
require_grep '\.claude/skills/autoresearch/' CONTRIBUTING.md
require_grep '\.opencode/' CONTRIBUTING.md
require_grep '\./scripts/release\.sh <version>' CONTRIBUTING.md
require_grep 'Codex plugin package' docs/system-architecture.md
require_grep 'runtime run/start/status/supervise/stop' docs/project-overview-pdr.md
require_grep 'parallel prepare/run/closeout/cleanup' docs/project-overview-pdr.md
require_grep 'Hook system reference \(11 hooks' guide/README.md
require_grep 'Codex plugin package \+ local marketplace entry' docs/development-roadmap.md
require_grep 'Companion repo registration through `--companion-repo-scope PATH=SCOPE`' docs/development-roadmap.md
require_grep 'Claude marketplace/plugin' COMPARISON.md
require_grep '13-command surface' COMPARISON.md
require_grep 'Structured metrics' COMPARISON.md
require_grep 'Claude, Codex, and OpenCode packages' COMPARISON.md
require_grep '\.agents/skills/autoresearch/' AGENTS.md
require_grep 'docs-manager helper agent' AGENTS.md
require_grep '\.agents/skills/autoresearch/' docs/changelog.md
require_grep 'hidden `docs-manager` helper agent' docs/changelog.md
require_grep 'MAX_RELEASE_BINARY_BYTES=\$\(\(5 \* 1024 \* 1024\)\)' scripts/run_contributor_gate.sh
require_grep 'MAX_RELEASE_BINARY_BYTES=\$\(\(5 \* 1024 \* 1024\)\)' scripts/release.sh
require_grep 'cargo fmt --manifest-path "\$ROOT/Cargo.toml" -- --check' scripts/release.sh
require_grep '"\$ROOT/scripts/validate_distribution\.sh"' scripts/release.sh
require_grep '\[10/10\] Committing and tagging' scripts/release.sh
require_grep 'git -C "\$ROOT" log --format=' scripts/release.sh
require_grep 'update_cargo_version "\$ROOT/Cargo.toml" "\$VERSION"' scripts/release.sh
require_grep 'linux-aarch64' .github/workflows/release.yml
require_grep 'macos-15-intel' .github/workflows/release.yml
require_grep 'macos-aarch64' .github/workflows/release.yml
require_grep 'windows-x86_64' .github/workflows/release.yml
require_grep 'gh release upload "\$TAG" --clobber' .github/workflows/release.yml
require_grep '\[package.metadata.binstall\]' Cargo.toml
require_grep '\{ name \}-v\{ version \}-\{ target \}\.tar\.gz' Cargo.toml
require_grep 'class Autoresearch < Formula' packaging/homebrew/autoresearch.rb.template
require_grep 'aarch64-unknown-linux-gnu' packaging/homebrew/autoresearch.rb.template
require_grep 'Homebrew formula and cargo-binstall support' docs/development-roadmap.md
require_grep 'Pre-built binaries for Linux \(x86_64, aarch64\), macOS \(x86_64, aarch64\), Windows' docs/development-roadmap.md
require_grep 'Tagged releases publish `\.tar\.gz` archives' docs/INSTALL.md
require_grep 'cargo binstall autoresearch' docs/INSTALL.md
require_grep 'src = "docs"' book.toml
require_grep '\[Installation\]\(INSTALL\.md\)' docs/SUMMARY.md
require_grep 'actions/deploy-pages@v4' .github/workflows/docs.yml
require_grep 'Comprehensive documentation site' docs/development-roadmap.md
require_grep '^book/$' .gitignore
if grep -q 'sed -i' "$ROOT/scripts/release.sh"; then
    fail "release script still uses non-portable sed -i"
fi
if grep -q 'mapfile' "$ROOT/scripts/release.sh"; then
    fail "release script still uses non-portable mapfile"
fi
require_grep 'workflow_dispatch:' .github/workflows/ci.yml
require_grep 'timeout-minutes: 25' .github/workflows/ci.yml
require_grep 'actions/cache@v4' .github/workflows/ci.yml
require_grep 'tests/\*\.sh' .github/workflows/ci.yml
require_grep 'bash -n "\$script"' .github/workflows/ci.yml
require_grep 'for script in install\.sh scripts/\*\.sh tests/\*\.sh' scripts/run_contributor_gate.sh
if grep -R -E '2[,.]5 ?M(B|o|Б)' "$ROOT/AGENTS.md" "$ROOT/CONTRIBUTING.md" "$ROOT/docs" "$ROOT/scripts/release.md" >/dev/null; then
    fail "docs still advertise the old 2.5MB binary size"
fi
if grep -q 'TODO: Fill in changes for this release' "$ROOT/scripts/release.sh"; then
    fail "release script still writes placeholder changelog entries"
fi

check_synced_reference_package "$ROOT/.agents/skills/autoresearch"
check_synced_reference_package "$ROOT/plugins/autoresearch/skills/autoresearch"
check_synced_reference_package "$ROOT/.opencode/skills/autoresearch"
check_synced_reference_package "$ROOT/.claude/skills/autoresearch"

echo "Distribution validation passed."
