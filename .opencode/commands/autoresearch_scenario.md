---
name: autoresearch_scenario
description: "Generate edge cases across 12 dimensions from a seed scenario"
argument-hint: "[Scenario: <text>] [Domain: <type>] [Scope: <glob>] [Iterations: N] [--depth <level>] [--focus <area>] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Scenario:` — seed scenario description
- `Domain:` or `--domain` — web, mobile, API, CLI, data pipeline, infrastructure
- `Scope:` or `--scope` — file globs for context
- `Focus:` or `--focus` — specific dimension to prioritize
- `--depth` — shallow (10), standard (20), deep (40+)
- `--format` — markdown (default), json, gherkin
- `Iterations:` or `--iterations` — default 20. "unlimited" for unbounded.
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if Scenario or Domain missing)

Ask user:
  Q1 (Scenario): "Describe the feature/flow to explore"
  Q2 (Domain): "What domain?" — web app, mobile, API, CLI, data pipeline, infrastructure
  Q3 (Scope): "Which files for context?"
  Q4 (Depth): "How deep?" — quick (10), standard (20), deep (40+), unlimited

## 12 Dimensions

| # | Dimension | Explores |
|---|---|---|
| 1 | Happy path | Normal successful flows |
| 2 | Validation | Input boundaries, types, formats |
| 3 | Permissions | Auth, roles, access control |
| 4 | Concurrency | Race conditions, deadlocks, ordering |
| 5 | State | Invalid transitions, corruption |
| 6 | Scale | High volume, large data, many users |
| 7 | Failure | Network errors, timeouts, partial failures |
| 8 | Security | Injection, abuse, bypass attempts |
| 9 | Integration | Third-party failures, API contract violations |
| 10 | Data | Null, empty, unicode, injection, overflow |
| 11 | UX | Confusion, misuse, accessibility |
| 12 | Recovery | Retry, rollback, idempotency |

## Iteration Loop

### Phase 1: Review
Check dimension coverage, identify underexplored dimensions.

### Phase 2: Generate
Pick next dimension, generate 3-5 specific scenarios.

### Phase 3: Classify
- **new** — genuinely novel edge case
- **extension** — builds on previous
- **duplicate** — skip

### Phase 4: Log
Append new/extension to TSV. Skip duplicates.

### Phase 5: Saturation Check
3 consecutive duplicates-only → dimension saturated. All saturated → early stop.

## Output

Write `scenarios.md` (by dimension), `edge-cases.md` (severity-ranked).

## Summary

Print: total scenarios, dimension coverage (X/12), severity distribution.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
