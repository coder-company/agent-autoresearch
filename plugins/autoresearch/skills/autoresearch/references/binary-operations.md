# Binary Operations

Use the `autoresearch` binary for stateful or mechanical work. Do not hand-edit run artifacts unless a recovery reference explicitly says to.

## Setup And Health

```bash
autoresearch init --verify "<cmd>" --direction <higher|lower>
autoresearch init --environment-summary auto --verify "<cmd>" --direction <higher|lower>
autoresearch init --companion-repo-scope ../frontend='src/**/*.ts' ...
autoresearch health --strict
autoresearch env --format json
autoresearch guard-presets --format json
autoresearch scope expand --format json
```

`init` creates `autoresearch-results/results.tsv`, `state.json`, `context.json`, and repo-local `.codex-autoresearch/pointer.json` files. For multi-repo runs, every companion repo needs a clean worktree and its own `--companion-repo-scope PATH=SCOPE`.

## Verify, Decide, And Log

```bash
autoresearch verify --command "<cmd>"
autoresearch verify --command "<cmd>" --repeat 3 --aggregate median
autoresearch verify --format metrics_json --key <metric> --command "<cmd>"
autoresearch guard --command "<cmd>"
autoresearch decide --decision auto --metric <value> --commit <sha> --description "<change>"
autoresearch log --iteration <n> --metric <value> --status blocked --description "<reason>"
```

Repeated verify supports scalar metrics. Use it for noisy checks and record the aggregate metric plus raw samples. For structured metrics, follow `references/results-logging.md`.

## Monitor And Analyze

```bash
autoresearch status --summary
autoresearch progress
autoresearch dashboard --once
autoresearch watch --lines 20 --format jsonl
autoresearch watch --websocket --websocket-addr 127.0.0.1:8765
autoresearch checkpoint --format json
autoresearch reanchor --format json
autoresearch evals --format json
autoresearch cost --per-iteration-usd 0.25 --format json
```

`watch --websocket` streams snapshot and row update payloads for WebSocket watch streams. `checkpoint` runs evals only when the active run reaches its configured or adaptive checkpoint interval.
`reanchor` reports whether the 10-iteration Protocol Fingerprint Check is due, verifies context paths, and prints the references to reload plus the `[RE-ANCHOR]` tag for the next TSV row when re-reading was needed.

## Background Runtime

```bash
autoresearch runtime run --cwd <workspace-root>
autoresearch runtime start --cwd <workspace-root>
autoresearch runtime status --cwd <workspace-root>
autoresearch runtime supervise --cwd <workspace-root>
autoresearch runtime stop --cwd <workspace-root>
```

Use `runtime run` for supervised background loops. Manual `start/status/supervise/stop` are control-plane tools for inspection and recovery.

## Parallel Experiments

```bash
autoresearch parallel prepare --workers 3
autoresearch parallel compare --a "<hypothesis A>" --b "<hypothesis B>"
autoresearch parallel run --manifest autoresearch-results/parallel-manifest.json --timeout-seconds 1200
autoresearch parallel closeout --batch-file autoresearch-results/parallel-workers.json --merge-strategy cherry-pick
autoresearch parallel closeout --batch-file autoresearch-results/parallel-workers.json --merge-strategy rebase
autoresearch parallel cleanup --manifest autoresearch-results/parallel-manifest.json
```

Parallel closeout is the only authoritative retention path for worker batches. It merges one candidate, re-runs verify and guard, logs worker audit rows, updates retained state once, then cleanup removes worktrees and branches.

## Integrations

```bash
autoresearch lessons --add "<strategy>" --context "<why>"
autoresearch lessons --workspace-context --last 5
autoresearch search --from-state --log
autoresearch mcp serve
autoresearch mcp call --server-command "<cmd>" --tool <name> --arguments '{}'
autoresearch workspace exec --command "<cmd>" --rollback-on-failure
autoresearch api --format json
```

Lessons preserve cross-run memory. Search escalation uses provider output as hypothesis input, not proof. MCP serve exposes read-only status/watch tools; MCP call invokes external stdio tools from scripts.
