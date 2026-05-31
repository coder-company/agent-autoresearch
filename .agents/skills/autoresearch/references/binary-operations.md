# Binary Operations

Use the `autoresearch` binary for stateful or mechanical work. Do not hand-edit run artifacts unless a recovery reference explicitly says to.

## Setup And Health

```bash
autoresearch init --verify "<cmd>" --direction <higher|lower>
autoresearch init --environment-summary auto --verify "<cmd>" --direction <higher|lower>
autoresearch init --companion-repo-scope ../frontend='src/**/*.ts' ...
autoresearch plan --goal "reduce any types" --format json --debug
autoresearch debug --symptom "API returns 500" --scope "src/**/*.rs" --depth deep --iterations 12 --severity high
autoresearch fix --target "npx tsc --noEmit" --scope "src/**/*.ts" --category type --iterations 7 --learn --evals
autoresearch improve --goal "Improve onboarding activation" --icp "Developer tools teams" --depth deep --iterations 24 --seeds 5 --evals --learn
autoresearch prd --title "Improve onboarding" --problem "New users stall before first run"
autoresearch security --scope "src/**/*.rs" --focus auth --depth deep --iterations 18 --diff --evals
autoresearch ship --target "Release v1.2.0" --type code-release --dry-run --monitor 15 --learn
autoresearch scenario --target "Checkout flow" --domain web --format threat-scenarios --scope "src/checkout/**" --iterations 16 --debug
autoresearch predict --proposal "Add cache warming to search results" --scope "src/search/**" --debug
autoresearch reason --question "Should we replace the storage layer" --mode debate --domain software --iterations 11 --judges 7 --convergence 4 --predict
autoresearch probe --subject "Payment retry workflow" --scope "src/payments/**" --iterations 9 --plan
autoresearch learn --mode summarize --scope "src/**/*.rs" --depth comprehensive --iterations 14 --evals
autoresearch health --strict
autoresearch env --format json
autoresearch guard-presets --format json
autoresearch scope expand --format json
```

`init` creates `autoresearch-results/results.tsv`, `state.json`, `context.json`, and repo-local `.codex-autoresearch/pointer.json` files. For multi-repo runs, every companion repo needs a clean worktree and its own `--companion-repo-scope PATH=SCOPE`.
`plan` scans repo tooling and returns a suggested scope, metric, direction, verify, guard, and iteration count without starting a run.
`plan --debug` writes `autoresearch-results/plan/handoff.json` with the derived config for downstream debug; `plan --chain <targets>` still records explicit downstream commands.
Native artifact generators default to ignored `autoresearch-results/<mode>/` paths. Use explicit output flags only for intentional non-default locations.
`debug` writes a hypothesis-driven investigation bundle with summary, findings, eliminated hypotheses, debug-results TSV, and handoff JSON.
`debug --fix` records `fix` as the next handoff target. `debug --chain <targets>` records comma-separated downstream targets plus eval propagation metadata when requested.
`debug --depth <level> --iterations <n> --severity <level>` records investigation budget and severity filter metadata in the summary and handoff.
`fix --iterations <n>` writes a repair-plan artifact bundle under `autoresearch-results/fix` with priority order, fix-results TSV, iteration budget, and handoff JSON.
`fix --from-debug` imports the latest debug handoff scope, symptom, and finding count before writing the repair-plan bundle.
`fix --learn --evals` records downstream learn handoff and checkpoint propagation metadata in handoff JSON; `fix --chain <targets>` still records explicit downstream targets.
`improve` writes a product-improvement artifact bundle with research findings, ranked plan, summary, improve-results TSV, and handoff JSON.
`improve --depth <level> --iterations <n> --evals` records active category count, iteration budget, and checkpoint metadata in the research bundle.
`improve --seeds <n> --discover|--no-discover --learn` records seed volume, discovery posture, and downstream learn handoff metadata; `improve --chain <targets>` still records explicit downstream targets.
`prd` writes a focused improve-mode markdown artifact with DECISION NEEDED markers, acceptance criteria, risks, success metrics, and a ready-to-run autoresearch config block.
`security` writes a STRIDE + OWASP audit artifact bundle: overview, threat model, attack surface map, coverage, findings, recommendations, results TSV, and handoff JSON.
`security --fail-on <severity> --fix` records the CI gate threshold, confirmed finding count, and downstream fix target metadata.
`security --depth <level> --iterations <n> --diff --fix --evals` records audit budget, delta mode, downstream fix handoff, and checkpoint propagation metadata.
`ship` writes an 8-phase checklist, summary, ship log TSV, and handoff JSON without performing external ship actions.
`ship --auto --force --rollback --monitor <minutes> --learn` records approval posture, rollback intent, monitoring window, and downstream learn handoff metadata without external side effects.
`scenario` writes a markdown artifact covering all 12 scenario dimensions for the requested target, domain, format, focus, and implementation scope.
`scenario --domain <domain> --depth <level> --iterations <n> --evals --debug` records domain, exploration budget, checkpoint metadata, and downstream debug handoff context.
`predict` writes a five-persona pre-implementation review with architecture, security, performance, UX, and adversarial findings.
`predict --depth <level> --adversarial --fail-on <severity>` records review profile, finding budget, incremental mode, and CI gate metadata.
`predict --debug` writes a sidecar handoff for downstream investigation; `predict --chain <targets>` still records comma-separated targets and eval propagation metadata when requested.
`reason` writes an adversarial debate artifact with candidate solutions, blind judge rubric, and convergence criteria.
`reason --predict` writes a sidecar handoff for downstream review; `reason --chain <targets>` still records debate context and comma-separated downstream targets.
`reason --iterations <n> --judges <n> --convergence <n> --judge-personas <list> --temperature <value>` records debate budget, panel size, stopping threshold, synthesis behavior, and generation hints.
`probe` writes eight persona-driven requirement questions, constraint slots, and the saturation rule used to decide when enough constraints have been found.
`probe --mode autonomous --depth <level> --iterations <n> --adversarial` records interrogation profile, persona count, rounds, and saturation threshold metadata.
`probe --plan` writes a sidecar handoff for downstream planning; `probe --chain <targets>` still records constraint context and comma-separated downstream targets.
`learn --depth <level> --iterations <n>` writes documentation summary artifacts with depth, iteration budget, validation report, learn-results TSV, and handoff JSON.
`learn --file <path> --depth <level> --topics <list> --no-fix --chain <targets>` records documentation profile, validation behavior, and downstream handoff metadata.

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
autoresearch evals --file autoresearch-results/results.tsv --format json --recommend
autoresearch cost --per-iteration-usd 0.25 --format json
```

`watch --websocket` streams snapshot and row update payloads for WebSocket watch streams. `checkpoint` runs evals only when the active run reaches its configured or adaptive checkpoint interval.
`evals --recommend` adds explicit go/no-go and next-step guidance to text, markdown, or JSON output.
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
