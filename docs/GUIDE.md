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
autoresearch verify --command "cat metric.txt" --repeat 3 --aggregate median
autoresearch plan --goal "reduce any types" --format json
autoresearch debug --symptom "API returns 500" --scope "src/**/*.rs"
autoresearch fix --target "npx tsc --noEmit" --scope "src/**/*.ts" --category type
autoresearch improve --goal "Improve onboarding activation" --icp "Developer tools teams"
autoresearch prd --title "Improve onboarding" --problem "New users stall before first run"
autoresearch security --scope "src/**/*.rs" --focus auth
autoresearch ship --target "Release v1.2.0" --type code-release --dry-run
autoresearch scenario --target "Checkout flow" --domain web --format test-scenarios --scope "src/checkout/**"
autoresearch predict --proposal "Add cache warming to search results" --scope "src/search/**"
autoresearch reason --question "Should we replace the storage layer" --mode debate --domain software
autoresearch probe --subject "Payment retry workflow" --scope "src/payments/**"
autoresearch learn --mode summarize --scope "src/**/*.rs"
autoresearch decide --decision auto --metric 4 --commit abc1234 --description "improved"
autoresearch status --summary
autoresearch progress
autoresearch cost --per-iteration-usd 0.25 --format json
autoresearch dashboard --once
autoresearch health --strict
autoresearch env --format json
autoresearch init --environment-summary auto --verify "cat metric.txt" --direction lower
autoresearch checkpoint --format json
autoresearch reanchor --format json
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
autoresearch evals --file autoresearch-results/results.tsv --format json
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
Use `autoresearch env --format json` to capture CPU, disk, container, toolchain, and recommended parallel-worker context before planning long or parallel runs; pass `--environment-summary auto` to `init` to persist that probe summary in `results.tsv`.
Use `autoresearch status --summary` for compact monitor-friendly counters.
Use `autoresearch progress` for the current metric, trend, counters, escalation state, and terminal metric history sparkline.
Use `autoresearch verify --repeat <n> --aggregate <median|mean|min|max|last>` for noisy scalar metrics; repeated verification returns the aggregate metric plus the raw samples.
Use `autoresearch cost --per-iteration-usd <usd>` or token/rate flags to estimate completed, remaining, and projected run spend.
Use `autoresearch dashboard --once` for a combined terminal view of status, trend, metric history, escalation, and recent rows; omit `--once` for live refresh.
Use `autoresearch checkpoint --format json` inside long loops to run evals only when the active iteration reaches the configured or adaptive checkpoint interval.
Use `autoresearch reanchor --format json` every 10 iterations or after context compaction to print the protocol fingerprint, reload references, and `[RE-ANCHOR]` logging tag.
Use `autoresearch watch --format <tsv|jsonl>` for human-readable tails or machine-readable JSON Lines.
Use `autoresearch watch --websocket --websocket-addr <host:port>` to serve snapshot and row update payloads to real-time dashboards. Add `--once` to print the initial WebSocket snapshot envelope without starting a server.
Use `autoresearch lessons --add <strategy> --context <note>` to append reusable lessons without editing `lessons.md` by hand.
Use `autoresearch search --from-state` with `--provider-command` or `AUTORESEARCH_SEARCH_CMD` to run cached, run-aware web searches. Add `--log` to append a `search` meta-iteration. When `decide` escalates to Web Search, it automatically runs the same cached helper with `AUTORESEARCH_SEARCH_CMD` and logs the result when timing/cooldown limits allow it.
Use `autoresearch parallel closeout --merge-strategy <cherry-pick|fast-forward|squash|rebase>` to select how the retained worker commit is merged.
Use `autoresearch parallel compare --a <hypothesis> --b <hypothesis>` to prepare a two-arm A/B batch that reuses `parallel run` and verified `parallel closeout`.
Use `autoresearch evals --file <path> --format json` after parallel closeout to include worker improvement counts and a sign-test summary for parallel batches.
Use `autoresearch completions <bash|zsh|fish|elvish|powershell>` to generate shell completions.
Use `autoresearch manpages --output-dir man/man1` to generate a local `autoresearch.1` manual page.
Use `autoresearch config template --output .autoresearch.toml` to write a starter project defaults file.
Use `autoresearch config validate` to parse defaults, validate options, and screen configured commands without running them.
Use `autoresearch plan --goal <goal> --format json` to get a launch-ready suggested scope, metric, direction, verify, guard, and iteration count from detected repo tooling.
Use `autoresearch plan --goal <goal> --chain <targets>` to write the derived config into a downstream handoff.
Native artifact generators default to ignored `autoresearch-results/<mode>/` paths; pass `--output` or `--output-dir` only when you intentionally want a different artifact location.
Use `autoresearch debug --symptom <failure> --scope <glob>` to write a hypothesis-driven investigation bundle with summary, findings, eliminated hypotheses, TSV, and handoff JSON.
Add `--fix` or `--chain <targets>` to `autoresearch debug` to record downstream chain metadata in the debug handoff.
Use `autoresearch debug --depth deep --iterations 12 --severity high` to override the investigation budget and record severity filter metadata.
Use `autoresearch fix --target <verify-command> --scope <glob> --iterations 7` to write a repair-plan bundle under `autoresearch-results/fix` with priority order, results TSV, iteration budget, and handoff JSON.
Use `autoresearch fix --from-debug` to import the latest debug handoff scope, symptom, and finding count into the repair plan.
Use `autoresearch fix --chain <targets> --evals` to record downstream handoff and checkpoint propagation metadata.
Use `autoresearch improve --goal <product-area> --icp <persona>` to write an improve-mode artifact bundle: research findings, ranked plan, summary, TSV, and handoff JSON.
Use `autoresearch improve --goal <product-area> --icp <persona> --depth deep --iterations 24 --evals` to override the research budget and record active category count plus checkpoint metadata.
Use `autoresearch improve --goal <product-area> --seeds 5 --no-discover --chain learn` to record seed volume, discovery posture, and downstream handoff metadata.
Use `autoresearch prd --title <title> --problem <problem>` to write a focused improve-mode PRD with DECISION NEEDED markers, acceptance criteria, risks, success metrics, and an autoresearch config block.
Use `autoresearch security --scope <glob> --focus <area>` to write a STRIDE + OWASP audit bundle with overview, threat model, attack surface, coverage, findings, recommendations, TSV, and handoff JSON.
Add `--fail-on <severity>` and `--fix` to `autoresearch security` to record CI gate and repair-chain metadata for confirmed findings.
Use `autoresearch security --scope <glob> --depth deep --iterations 18 --diff --chain fix --evals` to override the audit budget and record delta mode, downstream handoff, and checkpoint metadata.
Use `autoresearch ship --target <thing> --type <kind> --dry-run` to write an 8-phase ship checklist, summary, ship log, and handoff JSON without external side effects.
Use `autoresearch ship --target <thing> --auto --force --rollback --monitor 15 --chain learn` to record approval, rollback, monitoring, and downstream handoff metadata.
Use `autoresearch scenario --target <feature> --domain <general|web|mobile|api|cli|data-pipeline|infrastructure> --format <test-scenarios|threat-scenarios|use-cases|user-stories>` to write a 12-dimension scenario matrix for tests, threat modeling, or debug follow-up.
Use `autoresearch scenario --target <feature> --domain web --depth deep --iterations 16 --evals --debug` to override the exploration budget and record domain, checkpoint metadata, and downstream debug handoff.
Use `autoresearch predict --proposal <change>` to write a five-persona review covering architecture, security, performance, UX, and adversarial risks.
Use `autoresearch predict --proposal <change> --depth deep --adversarial --fail-on high` to record review profile and CI gate metadata.
Use `autoresearch predict --proposal <change> --debug` to record the review as handoff context for downstream investigation.
Use `autoresearch reason --question <decision>` to write an adversarial debate artifact with candidate solutions, blind judge rubric, and convergence criteria.
Use `autoresearch reason --question <decision> --chain predict,fix` to pass the selected debate context into downstream review or repair.
Use `autoresearch reason --question <decision> --iterations 11 --judges 7 --convergence 4 --temperature 0.2` to record debate budget, judge panel, convergence, synthesis, and generation hints.
Use `autoresearch probe --subject <requirement>` to write eight persona-driven questions, constraint slots, and a saturation rule before implementation.
Use `autoresearch probe --subject <requirement> --mode autonomous --depth deep --iterations 9 --adversarial` to override the interrogation round budget and record saturation metadata.
Use `autoresearch probe --subject <requirement> --chain plan` to pass discovered constraints into planning through handoff metadata.
Use `autoresearch learn --mode <init|update|check|summarize> --scope <glob>` to write documentation summary, validation, TSV, and handoff artifacts.
Use `autoresearch learn --mode check --file <path> --depth overview --iterations 14 --topics architecture,api --no-fix --evals` to record learn profile, specific-file scope, validation behavior, chain, and checkpoint metadata.
Use `autoresearch api --format json` to inspect the stable command/flag manifest and semver policy used by wrappers and agents.
Use `autoresearch mcp serve` as a stdio MCP server exposing read-only `autoresearch_status` and `autoresearch_watch_snapshot` tools.
Use `autoresearch mcp call --server-command <cmd> --tool <name> --arguments '{}'` to call a tool on an external stdio MCP server from an iteration script.
Use `autoresearch scope expand --format json` to resolve active primary and companion repo scopes, with package roots inferred from `Cargo.toml`, `package.json`, `pyproject.toml`, and `go.mod`.
Use `autoresearch workspace exec --command <cmd> --rollback-on-failure` to run one screened command across primary and companion repo targets, restoring attempted repos if any target fails.
Use `autoresearch guard-presets --format json` to suggest per-repo guard commands for primary and companion repositories.
Use `autoresearch lessons --workspace-context --last 5` from any managed repo to show the shared workspace lessons path and repo targets.
Use `autoresearch plugin list` and `autoresearch plugin validate --path <file>` to load local TOML mode plugin manifests with command safety screening.
Use `autoresearch plugin marketplace` to validate `.autoresearch/plugins/marketplace.toml` and every referenced community mode manifest before installing or sharing it.
Use `./install.sh --yes --vscode` to install the lightweight VS Code package from `integrations/vscode`; it opens `status --summary`, `dashboard --once`, and `watch --format jsonl` from editor commands.
Codex packages keep `.agents/skills/autoresearch/SKILL.md` as a thin router and load `references/binary-operations.md` only when native command details are needed.
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
