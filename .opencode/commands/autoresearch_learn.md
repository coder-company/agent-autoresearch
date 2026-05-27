---
name: autoresearch_learn
description: "Scout codebase and auto-generate docs with validation-fix loop"
argument-hint: "[Mode: <init|update|check|summarize>] [Scope: <glob>] [Iterations: N] [--depth <level>] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Mode:` or `--mode` — init (create from scratch), update (refresh), check (validate), summarize (brief overview)
- `Scope:` or `--scope` — file globs to document
- `Depth:` or `--depth` — overview, standard, comprehensive
- `--file <path>` — specific file to document
- `--scan` — force fresh codebase scout
- `--topics` — comma-separated focus topics
- `--no-fix` — validate only, don't auto-fix
- `Iterations:` or `--iterations` — default 10. "unlimited" for unbounded.
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if Mode or Scope missing)

Ask user:
  Q1 (Mode): "What to do?" — init, update, check, summarize
  Q2 (Scope): "Which files?"
  Q3 (Depth): "How detailed?" — overview, standard, comprehensive
  Q4 (Topics): "Focus on?" — architecture, API, database, testing, all

## Establish Baseline

1. Scout codebase: file tree, imports/exports, existing docs
2. Identify documentation gaps
3. Metric = files with valid documentation (higher is better)

## Summarize Mode (no loop)

If mode == summarize: one-shot scan → summary.md. Skip iteration loop.

## Iteration Loop (init/update/check modes)

### Phase 1: Scout
Scan for documentation gaps. If none remain → early stop.

### Phase 2: Generate/Update
Pick highest-priority gap. Write/update documentation for ONE file/module.

### Phase 3: Validate
Check docs against code: descriptions accurate? Examples valid? Links work?

### Phase 4: Fix (unless --no-fix)
If validation issues → fix. Commit: `docs: document {file/module}`

### Phase 5: Log
Append to TSV.

## Summary

Print: files documented, validation pass rate, issues found/fixed, remaining gaps.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
