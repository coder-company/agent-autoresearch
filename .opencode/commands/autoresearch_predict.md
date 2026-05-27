---
name: autoresearch_predict
description: "5 expert personas debate proposed changes before implementation"
argument-hint: "[Scope: <glob>] [Goal: <text>] [--depth shallow|standard|deep] [--adversarial] [--chain <targets>]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Scope:` or `--scope` — file globs to analyze
- `Goal:` or `--goal` — focus area
- `Depth:` or `--depth` — shallow (3 personas, 1 round), standard (5, 2), deep (8, 3)
- `--personas N` — override persona count (3-8)
- `--rounds N` — override debate rounds (1-3)
- `--adversarial` — hostile reviewer personas
- `--budget N` — max findings (default 40)
- `--fail-on <severity>` — CI gate
- `--chain`

## Setup (if Scope or Goal missing)

Ask user:
  Q1 (Scope): "Which files to analyze?"
  Q2 (Goal): "What should personas focus on?"
  Q3 (Depth): "How deep?" — shallow, standard, deep
  Q4 (Chain): "After analysis, chain to?" — debug, security, fix, no chain

## Phase 1: Reconnaissance

Scan in-scope files. Build: file inventory, dependency graph, API surface, data flow, test coverage map.

## Phase 2: Persona Generation

Load `references/predict-personas.md`.

**Default (5):** Architect, Security Analyst, Performance Engineer, Reliability Engineer, Devil's Advocate.
**Adversarial (--adversarial):** Breaker, Cheater, Scaler, Newbie, Malicious Insider.

## Phase 3: Independent Analysis

Each persona analyzes independently. Findings: title, severity, confidence, file:line, recommendation.

## Phase 4: Debate (per round)

Present findings cross-persona. Challenge, raise new issues, adjust confidence.

## Phase 5: Consensus

Deduplicate, resolve conflicts, anti-herd check, rank by severity × confidence × agreement.

## Phase 6: Report

Write `summary.md` (top findings), `debate.md` (full transcript).

## Phase 7: CI Gate

If `--fail-on` set: exit non-zero if findings exceed threshold.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
