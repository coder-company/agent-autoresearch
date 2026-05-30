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
- `Iterations:` or `--iterations` — integer N for bounded mode (default: 25). "unlimited" for unbounded.
- `--evals` — enable mid-loop checkpoints
- `--evals-interval N` — checkpoint frequency override
- `--chain <targets>` — comma-separated downstream commands

## Setup (if required context missing)

If Goal, Scope, Metric, or Verify missing → ask user:
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
6. Fail fast on any critical issue.

## Verify Safety Screen

Before first dry-run, screen Verify command for: rm -rf, fork bombs, pipe-to-shell, embedded credentials, outbound writes. Block dangerous commands.

## Establish Baseline (Iteration 0)

1. Run `autoresearch init --verify "{verify command}" --direction {higher|lower}` with any verify format, primary metric key, acceptance criteria, and required keep criteria flags.
2. Let the binary create `autoresearch-results/`, `results.tsv`, `state.json`, `context.json`, and `.codex-autoresearch/pointer.json`.
3. Use the returned baseline metric and state JSON.

## Iteration Loop (each turn)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of results TSV
- Read `autoresearch-results/context.json` when present
- Run `git log --oneline -10`
- Consult `autoresearch-results/lessons.md` for strategy insights

### Phase 2: Ideate
ONE specific, testable, atomic hypothesis. Different from all previous.

### Phase 3: Modify
ONE focused change within scope.

### Phase 4: Trial Commit
`git add -- <files>` then `git commit -m "experiment: {description}"`. Never stage `autoresearch-results/` or `.codex-autoresearch/`.

### Phase 5: Verify
Run `autoresearch verify --format metrics_json --key {primary_metric_key} --command "{verify command}"` for structured metrics, or `autoresearch verify --command "{verify command}"` for scalar metrics.

### Phase 6: Guard
If Guard set and metric improved → run Guard. Must exit 0.

### Phase 7: Decide
Run `autoresearch decide --decision auto --metric {metric} --metrics-json '{metrics_json}' --commit {sha} --description "{description}"`.
- **keep** — improved + guard passed + required keep criteria passed
- **discard** — flat/regressed, guard failed, or criteria failed → binary reverts the experiment commit
- **crash** — command errored → binary reverts the experiment commit

### Phase 8: Log
Let `autoresearch decide` append the results TSV row and update state/escalation JSON.

### Phase 9: Escalation
- 3 consecutive discards → REFINE
- 5 consecutive discards → PIVOT
- 2 PIVOTs without keep → web search
- 3 PIVOTs without keep → stop, report blocker

## Summary (after completion)

Print: total iterations, kept/discarded/crash counts, starting metric → final metric, improvement %, top 3 most effective changes.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order. Propagate --evals flag.
