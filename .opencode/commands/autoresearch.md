---
name: autoresearch
description: "Autonomous iteration loop: modify, verify, keep/discard against any metric"
argument-hint: "[Goal: <text>] [Scope: <glob>] [Metric: <text>] [Verify: <cmd>] [Guard: <cmd>] [Iterations: N] [--evals]"
---

EXECUTE IMMEDIATELY — do not deliberate before reading this protocol.

## Parse Arguments

Extract from $ARGUMENTS:
- `Goal:` — what to improve
- `Scope:` or `--scope` — file globs
- `Metric:` — what to measure
- `Direction:` — higher_is_better (default) or lower_is_better
- `Verify:` — shell command that outputs a scalar number or final-line JSON metrics object
- `Verify format:` — `scalar` (default) or `metrics_json`
- `Primary metric key:` — required when `Verify format` is `metrics_json`
- `Guard:` — optional safety command (must exit 0)
- `Acceptance criteria:` — optional metric thresholds for stopping
- `Required keep criteria:` — optional metric thresholds that every keep must satisfy
- `Companion repo:` or `--companion-repo-scope PATH=SCOPE` — optional clean companion repos for multi-repo runs
- `Environment summary:` or `--environment-summary TEXT` — optional resource/tool profile written to results TSV metadata
- `Iterations:` or `--iterations` — integer N for bounded mode (default: 25). "unlimited" for unbounded.
- `--evals` — enable mid-loop checkpoints
- `--evals-interval N` — checkpoint frequency override
- `--chain <targets>` — comma-separated downstream commands

## Setup (if required context missing)

If Goal, Scope, Metric, or Verify missing → use question (single batched call):
  Q1 (Goal): "What do you want to improve?"
  Q2 (Scope): "Which files?" — suggest globs from project structure
  Q3 (Metric+Verify): "How to measure? Provide a shell command that outputs a number"
  Q4 (Guard): "Safety command that must always pass?" — test cmd, build cmd, skip
If ALL provided inline → skip setup, proceed directly.

## Precondition Checks

1. Verify git repo exists (`git rev-parse --git-dir`)
2. Run `autoresearch health` when prior artifacts exist, before resume, or before unattended/background execution
3. Check clean working tree (`git status --porcelain`) — warn if dirty
4. Check for stale lock files, detached HEAD
5. If Guard set → run Guard to establish guard baseline
6. Fail fast on any critical issue. Warn on non-critical.

## Verify Safety Screen

Before first dry-run, screen Verify command for: rm -rf, fork bombs, pipe-to-shell, embedded credentials, outbound writes. Block dangerous commands.

## Establish Baseline (Iteration 0)

1. Run `autoresearch init --verify "{verify command}" --direction {higher|lower}` with any verify format, primary metric key, acceptance criteria, required keep criteria, and optional environment summary flags.
2. Let the binary create `autoresearch-results/`, `results.tsv`, `state.json`, `context.json`, and `.codex-autoresearch/pointer.json`.
3. Use the returned baseline metric and state JSON for `/goal`.

## Set /goal

After baseline established, activate the completion condition:

```
/goal {metric description} {direction} from {baseline} as measured by `{verify command}` with each iteration making one atomic change, committing, verifying, and keeping or reverting. Stop after {iterations} turns of iteration or when goal is reached.
```

If unbounded: omit "Stop after N turns" clause.

## Iteration Loop (each turn while /goal active)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of results TSV (`autoresearch-results/results.tsv`)
- Read `autoresearch-results/context.json` when present
- Run `git log --oneline -10` — see what worked/failed
- If last iteration was "keep" → run `git diff HEAD~1` to see what improved metric
- Identify: what worked, what failed, what's untried
- Consult `autoresearch-results/lessons.md` for strategy insights

### Phase 2: Ideate
- Based on review, choose ONE specific, testable hypothesis
- Hypothesis must be: atomic (one logical change), different from all previous attempts, within scope
- Priority: exploit last success → try untested idea → simplify → attempt bolder change
- If 3+ consecutive discards → apply REFINE (adjust within strategy)
- If 5+ consecutive discards → apply PIVOT (fundamentally different approach)

### Phase 3: Modify
- Make ONE focused change to improve the metric
- Change must be within declared scope
- Must fit in one sentence description

### Phase 4: Trial Commit
- Stage only scoped files: `git add -- <files>`
- Commit with prefix: `git commit -m "experiment: {description}"`
- Record commit SHA
- NEVER stage `autoresearch-results/` or `.codex-autoresearch/` artifacts

### Phase 5: Verify
- Run `autoresearch verify --format metrics_json --key {primary_metric_key} --command "{verify command}"` for structured metrics, or `autoresearch verify --command "{verify command}"` for scalar metrics
- Calculate delta from previous retained metric
- If verify output is unparseable: rerun once. If still unparseable → treat as crash.

### Phase 6: Guard (if configured)
- Run Guard command only after metric improvement detected
- Guard must exit 0 to pass
- If guard fails → revert regardless of metric improvement

### Phase 7: Decide
- Run `autoresearch decide --decision auto --metric {metric} --metrics-json '{metrics_json}' --commit {sha} --description "{description}"`
- **keep** — metric improved in correct direction, guard passed, and required keep criteria passed → commit stays
- **discard** — metric flat/regressed, guard failed, or criteria failed → binary reverts the experiment commit
- **crash** — verify/guard command errored → binary reverts the experiment commit
- **no-op** — no change made this iteration

Simplicity override: gain < 1% AND adds significant complexity → discard.
Metric unchanged AND code simpler → keep.

### Phase 8: Log
Let `autoresearch decide` append the results TSV row and update state/escalation JSON.

### Phase 9: Escalation Check
- 3 consecutive discards → REFINE: adjust parameters, consult lessons
- 5 consecutive discards → PIVOT: abandon strategy, re-read everything, try fundamentally different approach
- 2 PIVOTs without keep → web search for external solutions
- 3 PIVOTs without keep → soft blocker: clear /goal, report that human input needed
- 1 keep resets ALL escalation counters

### Eval Checkpoint (if --evals)
Interval = floor(iterations / 3), min 1. Fixed 10 if unbounded. Override: --evals-interval N.
Every {interval} iterations:
```
--- Eval Checkpoint (iterations {X}-{Y}) ---
Metric: {start} → {end} ({delta}) | Kept: {n}/{total} | Trend: {up/flat/down}
{one-line recommendation}
---
```
If plateau 3+ checkpoints → recommend clearing /goal early.

## Summary (after /goal clears or turns exhausted)

Print: total iterations, kept/discarded/crash counts, starting metric → final metric, improvement %, top 3 most effective changes.

## Chain Handoff

After completion, write handoff.json to output directory:
```json
{
  "version": "2.1.0",
  "protocol_version": "2.1.0",
  "binary_version": "<binary semver>",
  "source": "loop",
  "source_command": "loop",
  "timestamp": "<ISO>",
  "status": "COMPLETE|GOAL_MET|BOUNDED|BLOCKED|ERROR",
  "results_tsv": "autoresearch-results/results.tsv",
  "workspace_root": "<absolute workspace path>",
  "artifact_root": "<absolute autoresearch-results path>",
  "primary_repo": "<absolute primary repo path>",
  "repo_targets": [{"path": "<absolute repo path>", "scope": "src/**", "role": "primary"}],
  "results_path": "<absolute results.tsv path>",
  "handoff_path": "<absolute handoff.json path>",
  "goal": "...",
  "scope": [...],
  "hypothesis_queue": [],
  "summary": {},
  "findings": [],
  "config": {"goal": "...", "scope": [...], "metric": "...", "direction": "...", "verify": "..."},
  "chain": ["debug"],
  "next_target": "debug",
  "chain_continue": true,
  "propagate_evals": true,
  "evals_interval": 5
}
```
Invoke next target in --chain order. Propagate --evals flag.
