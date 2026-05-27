---
name: autoresearch
description: "Autonomous goal-directed iteration: modify, verify, keep/discard against any metric."
version: 0.1.0
---

# Autoresearch — Autonomous Goal-Directed Iteration (Codex)

Invoke via `$autoresearch` mention syntax. Modes are passed as keywords:
- `$autoresearch loop` — core metric iteration
- `$autoresearch plan` — goal → config wizard
- `$autoresearch debug` — bug hunting
- `$autoresearch fix` — error crushing
- `$autoresearch security` — STRIDE + OWASP audit
- `$autoresearch ship` — 8-phase ship workflow
- `$autoresearch scenario` — edge case generation
- `$autoresearch predict` — multi-persona debate
- `$autoresearch learn` — doc generation
- `$autoresearch reason` — adversarial refinement
- `$autoresearch probe` — requirement interrogation
- `$autoresearch improve` — ICP-driven product improvement
- `$autoresearch evals` — results analysis

## Safety Invariants
- Never push, publish, or deploy without explicit user approval.
- Bounded by default (25 iterations). Override with `Iterations: unlimited`.
- All results logged to `autoresearch-results/` directory.
- Never stage `autoresearch-results/` artifacts in experiment commits.

## Binary Operations

The `autoresearch` binary handles mechanical operations:
- `autoresearch verify` — run verify command, parse metric
- `autoresearch decide` — evaluate keep/discard logic
- `autoresearch hook <name>` — execute lifecycle hooks
- `autoresearch init` — initialize results directory and baseline

## Core Protocol (Each Turn)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of `autoresearch-results/results.tsv`
- Run `git log --oneline -10`
- Consult `autoresearch-results/lessons.md` for strategy insights

### Phase 2: Ideate
ONE specific, testable, atomic hypothesis. Different from all previous.

### Phase 3: Modify
ONE focused change within scope. Must fit in one sentence.

### Phase 4: Trial Commit
```bash
git add -- <scoped-files-only>
git commit -m "experiment: <what changed and why>"
```

### Phase 5: Verify
Run verify command. Final non-empty line = metric value.

### Phase 6: Guard (if configured)
Run only after metric improvement. Must exit 0.

### Phase 7: Decide
- **keep** — improved + guard passed → commit stays
- **discard** — flat/regressed OR guard failed → `git revert HEAD --no-edit`
- **crash** — command errored → `git revert HEAD --no-edit`

### Phase 8: Log
Append to `autoresearch-results/results.tsv`.

### Phase 9: Escalation
- 3 consecutive discards → REFINE
- 5 consecutive discards → PIVOT
- 2 PIVOTs without keep → Web search
- 3 PIVOTs without keep → Soft blocker

## Critical Rules

1. **One change per turn** — atomic experiments create causality.
2. **Read before write** — git log + results TSV before modifying.
3. **Mechanical verification only** — run the command, parse the number.
4. **Automatic rollback** — `git revert HEAD --no-edit` on failure.
5. **Simplicity wins** — equal metric + less code = KEEP.
6. **Git is memory** — experiments committed, failures reverted, TSV logs all.
7. **Never stage artifacts** — `autoresearch-results/` stays uncommitted.
8. **When stuck, escalate** — REFINE → PIVOT → Web Search → Stop.

## References

Load only what the current mode requires:
- `references/security-checklist.md` — STRIDE + OWASP tables
- `references/predict-personas.md` — Expert persona definitions
- `references/reason-judge-protocol.md` — Adversarial debate judge protocol
