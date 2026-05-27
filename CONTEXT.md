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
| **Insight** | `predict`, `learn`, `reason` | A synthesized conclusion from multiple perspectives or iterations |

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
| **One-shot** | `ship`, `plan`, `evals` | Single pass, no iteration |

---

## Scoring Systems

| System | Used by | Range | Meaning |
|--------|---------|-------|---------|
| **Severity ranking** | `security`, `debug` | Critical/High/Medium/Low/Info | Impact classification |
| **Composite metric** | `improve`, `fix` | Decimal (any range) | Single number from verify command |
| **Tiered ranking** | `scenario` | Tier 1–4 | Likelihood × impact matrix |
| **Convergence** | `reason`, `predict` | 0.0–1.0 | Agreement ratio among judges/personas |
| **Saturation** | `probe`, `learn` | 0–N consecutive nulls | Turns since last new information |

---

## Key Concepts

### Iteration
One complete cycle: modify → verify → decide (keep/discard). Tracked in `results.tsv`
as a single row. Counter lives in `state.json` as `iteration: u32`.

### Experiment Commit
A git commit created by the agent during an iteration. If kept, it stays in history.
If discarded, it's reverted (or hard-reset, depending on rollback strategy).

### Trial
The tentative change being evaluated. Before the decide step, the trial exists as
the HEAD commit. After decide, it's either the new baseline or rolled back.

### Guard
An optional safety command (e.g., `npm test`) that must exit 0 for a keep to be valid.
Runs after the verify command. Guard failure → automatic discard regardless of metric.

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

### Baseline
The initial metric measurement before any changes. Stored in `state.json` as
`baseline_metric`. All deltas are computed relative to the previous iteration's metric.

### Direction
Whether the metric should go `higher` (coverage, score) or `lower` (errors, latency).
Set once at init, immutable for the run.

### Run Tag
A timestamped identifier (e.g., `250527-1430`) for a specific run. Used in commit
messages and artifact filenames for disambiguation.
