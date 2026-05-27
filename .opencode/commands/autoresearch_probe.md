---
name: autoresearch_probe
description: "8 personas interrogate requirements until constraints saturate"
argument-hint: "[Topic: <text>] [Scope: <glob>] [--depth shallow|standard|deep] [--personas N] [--mode interactive|autonomous] [Iterations: N] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Topic:` — feature, requirement, or design to probe
- `Scope:` or `--scope` — file globs for codebase grounding
- `Depth:` or `--depth` — shallow (5 rounds), standard (15), deep (30)
- `--personas N` — active persona count (3-8, default 6)
- `--saturation-threshold N` — net-new constraints/round below which counts toward saturation (default 2)
- `--mode` — interactive (default) or autonomous (self-answers from codebase)
- `--adversarial` — rotate hostile personas to front
- `Iterations:` or `--iterations` — default 15. "unlimited" for unbounded.
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if Topic missing)

Ask user:
  Q1 (Topic): "What to probe?"
  Q2 (Scope): "Which files for context?"
  Q3 (Depth): "How deep?" — shallow, standard, deep, unlimited
  Q4 (Mode): "How to answer?" — interactive (you answer), autonomous (agent infers)

## 8 Personas

| # | Persona | Focus |
|---|---|---|
| 1 | Domain Expert | Business rules, domain constraints |
| 2 | End User | Usability, expectations, error recovery |
| 3 | Skeptic | Assumptions that might be wrong |
| 4 | Edge-Case Hunter | Boundary conditions, rare scenarios |
| 5 | Ops Engineer | Deployment, monitoring, scaling |
| 6 | Security Reviewer | Attack vectors, data protection |
| 7 | Contradiction Finder | Conflicts between requirements |
| 8 | Scope Guardian | Feature creep, unnecessary complexity |

## Round Loop

### Phase 1: Persona Activation
Select 2-3 personas. Each generates 3-5 probing questions.

### Phase 2: Codebase Grounding
Check questions against code. Annotate with file:line, existing behavior, gaps.

### Phase 3: Answer Capture
Interactive: present questions to user. Autonomous: infer from codebase.

### Phase 4: Constraint Extraction
Parse answers into atomic constraints. Deduplicate against registry.

### Phase 5: Cross-Check
Check for conflicts. Flag contradictions.

### Phase 6: Saturation Check
Net-new < threshold for 3 rounds → SATURATED, exit.

### Phase 7: Log
Append round data to output.

## Output

Write `constraints.md`, `conflicts.md`, ready-to-run autoresearch config.

## Summary

Print: total rounds, constraints found, saturation status, unresolved conflicts.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
