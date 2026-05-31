# Guide

Autoresearch is a loop controller for agents: define a measurable goal, modify one thing, verify mechanically, keep or discard, and repeat.

## Core Commands

| Need | Use |
|------|-----|
| Improve a metric | `/autoresearch` or `$autoresearch` |
| Pick a metric from a vague goal | `/autoresearch:plan` or `$autoresearch plan` |
| Find a root cause | `/autoresearch:debug` or `$autoresearch debug` |
| Reduce errors to zero | `/autoresearch:fix` or `$autoresearch fix` |
| Run a security audit | `/autoresearch:security` or `$autoresearch security` |
| Ship through gates | `/autoresearch:ship` or `$autoresearch ship` |
| Analyze prior runs | `/autoresearch:evals` or `$autoresearch evals` |

## Binary Operations

The agent-facing protocols delegate stateful work to the `autoresearch` binary:

```bash
autoresearch init --verify "cat metric.txt" --direction lower
autoresearch verify --command "cat metric.txt"
autoresearch decide --decision auto --metric 4 --commit abc1234 --description "improved"
autoresearch status --summary
autoresearch progress
autoresearch cost --per-iteration-usd 0.25 --format json
autoresearch health --strict
autoresearch watch --lines 20 --format jsonl
autoresearch watch --websocket --websocket-addr 127.0.0.1:8765
autoresearch lessons --add "Prefer fixture-level assertions" --context "reduced flaky tests"
autoresearch search --from-state --provider-command 'exa "$AUTORESEARCH_SEARCH_QUERY"' --log
autoresearch parallel prepare --workers 3
autoresearch parallel run --manifest autoresearch-results/parallel-manifest.json --timeout-seconds 1200
autoresearch parallel template --workers 3 --output autoresearch-results/parallel-workers.json
autoresearch parallel compare --a "simplify parser" --b "cache scan results"
autoresearch parallel closeout --batch-file autoresearch-results/parallel-workers.json --merge-strategy cherry-pick
autoresearch parallel cleanup --manifest autoresearch-results/parallel-manifest.json
autoresearch evals --format json
autoresearch api --format json
autoresearch mcp serve
autoresearch mcp call --server-command "autoresearch mcp serve" --tool autoresearch_status
autoresearch scope expand --format json
autoresearch workspace exec --command "cargo test" --rollback-on-failure
autoresearch guard-presets --format json
autoresearch lessons --workspace-context --last 5
autoresearch plugin list
autoresearch plugin validate --path .autoresearch/plugins/example.toml
autoresearch plugin marketplace
autoresearch completions zsh > ~/.zfunc/_autoresearch
```

Use `autoresearch runtime run` for supervised background Codex sessions and `autoresearch runtime status` / `autoresearch runtime stop` for control.
Use `autoresearch status --summary` for compact monitor-friendly counters.
Use `autoresearch progress` for the current metric, trend, counters, escalation state, and terminal metric history sparkline.
Use `autoresearch cost --per-iteration-usd <usd>` or token/rate flags to estimate completed, remaining, and projected run spend.
Use `autoresearch watch --format <tsv|jsonl>` for human-readable tails or machine-readable JSON Lines.
Use `autoresearch watch --websocket --websocket-addr <host:port>` to serve snapshot and row update payloads to real-time dashboards. Add `--once` to print the initial WebSocket snapshot envelope without starting a server.
Use `autoresearch lessons --add <strategy> --context <note>` to append reusable lessons without editing `lessons.md` by hand.
Use `autoresearch search --from-state` with `--provider-command` or `AUTORESEARCH_SEARCH_CMD` to run cached, run-aware web searches. Add `--log` to append a `search` meta-iteration. When `decide` escalates to Web Search, it automatically runs the same cached helper with `AUTORESEARCH_SEARCH_CMD` and logs the result when timing/cooldown limits allow it.
Use `autoresearch parallel closeout --merge-strategy <cherry-pick|fast-forward|squash|rebase>` to select how the retained worker commit is merged.
Use `autoresearch parallel compare --a <hypothesis> --b <hypothesis>` to prepare a two-arm A/B batch that reuses `parallel run` and verified `parallel closeout`.
Use `autoresearch evals --format json` after parallel closeout to include worker improvement counts and a sign-test summary for parallel batches.
Use `autoresearch completions <bash|zsh|fish|elvish|powershell>` to generate shell completions.
Use `autoresearch manpages --output-dir man/man1` to generate a local `autoresearch.1` manual page.
Use `autoresearch config template --output .autoresearch.toml` to write a starter project defaults file.
Use `autoresearch config validate` to parse defaults, validate options, and screen configured commands without running them.
Use `autoresearch api --format json` to inspect the stable command/flag manifest and semver policy used by wrappers and agents.
Use `autoresearch mcp serve` as a stdio MCP server exposing read-only `autoresearch_status` and `autoresearch_watch_snapshot` tools.
Use `autoresearch mcp call --server-command <cmd> --tool <name> --arguments '{}'` to call a tool on an external stdio MCP server from an iteration script.
Use `autoresearch scope expand --format json` to resolve active primary and companion repo scopes, with package roots inferred from `Cargo.toml`, `package.json`, `pyproject.toml`, and `go.mod`.
Use `autoresearch workspace exec --command <cmd> --rollback-on-failure` to run one screened command across primary and companion repo targets, restoring attempted repos if any target fails.
Use `autoresearch guard-presets --format json` to suggest per-repo guard commands for primary and companion repositories.
Use `autoresearch lessons --workspace-context --last 5` from any managed repo to show the shared workspace lessons path and repo targets.
Use `autoresearch plugin list` and `autoresearch plugin validate --path <file>` to load local TOML mode plugin manifests with command safety screening.
Use `autoresearch plugin marketplace` to validate `.autoresearch/plugins/marketplace.toml` and every referenced community mode manifest before installing or sharing it.
Use `.github/actions/autoresearch` in GitHub Actions to run `exec` mode with a checked-in goal, scope, metric, and verify command.

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: ./.github/actions/autoresearch
    with:
      goal: Reduce lint failures
      scope: '["src/**/*.rs", "tests/**/*.rs"]'
      metric: lint failure count
      verify: cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -1
      direction: lower
      iterations: "3"
```

## Project Defaults

`autoresearch init` reads `.autoresearch.toml` from the workspace root when present.
CLI flags override file values.

```toml
goal = "Reduce failing tests"
scope = ["src/**/*.rs", "tests/**/*.rs"]
metric = "failing test count"
direction = "lower"
verify = "cargo test 2>&1 | tail -1"
guard = "cargo fmt -- --check"
iterations = 25
run_tag = "nightly"
```

Run with defaults:

```bash
autoresearch init
```

Generate a starter file:

```bash
autoresearch config template --output .autoresearch.toml
autoresearch config validate
```

## Run Artifacts

All run state lives under `autoresearch-results/`:

```text
results.tsv
state.json
context.json
lessons.md
handoff.json
launch.json
runtime.json
runtime.log
```

Do not commit `autoresearch-results/` or `.codex-autoresearch/`.

## Detailed Guides

- [Docs Index](README.md)
- [Getting Started](../guide/getting-started.md)
- [Examples](EXAMPLES.md)
- [System Architecture](system-architecture.md)
- [Project Changelog](project-changelog.md)
- [Core Loop](../guide/autoresearch.md)
- [Codex](../guide/autoresearch-codex.md)
- [Examples by Domain](../guide/examples-by-domain.md)
- [Chains & Combinations](../guide/chains-and-combinations.md)
- [Advanced Patterns](../guide/advanced-patterns.md)
- [Hooks](../guide/hooks.md)
- [Full Guide Index](../guide/README.md)
