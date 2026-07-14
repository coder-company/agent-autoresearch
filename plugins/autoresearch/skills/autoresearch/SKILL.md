---
name: autoresearch
description: "Autonomous goal-directed iteration: modify, verify, keep/discard against any metric."
version: 0.1.2
---

# Autoresearch (Codex)

Use `$autoresearch [mode]` when the user wants an autonomous improve-verify loop, launch planning, or one of the specialized autoresearch workflows. Keep this entrypoint as the thin router; load detailed references only when that behavior is active.

## Modes

| Mode | Purpose |
| --- | --- |
| `loop` | Core metric iteration |
| `plan` | Goal-to-config wizard |
| `debug` | Bug hunting |
| `fix` | Error reduction |
| `security` | STRIDE + OWASP audit |
| `ship` | Release readiness |
| `scenario` | Edge-case exploration |
| `predict` | Multi-persona debate |
| `learn` | Documentation generation |
| `reason` | Adversarial refinement |
| `probe` | Requirement interrogation |
| `improve` | ICP-driven product improvement |
| `evals` | Results analysis |
| `exec` | Non-interactive CI/CD loop |

If no mode is supplied, infer one from the request; default to `loop` for metric improvement.

## Load Rules

Always load `references/core-principles.md`, `references/modes.md`, and `references/structured-output-spec.md`.

For fresh or resumable launches, load `references/session-resume.md`, `references/interaction-wizard.md`, and `references/environment-awareness.md`.

For active execution, load `references/runtime-hard-invariants.md`, `references/runtime-protocol.md`, and `references/results-logging.md`.

For CI/non-interactive runs, load `references/exec-workflow.md`.

Load only when needed:

- `references/autonomous-loop-protocol.md`, `references/loop-workflow.md`
- `references/plan-workflow.md`, `references/debug-workflow.md`, `references/fix-workflow.md`
- `references/security-workflow.md`, `references/ship-workflow.md`
- `references/pivot-protocol.md`, `references/escalation.md`
- `references/parallel-experiments-protocol.md`, `references/hypothesis-perspectives.md`
- `references/web-search-protocol.md`, `references/lessons-protocol.md`
- `references/health-check-protocol.md`, `references/binary-operations.md`
- `references/security-checklist.md`, `references/predict-personas.md`, `references/reason-judge-protocol.md`

## Native Binary

Use `references/binary-operations.md` for the CLI catalog. It covers `autoresearch health --strict`, `autoresearch runtime run`, `autoresearch parallel prepare/run/cleanup`, `timeout-seconds`, `merge-strategy` including `rebase`, `autoresearch dashboard --once`, WebSocket watch streams, `autoresearch lessons --add`, `autoresearch search --from-state --log`, `autoresearch mcp serve`, `mcp call --server-command`, `autoresearch workspace exec`, and multi-repo `--companion-repo-scope PATH=SCOPE`.

## Hard Invariants

1. Never push, publish, deploy, or run external ship actions without explicit user approval.
2. Keep runs bounded by default; use `Iterations: unlimited` only when the user asks.
3. Read before writing, then make one focused change per iteration.
4. Use mechanical verification only.
5. Create scoped trial commits before verification after launch approval.
6. Never stage `autoresearch-results/` or `.codex-autoresearch/`.
7. Never revert unrelated user changes.
8. Keep `autoresearch-results/state.json`, `results.tsv`, and `context.json` as the authoritative run memory.
9. On repeated discards, use REFINE -> PIVOT -> Web Search -> Stop.

## Core Turn

1. Read recent `results.tsv`, `context.json`, `lessons.md`, and `git log`.
2. Pick one new, testable hypothesis.
3. Modify only declared scope.
4. Commit with `experiment: <description>`.
5. Verify with `autoresearch verify`; for noisy scalar metrics use `--repeat N --aggregate median`.
6. Run guard only after metric improvement.
7. Decide with `autoresearch decide` or verified parallel closeout.
8. Log/update state through the binary.
