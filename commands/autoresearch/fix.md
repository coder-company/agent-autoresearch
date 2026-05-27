---
name: autoresearch:fix
description: "Crush errors one-by-one until zero remain"
argument-hint: "[Target: <cmd>] [Scope: <glob>] [Guard: <cmd>] [Iterations: N]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Target:` or `--target` — command showing errors (npm test, tsc, etc.)
- `Scope:` or `--scope` — file globs to modify
- `Guard:` or `--guard` — safety command
- `Iterations:` or `--iterations` — default 20
- `--category` — filter: test, type, lint, build

## Setup (if Target/Scope missing)
Auto-detect: run test suite, type checker, linter, build.
AskUserQuestion (single batch):
  Q1: "Found [N] test failures, [M] type errors. Fix what?" — all, tests only, types only
  Q2: "Safety command?" — npm test, tsc, skip
  Q3: "Which files?" — suggest globs from error locations

## Protocol
1. Run target → count errors (baseline)
2. Set `/goal "zero errors from <target command>, or stop after <N> turns"`
3. Each turn:
   - Run target, parse FIRST error
   - Read the failing code
   - Fix ONE error (smallest correct change)
   - Trial commit: `git commit -m "experiment: fix <error description>"`
   - Run target again → count errors
   - If count decreased: keep
   - If count same or increased: `git revert HEAD --no-edit`
   - Log iteration
4. Stop when error count reaches 0
