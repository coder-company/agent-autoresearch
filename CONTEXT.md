# Context: Domain Glossary

Reference for AI agents working with autoresearch. Defines the vocabulary used across
commands, references, and source code.

---

## Output Types (by subcommand)

| Type | Produced by | Shape |
|------|------------|-------|
| **Constraint** | `probe` | A confirmed requirement or boundary (e.g., "must support Node 16+") |
| **Finding** | `security`, `debug` | A discovered issue with severity, location, and evidence |
| **Hypothesis** | `debug`, `reason` | A testable explanation for observed behavior |
| **Scenario** | `scenario` | An edge case or failure mode with trigger condition and expected outcome |
| **Insight** | `improve` | A product improvement opportunity from ICP, market, or competitive research |
| **Prediction** | `predict` | A multi-persona assessment made before committing to a plan |
| **Lesson** | `lessons`, runtime closeout | A reusable observation persisted across runs to avoid repeating failures |

---

## Loop Shapes

Each mode uses one of these iteration patterns:

| Loop Shape | Used by | Terminates when |
|-----------|---------|-----------------|
| **Metric loop** | `improve`, `fix` | Metric reaches goal OR iteration cap |
| **Saturation loop** | `probe`, `learn` | No new information for N consecutive turns |
| **Hypothesis loop** | `debug`, `reason` | Hypothesis confirmed/falsified by evidence |
| **Refinement loop** | `predict`, `reason` | Judges converge on a single answer |
| **Exploration loop** | `scenario`, `security` | All dimensions explored OR cap hit |
| **Supervised runtime loop** | background runs | `runtime run` relaunches Codex until stop, needs_human, or restart cap |
| **Parallel closeout loop** | parallel batches | Worker results are cherry-picked, verified, guarded, and reduced to one authoritative row |
| **One-shot** | `ship`, `plan`, `evals` | Single pass, no iteration |

---

## Scoring Systems

| System | Used by | Range | Meaning |
|--------|---------|-------|---------|
| **Severity ranking** | `security`, `debug` | Critical/High/Medium/Low/Info | Impact classification |
| **Primary metric** | core loop, `fix`, `exec` | Decimal (any range) | Single number from scalar verify output or selected `metrics_json` key |
| **Criteria gates** | core loop, `exec`, parallel closeout | JSON metric thresholds | Acceptance, required-keep, and required-stop checks against structured metrics |
| **Tiered ranking** | `scenario` | Tier 1–4 | Likelihood × impact matrix |
| **Convergence** | `reason`, `predict` | 0.0–1.0 | Agreement ratio among judges/personas |
| **Saturation** | `probe`, `learn` | 0–N consecutive nulls | Turns since last new information |

---

## Key Concepts

### Iteration
One complete cycle: modify → verify → decide (keep/discard). Tracked in `results.tsv`
as a single row. Counter lives in `state.json` as `iteration: u32`.

### Workspace Root
The directory that owns `autoresearch-results/`. Runtime commands resolve status,
resume, watch, lessons, and handoff artifacts from this root or from a repo-local
`.codex-autoresearch/pointer.json`.

### Primary Repo
The git repository where normal run artifacts and authoritative commits are tracked.
Single-repo runs use the workspace root as the primary repo.

### Companion Repo
A clean git repository declared with `--companion-repo-scope PATH=SCOPE`. It receives
a repo-local pointer back to the workspace context and participates in health/runtime
dirty-worktree checks without silently widening editable scope.

### Experiment Commit
A git commit created by the agent during an iteration. If kept, it stays in history.
If discarded, it's reverted (or hard-reset, depending on rollback strategy).

### Trial
The tentative change being evaluated. Before the decide step, the trial exists as
the HEAD commit. After decide, it's either the new baseline or rolled back.

### Structured Metrics
When `--format metrics_json` is used, the verify command's final line is a JSON object.
The TSV primary metric comes from `--key`, while acceptance and required-keep criteria
can inspect any metric key in that object.

### Guard
An optional safety command (e.g., `npm test`) that must exit 0 for a keep to be valid.
Runs after the verify command. Guard failure → automatic discard regardless of metric.

### Health Preflight
Native integrity check exposed as `autoresearch health`. It reports git state, disk
headroom, verify/guard command existence, TSV/JSON integrity, and context pointer
consistency. Runtime launches and parallel closeout use this class of check before
writing authoritative state.

### Escalation
Progressive response to consecutive failures:
1. **Refine** (3 consecutive discards): try smaller, safer changes
2. **Pivot** (5 consecutive discards): change strategy entirely
3. **Web Search** (7 consecutive discards): look for external solutions
4. **Stop** (10 consecutive discards): halt and report

### Pivot
A deliberate strategy change triggered by escalation. Resets the approach without
resetting the metric. Recorded as `status: pivot` in the TSV.

### Handoff
A structured JSON file (`handoff.json`) written at the end of a mode to pass context
to a downstream command. Contains source mode, findings, config, and status.

### Lessons
Markdown entries in `lessons.md` recording what worked and what didn't. Persists across
sessions. Queried by the agent at the start of each iteration to avoid repeating failures.

### Launch Manifest
`autoresearch-results/launch.json`, written for background runtime sessions. It records
execution policy, Codex binary, workspace root, repo targets, run config, and runtime
prompt paths so detached relaunches do not depend on conversational memory.

### Runtime Snapshot
`autoresearch-results/runtime.json`, the current background controller state. It records
whether the run is running, idle, stopped, relaunching, or needs human help, plus the
supervisor recommendation and restart/stagnation counters.

### Parallel Worker
A branch-backed git worktree created by `autoresearch parallel prepare`. Worker audit
rows use iteration labels like `5a`, `5b`; the retained batch result is still one main
iteration row after `parallel closeout` verifies and guards the selected worker.

### Baseline
The initial metric measurement before any changes. Stored in `state.json` as
`baseline_metric`. All deltas are computed relative to the previous iteration's metric.

### Direction
Whether the metric should go `higher` (coverage, score) or `lower` (errors, latency).
Set once at init, immutable for the run.

### Run Tag
A timestamped identifier (e.g., `250527-1430`) for a specific run. Used in commit
messages and artifact filenames for disambiguation.

### Execution Policy
Runtime and parallel worker launches default to `danger_full_access` for detached
Codex sessions. `workspace_write` is the opt-in sandboxed path used for tests or
restricted environments.
