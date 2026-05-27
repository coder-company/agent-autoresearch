---
name: autoresearch_improve
description: "Research-driven product improvement: ICP challenges → tiered features → PRDs"
argument-hint: "[Goal: <text>] [ICP: <persona>] [Scope: <glob>] [Iterations: N] [--depth shallow|standard|deep] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Goal:` — product or feature area to improve
- `ICP:` or `--icp` — ideal customer profile description
- `Scope:` or `--scope` — file globs for codebase context
- `Depth:` or `--depth` — shallow (3 categories, 10 iterations), standard (5, 20), deep (5, 40+)
- `--seeds N` — seed ideas per category (default 5)
- `--discover` — enable discovery research (default ON)
- `--no-discover` — disable discovery research
- `Iterations:` or `--iterations` — default 20. "unlimited" for unbounded.
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if Goal or ICP missing)

Ask user:
  Q1 (Goal): "What product area to improve?"
  Q2 (ICP): "Who is the ideal customer?"
  Q3 (Scope): "Which files for context?"
  Q4 (Depth): "How deep?" — shallow, standard, deep, unlimited
  Q5 (Discovery): "Include external research?" — yes, no

## Phase 1: Resolve Product Context

Read codebase, parse docs, build product map: features × touchpoints × state.

## Phase 2: Research Categories

| # | Category | Focus |
|---|---|---|
| 1 | ICP Challenges | Pain points, unmet needs, workflow friction |
| 2 | Competitor Gaps | What competitors miss |
| 3 | Market Trends | Emerging patterns, shifting standards |
| 4 | UX Patterns | Interaction improvements, accessibility |
| 5 | Revenue/Growth | Monetization, expansion, viral loops |

## Iteration Loop

### Phase 1: Review
Check category coverage, identify underexplored areas.

### Phase 2: Research
Pick category, generate 3-5 improvement ideas.

### Phase 3: ICP Binary Gate
"Does this serve the ICP's core workflow?" YES → rank. NO → discard.

### Phase 4: Tiered Ranking
Must-have / Nice-to-have / Moonshot. Score: impact × confidence × alignment / effort.

### Phase 5: Saturation Detection
3 iterations with zero new ICP-validated ideas → category saturated. All saturated → stop.

### Phase 6: Log
Append to TSV.

## User Selection + PRD Generation

Present ranked improvements by tier. User selects for PRD generation. Write focused PRDs.

## Summary

Print: total ideas, ICP pass rate, tier distribution, categories covered.

## Chain Handoff

Write handoff.json. Terminal emitter — produces PRDs as final output.
