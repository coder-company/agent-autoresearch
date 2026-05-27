---
name: autoresearch:probe
description: "8 personas interrogate requirements until constraints saturate"
argument-hint: "[Topic: <text>] [Scope: <glob>] [--depth shallow|standard|deep] [Iterations: N]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Topic:` — feature/requirement/design to probe
- `Scope:` or `--scope` — file globs for context
- `--depth` — shallow (5), standard (15), deep (30)
- `--personas N` — active count (3-8, default 6)
- `--mode` — interactive (user answers) or autonomous (infer from code)
- `Iterations:` — default 15

## Setup (if Topic missing)
AskUserQuestion:
  Q1: "What to probe?" — feature, requirement, design
  Q2: "Which files for context?"
  Q3: "Mode?" — interactive (you answer) or autonomous (infer from code)

## Protocol (bounded, no /goal)
Each turn:
1. Rotate active persona (Security Architect, UX Designer, Performance Engineer, DBA, DevOps, QA, Product Manager, Accessibility Expert)
2. Persona asks pointed questions from their domain
3. In interactive mode: present question to user
4. In autonomous mode: answer from codebase evidence
5. Extract constraints/requirements from answers
6. Track saturation: net-new constraints per round
7. Stop when <2 new constraints per round for 3 consecutive rounds
8. Output: complete constraint list + coverage matrix
