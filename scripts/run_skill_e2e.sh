#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MODE="${1:-}"
KEEP_TEMP=1

usage() {
    cat <<'EOF'
Usage:
  ./scripts/run_skill_e2e.sh binary-smoke [--clean]
  ./scripts/run_skill_e2e.sh multi-repo-smoke [--clean]
  ./scripts/run_skill_e2e.sh runtime-smoke [--clean]
  ./scripts/run_skill_e2e.sh parallel-smoke [--clean]

Modes:
  binary-smoke  Create a disposable git repo and exercise init, decide, status,
                watch, and evals through the autoresearch binary.
  multi-repo-smoke
                Create primary + companion repos and exercise companion init,
                health, handoff, and runtime launch metadata.
  runtime-smoke
                Create a disposable git repo, start a fake detached Codex
                runtime, verify status artifacts, and stop it.
  parallel-smoke
                Create a disposable git repo, prepare parallel worktrees, run
                fake Codex workers, verify manifest updates, and clean up.

Flags:
  --clean       Delete the temp repo after a successful run.
EOF
}

if [[ -z "$MODE" ]]; then
    usage
    exit 1
fi
shift || true

while [[ $# -gt 0 ]]; do
    case "$1" in
        --clean)
            KEEP_TEMP=0
            ;;
        *)
            echo "Unknown flag: $1" >&2
            usage
            exit 1
            ;;
    esac
    shift
done

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required tool: $1" >&2
        exit 1
    fi
}

autoresearch_bin() {
    if [[ -n "${AUTORESEARCH_BIN:-}" ]]; then
        printf '%s\n' "$AUTORESEARCH_BIN"
        return
    fi

    local bin="$ROOT/target/debug/autoresearch"
    cargo build --manifest-path "$ROOT/Cargo.toml" >/dev/null
    printf '%s\n' "$bin"
}

init_fixture_repo() {
    local repo="$1"
    mkdir -p "$repo"
    git -C "$repo" init -b main >/dev/null
    git -C "$repo" config user.name e2e-bot
    git -C "$repo" config user.email e2e@example.com
    printf 'autoresearch-results/\n.codex-autoresearch/\n' > "$repo/.gitignore"
    printf '5\n' > "$repo/metric.txt"
    git -C "$repo" add .
    git -C "$repo" commit -m "baseline" >/dev/null
}

write_sleeping_fake_codex() {
    local path="$1"
    cat > "$path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" != "exec" ]]; then
    echo "expected codex exec" >&2
    exit 64
fi
shift
while [[ $# -gt 0 ]]; do
    shift
done
cat >/dev/null
sleep 30
EOF
    chmod +x "$path"
}

write_exiting_fake_codex() {
    local path="$1"
    cat > "$path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" != "exec" ]]; then
    echo "expected codex exec" >&2
    exit 64
fi
shift
while [[ $# -gt 0 ]]; do
    shift
done
cat >/dev/null
exit 0
EOF
    chmod +x "$path"
}

cleanup_if_requested() {
    local tmpdir="$1"
    if [[ "$KEEP_TEMP" -eq 0 ]]; then
        rm -rf "$tmpdir"
    else
        echo "Temp repo kept at: $tmpdir"
    fi
}

run_binary_smoke() {
    require_tool git

    local bin tmpdir repo trial_commit watch_output evals_output
    bin="$(autoresearch_bin)"
    tmpdir="$(mktemp -d)"
    repo="$tmpdir/repo"

    init_fixture_repo "$repo"

    "$bin" init \
        --verify "cat metric.txt" \
        --direction lower \
        --goal "Reduce marker count" \
        --scope metric.txt \
        --cwd "$repo" >/dev/null

    printf '4\n' > "$repo/metric.txt"
    git -C "$repo" add metric.txt
    git -C "$repo" commit -m "experiment: reduce marker count" >/dev/null
    trial_commit="$(git -C "$repo" rev-parse --short HEAD)"

    "$bin" decide \
        --decision auto \
        --metric 4 \
        --commit "$trial_commit" \
        --description "reduced marker count" \
        --cwd "$repo" >/dev/null

    "$bin" status --cwd "$repo" >/dev/null
    watch_output="$("$bin" watch --once --lines 2 --cwd "$repo")"
    grep -q 'reduced marker count' <<<"$watch_output"
    evals_output="$(cd "$repo" && "$bin" evals --format json)"
    grep -q '"keeps": 1' <<<"$evals_output"
    grep -q $'1\t' "$repo/autoresearch-results/results.tsv"
    grep -q 'reduced marker count' "$repo/autoresearch-results/results.tsv"

    echo "binary smoke: OK"
    cleanup_if_requested "$tmpdir"
}

run_multi_repo_smoke() {
    require_tool git

    local bin tmpdir primary companion health_output
    bin="$(autoresearch_bin)"
    tmpdir="$(mktemp -d)"
    primary="$tmpdir/primary"
    companion="$tmpdir/frontend"

    init_fixture_repo "$primary"
    init_fixture_repo "$companion"
    mkdir -p "$companion/pkg"
    printf 'pub fn helper() {}\n' > "$companion/pkg/helper.rs"
    git -C "$companion" add pkg/helper.rs
    git -C "$companion" commit -m "add helper" >/dev/null

    "$bin" init \
        --verify "cat metric.txt" \
        --direction higher \
        --goal "Exercise multi-repo metadata" \
        --scope "src/**/*.rs" \
        --run-mode background \
        --workspace-root "$primary" \
        --primary-repo "$primary" \
        --companion-repo-scope "$companion=pkg/**/*.rs" \
        --cwd "$primary" >/dev/null

    grep -q '"repo_targets"' "$primary/autoresearch-results/context.json"
    grep -Fq "$companion" "$primary/autoresearch-results/context.json"
    test -f "$companion/.codex-autoresearch/pointer.json"

    health_output="$("$bin" health --verify "cat metric.txt" --min-free-mb 1 --cwd "$primary")"
    grep -q '"decision": "ok"' <<<"$health_output"

    "$bin" handoff \
        --source loop \
        --status COMPLETE \
        --config '{"goal":"multi","scope":["src/**/*.rs"],"metric":"score","direction":"higher","verify":"cat metric.txt"}' \
        --cwd "$primary" >/dev/null
    grep -q '"repo_targets"' "$primary/autoresearch-results/handoff.json"
    grep -Fq "$companion" "$primary/autoresearch-results/handoff.json"

    "$bin" runtime start \
        --dry-run \
        --execution-policy workspace_write \
        --codex-bin codex \
        --cwd "$primary" >/dev/null
    grep -q '"repo_targets"' "$primary/autoresearch-results/launch.json"
    grep -Fq "$companion" "$primary/autoresearch-results/launch.json"
    grep -Fq 'scope=pkg/**/*.rs' "$primary/autoresearch-results/launch.json"

    echo "multi-repo smoke: OK"
    cleanup_if_requested "$tmpdir"
}

run_runtime_smoke() {
    require_tool git

    local bin tmpdir repo fake_codex start_output status_output stop_output
    bin="$(autoresearch_bin)"
    tmpdir="$(mktemp -d)"
    repo="$tmpdir/repo"
    fake_codex="$tmpdir/fake-codex"

    init_fixture_repo "$repo"
    write_sleeping_fake_codex "$fake_codex"

    "$bin" init \
        --verify "cat metric.txt" \
        --direction lower \
        --goal "Exercise background runtime control" \
        --scope metric.txt \
        --run-mode background \
        --cwd "$repo" >/dev/null

    start_output="$("$bin" runtime start \
        --execution-policy workspace_write \
        --codex-bin "$fake_codex" \
        --cwd "$repo")"
    grep -q '"status": "ok"' <<<"$start_output"
    grep -q '"status": "running"' "$repo/autoresearch-results/runtime.json"
    grep -q "\"codex_bin\": \"$fake_codex\"" "$repo/autoresearch-results/launch.json"

    status_output="$("$bin" runtime status --cwd "$repo")"
    grep -q '"status": "running"' <<<"$status_output"

    stop_output="$("$bin" runtime stop --cwd "$repo")"
    grep -q '"status": "stopped"' <<<"$stop_output"
    grep -q 'runtime stop requested' "$repo/autoresearch-results/runtime.log"

    echo "runtime smoke: OK"
    cleanup_if_requested "$tmpdir"
}

run_parallel_smoke() {
    require_tool git

    local bin tmpdir repo fake_codex prepare_output run_output cleanup_output manifest
    bin="$(autoresearch_bin)"
    tmpdir="$(mktemp -d)"
    repo="$tmpdir/repo"
    fake_codex="$tmpdir/fake-codex"
    manifest="autoresearch-results/parallel-manifest.json"

    init_fixture_repo "$repo"
    write_exiting_fake_codex "$fake_codex"

    "$bin" init \
        --verify "cat metric.txt" \
        --direction lower \
        --goal "Exercise parallel worker execution" \
        --scope metric.txt \
        --cwd "$repo" >/dev/null

    prepare_output="$("$bin" parallel prepare \
        --workers 2 \
        --branch-prefix "autoresearch/e2e" \
        --cwd "$repo")"
    grep -q '"status": "ok"' <<<"$prepare_output"
    grep -q '"worker_id": "a"' "$repo/$manifest"
    test -d "$repo/autoresearch-results/parallel-worktrees/iteration-1/worker-a"

    run_output="$("$bin" parallel run \
        --manifest "$manifest" \
        --execution-policy workspace_write \
        --codex-bin "$fake_codex" \
        --timeout-seconds 5 \
        --cwd "$repo")"
    grep -q '"status": "ok"' <<<"$run_output"
    grep -q '"worker_runs"' "$repo/$manifest"
    grep -q '"status": "completed"' "$repo/$manifest"

    cleanup_output="$("$bin" parallel cleanup \
        --manifest "$manifest" \
        --cwd "$repo")"
    grep -q '"status": "ok"' <<<"$cleanup_output"
    grep -q '"status": "cleaned"' "$repo/$manifest"
    test ! -d "$repo/autoresearch-results/parallel-worktrees/iteration-1/worker-a"
    ! git -C "$repo" show-ref --verify --quiet refs/heads/autoresearch/e2e-1-a

    echo "parallel smoke: OK"
    cleanup_if_requested "$tmpdir"
}

case "$MODE" in
    binary-smoke)
        run_binary_smoke
        ;;
    multi-repo-smoke)
        run_multi_repo_smoke
        ;;
    runtime-smoke)
        run_runtime_smoke
        ;;
    parallel-smoke)
        run_parallel_smoke
        ;;
    *)
        echo "Unknown mode: $MODE" >&2
        usage
        exit 1
        ;;
esac
