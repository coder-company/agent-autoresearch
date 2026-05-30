---
name: autoresearch:fix
description: "Crush errors one-by-one until zero remain: tests, types, lint, build"
argument-hint: "[Target: <cmd>] [Scope: <glob>] [Guard: <cmd>] [Iterations: N] [--evals] [--from-debug]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Target:` or `--target` — command that shows errors (e.g., `npm test`, `tsc --noEmit`)
- `Scope:` or `--scope` — file globs to modify
- `Guard:` or `--guard` — safety command (must always pass)
- `Iterations:` or `--iterations` — default 20. "unlimited" for unbounded.
- `--from-debug` — read handoff.json from previous debug run
- `--category` — filter: test, type, lint, build
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if required context missing)

If Target and Scope both missing:
1. Auto-detect failures: run test suite, type checker, linter, build
2. Present results via AskUserQuestion (single batched call):
   Q1 (Fix What): "Found [N] test failures, [M] type errors, [K] lint errors. Fix what?" — everything, only tests, only types, only lint
   Q2 (Guard): "Safety command that must always pass?" — npm test, tsc, npm run build, skip
   Q3 (Scope): "Which files can I modify?" — suggested globs from error locations + all
   Q4 (Launch): "Ready?" — fix until zero, fix with limit, cancel
If all provided → skip setup.
If --from-debug → read handoff.json for scope and findings.

## Precondition Checks

Verify: git repo exists, clean working tree, no lock files, no detached HEAD. Fail fast on critical issues.

## Establish Baseline (Iteration 0)

1. Run `autoresearch init --verify "{target command}" --direction lower`.
2. Let the binary create `autoresearch-results/`, `results.tsv`, `state.json`, `context.json`, and `.codex-autoresearch/pointer.json`.
3. Use the returned baseline error count as the metric.

## Set /goal

After baseline established, activate the completion condition:

```
/goal error count reaches 0 as measured by running {target command} and counting failures, or stop after {iterations} turns
```

## Iteration Loop (until zero errors or max_iterations)

### Phase 1: Review
- Read results TSV + `autoresearch-results/context.json` + git log
- Run Target to get current error list
- If error count == 0 → exit loop (SUCCESS)

### Phase 2: Prioritize
Order: crash/fatal → test failures → type errors → lint → warnings.
Within category: easiest first (single-file fixes before cross-file).

### Phase 3: Fix ONE Thing
- Pick the highest-priority error
- Make ONE focused fix (atomic — addresses exactly one error)
- Record error type and which error was fixed

### Phase 4: Commit
- Stage only scoped files and commit: `experiment: fix {error_type} — {description}`. Never stage `autoresearch-results/` or `.codex-autoresearch/`.

### Phase 5: Verify
- Run `autoresearch verify --command "{target command}"` → count errors → compute delta
- Expected: error count decreased by 1 or more

### Phase 6: Guard
- If Guard set → run `autoresearch guard --command "{guard command}"`.

### Phase 7: Decide
- Run `autoresearch decide --decision auto --metric {error_count} --commit {sha} --description "{description}"`.
- **keep** — error count decreased AND guard passes
- **keep (reworked)** — fix needed adjustment, second attempt worked
- **discard** — error count same/increased or guard failed → binary reverts the experiment commit
- **crash** — target/guard command failed → binary reverts the experiment commit
- **hook-blocked** — git hook blocked the commit
- **metric-error** — target output not parseable → binary reverts the experiment commit

### Phase 8: Log
Let `autoresearch decide` append the results TSV row and update state/escalation JSON.

### Eval Checkpoint
If --evals: check if current_iteration % interval == 0 → run checkpoint analysis.

### Bounded Check
If bounded: current_iteration >= max_iterations → exit loop, print summary.

## Summary

Print: total errors fixed, remaining errors, error types distribution, fix success rate.

## Eval Checkpoint (--evals flag)

If --evals present:
- Compute interval: floor(max_iterations / 3), min 1. Fixed 10 if unbounded. Override: --evals-interval N.
- Every {interval} iterations, pause and analyze current results TSV.
- Print: `--- Eval Checkpoint (iterations {X}-{Y}) ---\nErrors: {start} → {end} ({delta}) | Kept: {n}/{total} | Trend: {up/flat/down}\n{one-line recommendation}\n---`
- If plateau 3+ checkpoints → recommend early stop.
- At loop end → full evals summary to evals-summary.md in output directory.

## Chain Handoff

After completion, write `autoresearch-results/handoff.json`: version "0.1.0", source "fix", timestamp, status (COMPLETE|USER_INTERRUPT|BOUNDED|ERROR), results_tsv path, findings = unfixed errors, config{target, scope, guard}.
Invoke next target in --chain order. Propagate --evals flag.
