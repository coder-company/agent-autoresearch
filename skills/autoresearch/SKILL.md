---
name: autoresearch
description: "Autonomous goal-directed iteration: modify, verify, keep/discard against any metric. Uses /goal for native multi-turn continuation."
version: 0.1.0
---

# Autoresearch — Autonomous Goal-Directed Iteration

## Safety Invariants (all subcommands)
- Never push, publish, or deploy without explicit user approval.
- Bounded by default (25 iterations for the root loop; subcommands document their own defaults). Override with `Iterations: unlimited`.
- All results logged to `autoresearch-results/` directory.
- Chain handoff via `handoff.json`. Evals reads `results.tsv`.
- Never stage `autoresearch-results/` or `.codex-autoresearch/` artifacts in experiment commits.

## /goal Integration

This skill uses Claude Code's `/goal` command as the native continuation engine. After setup:
1. Baseline is measured.
2. `/goal` is set with the completion condition.
3. Each turn executes one full iteration of the protocol.
4. The /goal evaluator checks the condition after each turn — if not met, another turn fires.
5. Goal clears when condition met, iteration cap reached, or manually cleared on blocker.

## Subcommands

| Command | Does | /goal Condition |
|---|---|---|
| `/autoresearch` | Iterate against a metric | `{metric} {direction} from {baseline} toward {target}` |
| `/autoresearch:plan` | Convert a goal into validated config | No /goal (interactive) |
| `/autoresearch:debug` | Hunt bugs: hypothesize → test → falsify | `cumulative findings keep increasing` |
| `/autoresearch:fix` | Crush errors one-by-one until zero | `error count reaches 0` |
| `/autoresearch:security` | STRIDE + OWASP audit | `OWASP 10/10 + STRIDE 6/6` |
| `/autoresearch:ship` | Ship through 8 phases | `all 8 phases passed` |
| `/autoresearch:scenario` | Generate edge cases across 12 dimensions | `all 12 dimensions saturated` |
| `/autoresearch:predict` | 5 expert personas debate | No /goal (single analysis pass) |
| `/autoresearch:learn` | Scout, generate docs, validate | `all doc gaps filled` |
| `/autoresearch:reason` | Adversarial debate with blind judges | `convergence: incumbent wins N rounds` |
| `/autoresearch:probe` | 8 personas interrogate requirements | `constraint saturation reached` |
| `/autoresearch:improve` | Research ICP needs and generate product improvement PRDs | `validated improvements identified` |
| `/autoresearch:evals` | Analyze iteration results | No /goal (analysis tool) |

## Core Protocol (Each Turn)

### Phase 1: Read (git history as memory)
- Read last 10-20 lines of `autoresearch-results/results.tsv`
- Read `autoresearch-results/context.json` when present
- Run `git log --oneline -10` — see what worked/failed
- If last iteration was "keep" → run `git diff HEAD~1`
- Consult `autoresearch-results/lessons.md` for strategy insights

### Phase 2: Ideate
ONE specific, testable, atomic hypothesis. Different from all previous.

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
NEVER stage autoresearch-results/ or .codex-autoresearch/ artifacts.

### Phase 5: Verify
Run `autoresearch verify --format metrics_json --key <metric>` for structured output, or `autoresearch verify --command "<cmd>"` for scalar output.
If unparseable: rerun once. Still unparseable → crash.

### Phase 6: Guard (if configured)
Run only after metric improvement. Must exit 0.
If fails → revert regardless of improvement.

### Phase 7: Decide
- Prefer `autoresearch decide --decision auto --metric <value> --metrics-json '<json>' --commit <sha>`.
- For parallel worker batches, use `autoresearch parallel prepare` to create worktrees and prompts, `autoresearch parallel run --timeout-seconds <seconds>` to launch workers and record crashes/timeouts, `autoresearch parallel closeout --batch-file <workers.json>` to cherry-pick, verify, and retain one result, and `autoresearch parallel cleanup` after closeout. Use `autoresearch parallel template` only when worker branches already exist.
- **keep** — improved + guard passed + required keep criteria passed → commit stays
- **discard** — flat/regressed OR guard/criteria failed → binary reverts the experiment commit
- **crash** — command errored → binary reverts the experiment commit

Simplicity override: gain < 1% + added complexity = discard.
Metric unchanged + simpler code = keep.

### Phase 8: Log
The binary decision/log command appends to `autoresearch-results/results.tsv`:
```
{iteration}\t{commit|-}\t{metric}\t{delta}\t{guard}\t{status}\t{description}
```
Update `autoresearch-results/state.json`.

### Phase 9: Escalation
- 3 consecutive discards → **REFINE**: adjust within strategy, consult lessons
- 5 consecutive discards → **PIVOT**: abandon strategy entirely, fundamentally different approach
- 2 PIVOTs without keep → **Web search**: look externally
- 3 PIVOTs without keep → **Soft blocker**: clear /goal, report human input needed
- 1 keep resets ALL counters

## Critical Rules

1. **One change per turn** — atomic experiments create causality.
2. **Read before write** — git log + results TSV before modifying.
3. **Mechanical verification only** — run the command, parse the number.
4. **Automatic rollback** — `git revert HEAD --no-edit` on failure.
5. **Simplicity wins** — equal metric + less code = KEEP.
6. **Git is memory** — experiments committed, failures reverted, TSV logs all.
7. **Never stage artifacts** — `autoresearch-results/` and `.codex-autoresearch/` stay uncommitted.
8. **When stuck, escalate** — REFINE → PIVOT → Web Search → Stop.
9. **Never ask after launch** — once /goal is set, keep iterating. Apply best practices on ambiguity.
10. **Progress every 5 iterations** — report baseline vs current vs best.

## Results Artifacts

All under `autoresearch-results/` (never committed):

| File | Purpose |
|---|---|
| `results.tsv` | Every iteration: metric, delta, status, description |
| `state.json` | Machine-readable resume snapshot |
| `context.json` | Canonical run config, repo, baseline, and artifact pointers |
| `lessons.md` | Cross-run learning (positive + negative + strategic) |
| `handoff.json` | Chain handoff to downstream commands |
| `launch.json` | Background runtime launch manifest |
| `runtime.json` | Background runtime status and supervisor recommendation |
| `runtime.log` | Background runtime log |

Additionally `.codex-autoresearch/pointer.json` points tools to the canonical context artifact and must stay uncommitted.
Use `autoresearch watch --lines 20` from another terminal to tail the active results log during long-running sessions.

## TSV Format

```tsv
# metric_direction: higher
iteration	commit	metric	delta	guard	status	description
0	a1b2c3d	85.2	0	-	baseline	initial state
1	b2c3d4e	87.1	+1.9	pass	keep	add auth edge case tests
2	-	86.5	-0.6	-	discard	refactor broke 2 tests
3	c3d4e5f	88.3	+1.2	pass	keep	add error handling tests
```

## Eval Checkpoints (--evals flag)

Interval = floor(iterations / 3), min 1. Fixed 10 if unbounded.
Every interval iterations:
```
--- Eval Checkpoint (iterations {X}-{Y}) ---
Metric: {start} → {end} ({delta}) | Kept: {n}/{total} | Trend: {up/flat/down}
{one-line recommendation}
---
```
If plateau 3+ checkpoints → recommend clearing /goal early.

## Chain Handoff

Write `handoff.json` after completion:
```json
{
  "version": "0.1.0",
  "source": "<mode>",
  "timestamp": "<ISO>",
  "status": "COMPLETE|GOAL_MET|BOUNDED|BLOCKED|ERROR",
  "results_tsv": "autoresearch-results/results.tsv",
  "findings": [],
  "config": {}
}
```
Invoke next --chain target. Propagate --evals.

## References

Load only what the current mode requires:

### Always loaded
- `references/core-principles.md` — 8 foundational rules
- `references/runtime-protocol.md` — Closeout order, state machine, TSV/verify contracts
- `references/runtime-hard-invariants.md` — Primary execution checklist during active runs

### Before launch
- `references/interaction-wizard.md` — Scan → questions → confirm → launch
- `references/session-resume.md` — Detect and recover interrupted runs

### Mode-specific workflows
- `references/loop-workflow.md` — Core iteration loop
- `references/autonomous-loop-protocol.md` — Full loop detail (setup, recovery, escalation)
- `references/debug-workflow.md` — Bug hunting protocol
- `references/fix-workflow.md` — Error crushing protocol
- `references/security-workflow.md` — STRIDE + OWASP audit workflow
- `references/plan-workflow.md` — Goal → config conversion
- `references/ship-workflow.md` — 8-phase ship workflow

### Cross-cutting protocols
- `references/escalation.md` — REFINE → PIVOT → Web Search → Stop
- `references/pivot-protocol.md` — Full escalation ladder detail
- `references/lessons-protocol.md` — Cross-run learning extraction
- `references/results-logging.md` — TSV schema, row semantics, state contract
- `references/structured-output-spec.md` — Output formatting for all modes
- `references/health-check-protocol.md` — Disk, git, verify, integrity checks
- `references/parallel-experiments-protocol.md` — Git worktree parallel experiments
- `references/hypothesis-perspectives.md` — 4-lens hypothesis generation
- `references/environment-awareness.md` — Hardware/toolchain detection
- `references/web-search-protocol.md` — External research when stuck
- `references/exec-workflow.md` — Non-interactive CI/CD mode

### Domain-specific
- `references/security-checklist.md` — STRIDE + OWASP tables
- `references/predict-personas.md` — Expert persona definitions
- `references/reason-judge-protocol.md` — Adversarial debate judge protocol
