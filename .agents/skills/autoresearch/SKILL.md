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
- Never stage `autoresearch-results/` or `.codex-autoresearch/` artifacts in experiment commits.

## Binary Operations

The `autoresearch` binary handles mechanical operations:
- `autoresearch init` — initialize results directory, baseline, config, and canonical context
- `autoresearch health` — preflight git/artifact/disk/verify/context state
- `autoresearch verify` — run verify command, parse metric or metrics JSON
- `autoresearch decide` — evaluate keep/discard logic, criteria gates, rollback, and escalation
- `autoresearch parallel closeout` — select a parallel worker winner, log audit rows, and update retained state once
- `autoresearch runtime run` — execute the supervised background loop; `start/status/supervise/stop` remain available for manual control
- `autoresearch status|resume|progress|lessons|evals` — inspect/resume/analyze runs
- `autoresearch hook <name>` — execute lifecycle hooks

## Core Protocol (Each Turn)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of `autoresearch-results/results.tsv`
- Read `autoresearch-results/context.json` when present
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
Run `autoresearch verify --format metrics_json --key <metric>` for structured output, or `autoresearch verify --command "<cmd>"` for scalar output.

### Phase 6: Guard (if configured)
Run only after metric improvement. Must exit 0.

### Phase 7: Decide
- Prefer `autoresearch decide --decision auto --metric <value> --metrics-json '<json>' --commit <sha>`.
- For parallel worker batches, use `autoresearch parallel closeout --batch-file <workers.json>` instead of hand-editing worker rows.
- **keep** — improved + guard passed + required keep criteria passed → commit stays
- **discard** — flat/regressed OR guard/criteria failed → binary reverts the experiment commit
- **crash** — command errored → binary reverts the experiment commit

### Phase 8: Log
Use the binary decision/log command to append `autoresearch-results/results.tsv` and update state.

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
7. **Never stage artifacts** — `autoresearch-results/` and `.codex-autoresearch/` stay uncommitted.
8. **When stuck, escalate** — REFINE → PIVOT → Web Search → Stop.

## References

Load only what the current mode requires:
- `references/security-checklist.md` — STRIDE + OWASP tables
- `references/predict-personas.md` — Expert persona definitions
- `references/reason-judge-protocol.md` — Adversarial debate judge protocol
