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

1. Run `autoresearch init --verify "{target command}" --direction lower`.
2. Let the binary create `autoresearch-results/`, `results.tsv`, `state.json`, `context.json`, and `.codex-autoresearch/pointer.json`.
3. Use the returned baseline error count as the metric.

## Iteration Loop (until zero errors or max_iterations)

### Phase 1: Review
- Read results TSV + `autoresearch-results/context.json` + git log
- Run Target to get current error list
- If error count == 0 → exit loop (SUCCESS)

### Phase 2: Prioritize
Order: crash/fatal → test failures → type errors → lint → warnings.

### Phase 3: Fix ONE Thing
- Pick highest-priority error, make ONE focused fix

### Phase 4: Commit
Stage only scoped files and commit: `git commit -m "experiment: fix {error_type} — {description}"`. Never stage `autoresearch-results/` or `.codex-autoresearch/`.

### Phase 5: Verify
Run `autoresearch verify --command "{target command}"` → count errors → compute delta.

### Phase 6: Guard
If Guard set → run `autoresearch guard --command "{guard command}"`.

### Phase 7: Decide
Run `autoresearch decide --decision auto --metric {error_count} --commit {sha} --description "{description}"`.
- **keep** — error count decreased AND guard passes
- **discard** — error count same/increased or guard failed → binary reverts the experiment commit

### Phase 8: Log
Let `autoresearch decide` append the results TSV row and update state/escalation JSON.

## Summary

Print: total errors fixed, remaining errors, fix success rate.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
