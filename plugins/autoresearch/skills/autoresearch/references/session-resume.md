# Session Resume Protocol

## Detection

At session start, check for a prior interrupted run:

```bash
autoresearch resume [--cwd <path>]
```

Returns JSON:
- `{"resumable": false}` — no prior run, proceed with fresh setup
- `{"resumable": true, ...}` — prior run exists, decide resume vs fresh start

## Artifacts

All under `autoresearch-results/` (never committed):

| File | Purpose | Recovery Weight |
|------|---------|----------------|
| `state.json` | Atomic state snapshot (primary) | **Primary** |
| `results.tsv` | Iteration log | Strong |
| `lessons.md` | Cross-run learning | Moderate |
| `escalation.json` | REFINE/PIVOT counters | Moderate |
| `handoff.json` | Chain handoff for downstream | Weak |

## Recovery Priority Matrix

| # | Condition | Decision |
|---|-----------|----------|
| 1 | state.json valid + phase is "iterating" | **Full resume** — skip wizard |
| 2 | state.json valid + phase mismatch with TSV | **Mini-wizard** — 1 round confirmation |
| 3 | state.json missing + results.tsv exists | **TSV fallback** — reconstruct and confirm |
| 4 | No artifacts found | **Fresh start** — normal wizard |

## Full Resume (Priority 1)

1. Load state from `autoresearch resume` output.
2. Print resume banner:
   ```
   Resuming from iteration {N}, metric: {current} (best: {best}).
   {keeps} kept, {discards} discarded so far.
   ```
3. Re-read lessons.md for strategy context.
4. Run verify command once to confirm it still works.
5. Set /goal (Claude Code) or create_goal (Codex) with same condition.
6. Continue iterating from iteration N+1.

## Mini-Wizard (Priority 2)

1. Show what was detected (iteration, metric, status).
2. Ask ONE question: "Resume from saved state, or start fresh?"
3. If resume: re-confirm scope + verify in a single block, then launch.
4. If fresh: archive old artifacts (rename to `.prev`), run full wizard.

## Agent-Specific Goal Restoration

### Claude Code
```
/goal <metric description> <direction> from <current_metric> as measured by `<verify>`, or stop after <remaining_iterations> turns
```

### Codex (foreground)
- Call `get_goal` — if matching goal exists, reuse it
- If no goal: call `create_goal` with confirmed objective
- Continue iterating in current session

### Codex (background)
- Do not create/mutate goals for background runs
- Runtime controller owns continuation
- Resume via `autoresearch resume` + relaunch

## Fresh Start Archival

When starting fresh with prior artifacts:
```bash
# Rename (not delete) prior artifacts
mv autoresearch-results/state.json autoresearch-results/state.json.prev
mv autoresearch-results/results.tsv autoresearch-results/results.tsv.prev
mv autoresearch-results/escalation.json autoresearch-results/escalation.json.prev
# Keep lessons.md (cross-run learning persists)
```

Then proceed with normal `autoresearch init`.

## Edge Cases

- **Corrupt state.json**: rename to `.bak`, fall back to TSV reconstruction
- **Corrupt results.tsv**: start fresh, preserve lessons.md
- **Different goal**: if recovered config doesn't match current request, start fresh
- **Drift**: if verify command returns different metric than state.current_metric, log a `drift` row and continue from recalibrated state
