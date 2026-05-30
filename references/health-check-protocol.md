# Health Check Protocol

Self-monitoring system that validates environment and run integrity at managed-runtime cycle boundaries. Catches problems before they corrupt results.

The executable companions are:

- `autoresearch status ...`
- `autoresearch health ...`

`autoresearch health` is the canonical lightweight integrity checker. It must:

- treat corrupt or unreconstructable results/state combinations as blockers,
- surface recoverable JSON/TSV divergence as warnings,
- report git state, disk headroom, verify-command availability, and result/state row consistency as structured JSON.

The extended checks below remain protocol-level review items. They may be orchestrated by the runtime or contributor gate, but `autoresearch health` must not claim to perform them unless the command actually does.

## Check Frequency

Here `<skill-root>` means the directory containing the loaded `SKILL.md`.

### Every Managed-Runtime Cycle Boundary (Lightweight)

Run before each detached Codex session. `autoresearch runtime run` repeats this native preflight before every launch/relaunch. `autoresearch runtime start` performs the same preflight for a single manual launch. Both treat missing `context.json` as a launch blocker before writing launch/runtime artifacts:

| Check | How | Failure Action |
|-------|-----|----------------|
| Disk space | `df -m . \| awk 'NR==2{print $4}'` >= 500MB | Warning at <1GB, hard blocker at <500MB |
| Git state | For single-repo runs, `git status --porcelain` shows only expected files and autoresearch-owned artifacts. For multi-repo runs, apply the same check to the primary repo and every companion repo declared in the launch manifest. | Warning if unexpected files; hard blocker if repo is corrupt |
| Verify command | Confirm the configured verify command still resolves to an executable | Hard blocker if the verify command is missing |
| Log integrity | `autoresearch resume ...` can reconstruct TSV state | Hard blocker if the TSV is corrupt |
| JSON state integrity | `autoresearch resume` reports `full_resume` or a recoverable fallback | Warning on divergence; optionally rewrite state from TSV. Hard blocker if both TSV and JSON are unusable |

### Every 10 Iterations (Extended Review)

Run at iterations 10, 20, 30, etc. only when the workflow or runtime explicitly schedules them. These are protocol-level review items, not behavior implemented by `autoresearch health` itself:

| Check | How | Failure Action |
|-------|-----|----------------|
| External modifications | `git log --oneline -5` matches expected commit sequence | Warning if unexpected commits appeared |
| Scope integrity | All in-scope files still exist | Hard blocker if scope files deleted |
| Environment drift | Re-check disk space, verify GPU if initially detected | Warning on degradation |
| Verify consistency | Run verify twice, compare results | Warning if results differ (flaky verify) |
| Guard consistency | Run guard once, confirm still passes on current state | Warning if guard started failing without code changes |
| Context health | Protocol Fingerprint Check from `runtime-hard-invariants.md` (detailed in Phase 8.7) | Re-read loaded runtime docs; log `[RE-ANCHOR]` |
| Wall-clock | Compare current iteration time with the recent rolling average | Warning if >3x average (possible resource contention) |

## Helper Output Contract

`autoresearch health` does not mutate `autoresearch-results/results.tsv`, retry verify commands, or escalate warnings over time. It returns structured JSON:

```json
{
  "decision": "ok|warn|block",
  "git_state": "clean|only_artifacts|dirty|unavailable",
  "free_mb": 1024,
  "main_rows": 4,
  "expected_rows": 4,
  "warnings": [{"code": "...", "message": "..."}],
  "blockers": [{"code": "...", "message": "..."}]
}
```

Follow-up actions belong to the caller:

- `decision = ok`: continue.
- `decision = warn`: surface the warnings and decide whether to continue, repair state, or stop.
- `decision = block`: stop or hand off to a human/operator.

## Hard Blocker Criteria

These issues stop the loop immediately:

| Issue | Reason |
|-------|--------|
| Disk < 500MB | Cannot safely commit or create files |
| Results log corrupted or missing | Cannot track progress |
| Both JSON state and TSV corrupted | Cannot recover run state; data integrity lost |
| Git repo in broken state | Cannot commit or revert |
| Verify command no longer exists | Cannot measure progress |
| All scope files deleted | Nothing to modify |

The command itself only reports the blocker. Runtime-specific revert/log/summary behavior must be implemented by the caller if desired.

## Wall-Clock Tracking

Track iteration timing to detect resource contention or environment degradation:

```
iteration_times = [t1, t2, t3, ...]
rolling_avg = average(last 5 iterations)
current_time = time of current iteration
```

Thresholds:
- Warning: current_time > 3x rolling_avg
- Concern: 3 consecutive iterations > 2x rolling_avg
- No hard blocker for timing alone (could be legitimate workload variation)

## Integration Points

- **autonomous-loop-protocol.md:** Runs as the detailed reference for Phase 8.5 (Health Check) and Phase 8.7 (Re-Anchoring). Context health feeds into the Protocol Fingerprint Check defined in `runtime-hard-invariants.md`.
- **environment-awareness.md:** Initial probes establish baselines for drift detection.
- **parallel-experiments-protocol.md:** native parallel batch closeout should reuse the lightweight health/worktree preflight before it accepts a completed batch into the authoritative run state.
- **multi-repo runs:** the command remains anchored in the primary repo for results/state/log integrity, but companion repos participate in worktree-scope checks through the launch-manifest repo list.
- **results-logging.md:** `autoresearch health` returns structured findings; append TSV rows only when the runtime explicitly chooses to log a blocker or recovery event.
- **session-resume.md:** JSON/TSV integrity checks should reuse `autoresearch resume` decisions and launch/runtime control files instead of maintaining a second row-count heuristic.
- **SKILL.md:** Listed in the load order for iterating modes.
