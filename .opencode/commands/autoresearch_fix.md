---
name: autoresearch_fix
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
2. Ask user:
   Q1 (Fix What): "Found [N] failures. Fix what?" — everything, only tests, only types, only lint
   Q2 (Guard): "Safety command?" — npm test, tsc, npm run build, skip
   Q3 (Scope): "Which files can I modify?"
   Q4 (Launch): "Ready?" — fix until zero, fix with limit, cancel

## Establish Baseline (Iteration 0)

1. Run Target → count errors (metric = error count, direction = lower_is_better)
2. Create output directory, write TSV header + baseline

## Iteration Loop (until zero errors or max_iterations)

### Phase 1: Review
- Run Target to get current error list
- If error count == 0 → exit loop (SUCCESS)

### Phase 2: Prioritize
Order: crash/fatal → test failures → type errors → lint → warnings.

### Phase 3: Fix ONE Thing
- Pick highest-priority error, make ONE focused fix

### Phase 4: Commit
`git commit -m "experiment: fix {error_type} — {description}"`

### Phase 5: Verify
Run Target → count errors → compute delta.

### Phase 6: Guard
If Guard set → run Guard. If fails → revert.

### Phase 7: Decide
- **keep** — error count decreased AND guard passes
- **discard** — error count same/increased → revert

### Phase 8: Log
Append row to TSV.

## Summary

Print: total errors fixed, remaining errors, fix success rate.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
