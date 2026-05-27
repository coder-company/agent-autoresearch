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
- `Verify:` — shell command that outputs a number on its final non-empty line
- `Guard:` — optional safety command (must exit 0)
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
2. Check clean working tree (`git status --porcelain`) — warn if dirty
3. Check for stale lock files, detached HEAD
4. If Guard set → run Guard to establish guard baseline
5. Fail fast on any critical issue.

## Verify Safety Screen

Before first dry-run, screen Verify command for: rm -rf, fork bombs, pipe-to-shell, embedded credentials, outbound writes. Block dangerous commands.

## Establish Baseline (Iteration 0)

1. Run Verify command → extract numeric metric from final non-empty line
2. Record as iteration 0 in TSV
3. Create output directory: `autoresearch-results/`
4. Write TSV header + baseline row
5. Write state.json with baseline snapshot

## Iteration Loop (each turn)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of results TSV
- Run `git log --oneline -10`
- Consult `autoresearch-results/lessons.md` for strategy insights

### Phase 2: Ideate
ONE specific, testable, atomic hypothesis. Different from all previous.

### Phase 3: Modify
ONE focused change within scope.

### Phase 4: Trial Commit
`git add -- <files>` then `git commit -m "experiment: {description}"`

### Phase 5: Verify
Run Verify → extract metric → compute delta.

### Phase 6: Guard
If Guard set and metric improved → run Guard. Must exit 0.

### Phase 7: Decide
- **keep** — improved + guard passed
- **discard** — flat/regressed or guard failed → `git revert HEAD --no-edit`
- **crash** — command errored → revert

### Phase 8: Log
Append to results TSV. Update state.json.

### Phase 9: Escalation
- 3 consecutive discards → REFINE
- 5 consecutive discards → PIVOT
- 2 PIVOTs without keep → web search
- 3 PIVOTs without keep → stop, report blocker

## Summary (after completion)

Print: total iterations, kept/discarded/crash counts, starting metric → final metric, improvement %, top 3 most effective changes.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order. Propagate --evals flag.
