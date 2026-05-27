---
name: autoresearch_debug
description: "Hunt bugs with scientific method: hypothesize, test, falsify, repeat"
argument-hint: "[Scope: <glob>] [Symptom: <text>] [Iterations: N] [--fix] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Scope:` or `--scope` — file globs to investigate
- `Symptom:` or `--symptom` — error message or behavior description
- `Iterations:` or `--iterations` — default 15. "unlimited" for unbounded.
- `--fix` — shorthand for `--chain fix`
- `--severity` — filter: critical, high, medium, low
- `--technique` — force specific technique
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if required context missing)

If Scope and Symptom both missing:
1. Auto-scan: run tests, lint, typecheck to detect existing failures
2. Ask user:
   Q1 (Issue): "What's the problem?"
   Q2 (Scope): "Which files?"
   Q3 (Depth): "How deep?" — quick (5), standard (15), deep (30+), unlimited
   Q4 (After): "When bugs found?" — report only, find and fix, ask each time

## Investigation Techniques

| Technique | When to Use |
|---|---|
| Binary search | Know when it worked, find when it broke |
| Differential | Compare working vs broken state |
| Minimal reproduction | Simplify to smallest failing case |
| Trace | Follow execution path through code |
| Pattern search | Grep for known anti-patterns |
| Working backwards | Start from error, trace to root cause |

## Establish Baseline

1. Auto-scan for failures if no symptom provided
2. Create output directory: `autoresearch-results/`
3. TSV header: `iteration\ttimestamp\thypothesis\tstatus\ttechnique\tevidence\tfile_line`
4. Metric = cumulative confirmed findings count

## Iteration Loop

### Phase 1: Review Context
- Read results TSV (past findings)
- Assess what's been tested, what vectors remain

### Phase 2: Hypothesize
- Form ONE specific, falsifiable hypothesis
- Format: "I hypothesize that {X} because {evidence}. Test by {Y}."

### Phase 3: Investigate
- Apply appropriate technique
- Read relevant code, run targeted tests, check logs
- Collect evidence (file:line references required)

### Phase 4: Classify
- **confirmed** — bug found with evidence
- **disproven** — hypothesis wrong
- **inconclusive** — needs different approach

### Phase 5: Log
Append to TSV. Check eval checkpoint if --evals.

## Summary

Print: total hypotheses tested, confirmed/disproven/inconclusive counts, all confirmed bugs with severity and file:line.

## Chain Handoff

Write handoff.json. If --fix → chain to fix automatically.
