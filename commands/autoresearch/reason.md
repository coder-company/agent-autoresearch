---
name: autoresearch:reason
description: "Adversarial debate with blind judges until convergence"
argument-hint: "[Task: <question>] [Domain: <type>] [--judges N] [Iterations: N]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Task:` — question/proposal/claim to refine
- `Domain:` — software, product, business, security, research
- `--judges N` — judge count (3 default, 5 thorough, 7 deep)
- `--convergence N` — consecutive wins to stop (default: 3)
- `--mode` — convergent (default), creative, debate
- `Iterations:` — default 8

## Setup (if Task missing)
AskUserQuestion:
  Q1: "What should be reasoned about?"
  Q2: "Domain?" — software, product, business, security
  Q3: "Mode?" — convergent, creative, debate

## Protocol
Set `/goal "convergence: incumbent answer wins <N> consecutive rounds, or stop after <iterations> turns"`
Each turn:
1. Generate challenger answer (different approach/framing)
2. Present incumbent vs challenger to blind judge panel
3. Judges score independently (no knowledge of which is incumbent)
4. Winner becomes new incumbent
5. Track consecutive wins
6. Stop on convergence or iteration cap
7. Output: final answer + reasoning chain + judge scores
