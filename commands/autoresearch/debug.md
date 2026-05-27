---
name: autoresearch:debug
description: "Hunt bugs: hypothesize, test, falsify, repeat"
argument-hint: "[Scope: <glob>] [Symptom: <text>] [Iterations: N] [--fix]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Scope:` or `--scope` — file globs to investigate
- `Symptom:` or `--symptom` — error message or behavior
- `Iterations:` or `--iterations` — default 15
- `--fix` — after finding root cause, auto-fix

## Setup (if Scope/Symptom missing)
Auto-scan: run tests, lint, typecheck to detect failures.
AskUserQuestion (single batch):
  Q1: "What's the problem?" — specific error, failing tests, behavior
  Q2: "Which files?" — suggest globs from error locations
  Q3: "How deep?" — quick (5), standard (15), deep (30+)

## Protocol
1. Establish baseline: run tests/lint/typecheck, count failures
2. Set `/goal "zero failures in <scope> as measured by <verify>, or stop after <N> turns"`
3. Each turn:
   - Form hypothesis from error output + code reading
   - Add diagnostic (log, assert, minimal repro)
   - Run verify → confirm or falsify hypothesis
   - If confirmed: fix the root cause (one fix)
   - If falsified: revert diagnostic, form new hypothesis
   - Log finding to results TSV
4. On --fix: chain to /autoresearch:fix with findings
