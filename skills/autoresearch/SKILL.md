---
name: autoresearch
description: "Autonomous goal-directed iteration: modify, verify, keep/discard against any metric. Uses /goal for native continuation."
version: 0.1.0
---

# Autoresearch — Autonomous Goal-Directed Iteration

One metric. Constrained scope. Fast verification. Automatic rollback. Git as memory.

## How It Works With /goal

Autoresearch uses Claude Code's `/goal` command as the native continuation engine:

1. You describe what you want improved.
2. Autoresearch establishes baseline, scope, metric, verify command.
3. It sets `/goal` with the completion condition (metric target or zero-errors).
4. Each turn: read context → ideate → modify one thing → trial commit → verify → keep/discard → log.
5. `/goal` evaluator checks if condition is met. If not, another turn fires automatically.

You walk away. Come back to a log of experiments and a better codebase.

## Subcommands

| Command | Does | Drives With |
|---|---|---|
| `/autoresearch` | Iterate against a metric | `/goal "metric reaches <target>"` |
| `/autoresearch:plan` | Convert goal → validated config | Interactive (no goal) |
| `/autoresearch:debug` | Hunt bugs: hypothesize → test → falsify | `/goal "zero failures in <scope>"` |
| `/autoresearch:fix` | Crush errors one-by-one | `/goal "zero errors remain"` |
| `/autoresearch:security` | STRIDE + OWASP audit | `/goal "all critical/high findings resolved"` |
| `/autoresearch:ship` | 8-phase ship workflow | `/goal "PR merged and deploy healthy"` |
| `/autoresearch:scenario` | Edge case exploration | Bounded iterations |
| `/autoresearch:predict` | Multi-persona debate | Single turn (no goal) |
| `/autoresearch:learn` | Auto-generate docs | `/goal "all doc gaps filled"` |
| `/autoresearch:reason` | Adversarial refinement | `/goal "convergence: incumbent wins 3 consecutive"` |
| `/autoresearch:probe` | Requirement interrogation | Bounded iterations |
| `/autoresearch:evals` | Analyze results TSV | Single turn (no goal) |

## Core Protocol (Each Turn)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of `autoresearch-results/results.tsv`
- Run `git log --oneline -10` to see what worked/failed
- If last keep → `git diff HEAD~1` to see what improved metric
- Consult `autoresearch-results/lessons.md` for strategy insights

### Phase 2: Ideate
Choose ONE hypothesis. Good: specific, testable, atomic. Bad: vague, multi-step, untestable.

Priority:
1. Exploit last successful direction
2. Try untested idea informed by lessons
3. Simplify while preserving metric
4. Attempt bolder change when small ideas stall

### Phase 3: Modify
ONE focused change within scope. Must fit in one sentence.

### Phase 4: Trial Commit
```bash
git add -- <scoped-files-only>
git commit -m "experiment: <what changed and why>"
```

### Phase 5: Verify
Run the verify command. Use the helper:
```bash
autoresearch verify --command "<verify cmd>"
```
Returns JSON: `{"metric": "85.2", "exit_code": 0, "duration_ms": 1200}`

### Phase 6: Guard (if configured)
Run guard command. Must pass regardless of metric improvement.

### Phase 7: Decide
- **keep** — metric improved + guard passed → commit stays
- **discard** — metric flat/regressed OR guard failed → `git revert HEAD --no-edit`
- **crash** — verify/guard errored → revert, log crash

Simplicity override: marginal gain (<1%) + significant complexity = discard.

### Phase 8: Log
```bash
autoresearch log --iteration <N> --commit <sha> --metric <val> --delta <d> --status <keep|discard|crash> --description "<text>"
```

## /goal Integration

### Setting the goal
After config is confirmed, set the goal with the completion condition:

```
/goal <metric> reaches <target> as measured by <verify command>, or stop after <N> turns
```

Examples:
```
/goal test coverage reaches 90% as measured by `npm test -- --coverage | tail -1`, or stop after 50 turns
/goal zero `any` types remain in src/**/*.ts as measured by `grep -rc 'any' src/ | tail -1`, or stop after 30 turns
/goal all tests pass as measured by `pytest --tb=short; echo $?` outputting 0, or stop after 20 turns
```

### Bounded vs Unbounded
- Include "or stop after N turns" for bounded runs.
- Omit for unbounded (runs until condition met or you interrupt).

### Goal with constraints
```
/goal response time under 200ms as measured by `./bench.sh | tail -1` without breaking any existing tests (`npm test` must stay green), or stop after 40 turns
```

## Escalation (When Stuck)

| Consecutive discards | Action |
|---------------------|--------|
| 3 | **REFINE** — adjust within current strategy |
| 5 | **PIVOT** — fundamentally different approach |
| 2 PIVOTs without improvement | **Web search** — look externally |
| 3 PIVOTs without improvement | **Stop** — report blocker, clear goal |

One `keep` resets all counters.

## 8 Critical Rules

1. **One change per turn** — atomic. If it breaks, you know why.
2. **Read before write** — understand full context before modifying.
3. **Mechanical verification only** — no "looks good." Run the command.
4. **Automatic rollback** — `git revert HEAD --no-edit` on failure.
5. **Simplicity wins** — equal metric + less code = KEEP.
6. **Git is memory** — read `git log` + `git diff` before each iteration.
7. **Never stage autoresearch artifacts** — `autoresearch-results/` stays uncommitted.
8. **When stuck, think harder** — re-read lessons, combine near-misses, try radical changes.

## Results Artifacts

All under `autoresearch-results/` (never committed):
- `results.tsv` — iteration log
- `state.json` — resume snapshot
- `lessons.md` — cross-run learning

## Binary CLI Reference

The `autoresearch` binary handles mechanical operations:

| Command | Purpose |
|---------|---------|
| `autoresearch verify --command "..."` | Run verify, return JSON metric |
| `autoresearch log --iteration N ...` | Append row to results.tsv |
| `autoresearch status` | Show current state.json |
| `autoresearch hook <name>` | Plugin hook dispatch |

## Quick Start

```
/autoresearch
Goal: Increase test coverage from 72% to 90%
Scope: src/**/*.ts
Verify: npm test -- --coverage | grep "All files" | awk '{print $10}'
```

The skill:
1. Runs verify → baseline (72%)
2. Sets `/goal coverage reaches 90% as measured by verify command, or stop after 50 turns`
3. Each turn: read → ideate → modify → commit → verify → keep/discard → log
4. You come back to 90% coverage and a log of every experiment.
