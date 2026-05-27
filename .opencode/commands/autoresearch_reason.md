---
name: autoresearch_reason
description: "Adversarial debate with blind judges until convergence"
argument-hint: "[Task: <question>] [Domain: <type>] [--mode convergent|creative|debate] [--judges N] [Iterations: N] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Task:` — question, proposal, design, argument, or claim to refine
- `Domain:` or `--domain` — software, product, business, security, research, content
- `Mode:` or `--mode` — convergent (default), creative, debate
- `--judges N` — blind judge count (3 default, 5 thorough, 7 deep)
- `--convergence N` — stop when incumbent wins N consecutive rounds (default 3)
- `Iterations:` or `--iterations` — default 8. "unlimited" for unbounded.
- `--no-synthesis` — skip synthesis, pure debate only
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if Task or Domain missing)

Ask user:
  Q1 (Task): "What should be reasoned about?"
  Q2 (Domain): "What domain?" — software, product, business, security, research, content
  Q3 (Mode): "Refinement mode?" — convergent, creative, debate
  Q4 (Judges): "How many blind judges?" — 3, 5, 7

## Setup Phase

Load `references/reason-judge-protocol.md`. Create output directory.
TSV header: `round\ttimestamp\tcandidate_label\tjudge_verdict\tconvergence_count\tdescription`

## Round Loop

### Phase 1: Generate-A
Author-A generates candidate (cold-start, task + domain context only).

### Phase 2: Critic
Critic finds ≥3 specific weaknesses (adversarial, no compliments).

### Phase 3: Generate-B
Author-B produces candidate addressing critique while preserving A's strengths.

### Phase 4: Synthesize (unless --no-synthesis)
Synthesizer merges best of A and B.

### Phase 5: Blind Judge Panel
Judges evaluate randomized-label candidates independently. Majority vote wins.

### Phase 6: Convergence Check
Winner == incumbent → count++. Winner != incumbent → count=1, new incumbent.
Convergent mode: count >= N → CONVERGED, stop.

### Phase 7: Oscillation Guard
5+ incumbent changes in 8 rounds → recommend early stop.

### Phase 8: Log
Append to TSV.

## Output

Write `lineage.md` (full history), `summary.md` (final winner + insights).

## Summary

Print: total rounds, convergence status, final winner, judge agreement rate.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
