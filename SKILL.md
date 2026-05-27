---
name: autoresearch
description: "Autonomous goal-directed iteration for coding agents. Use when the user wants to run an unattended improve-verify loop toward a measurable outcome — especially for overnight runs. Also covers debugging, fixing, security auditing, and ship-readiness workflows. Do not use for one-shot coding help or casual Q&A."
metadata:
  short-description: "Run an unattended improve-verify loop"
---

# autoresearch

Autonomous goal-directed iteration. Modify → Verify → Keep/Discard → Repeat.

## When Activated

1. Classify the request as `loop`, `plan`, `debug`, `fix`, `security`, `ship`, or `exec`.
2. Load `references/core-principles.md` and `references/runtime-protocol.md`.
3. Load `references/interaction-wizard.md` for every new interactive launch.
4. Load `references/session-resume.md` if prior artifacts are detected.
5. Use the bundled binary (`bin/autoresearch`) for all mechanical operations.

## Core Loop

1. Read context: `git log --oneline -10`, last 10 rows of `autoresearch-results/results.tsv`, `autoresearch-results/lessons.md`.
2. Define ONE specific, testable hypothesis.
3. Make ONE focused change within scope.
4. Trial commit: `git add -- <scoped-files>; git commit -m "experiment: <desc>"`
5. Verify: `autoresearch verify --command "<cmd>"`
6. Guard (if configured): `autoresearch guard --command "<cmd>"`
7. Decide: `autoresearch decide --decision <keep|discard|crash> --metric <val> --commit <sha> --description "<text>"`
8. Repeat.

## Modes

| Mode | Purpose |
|------|---------|
| `loop` | Iterate against a metric until goal reached |
| `plan` | Convert vague goal into launch-ready config |
| `debug` | Hunt bugs with falsifiable hypotheses |
| `fix` | Reduce errors to zero, one at a time |
| `security` | STRIDE + OWASP structured audit |
| `ship` | Gate and execute ship workflow |
| `exec` | Non-interactive CI/CD mode, JSON output |

## Goal Integration

### Codex (foreground)
When model-visible goal tools are available:
- After launch approval, call `get_goal`
- Reuse a matching non-complete current goal
- Or call `create_goal` with the confirmed objective when no goal exists
- Mark the goal complete with `update_goal` only when the autoresearch stop condition is satisfied
- Mark it blocked only when the run truly cannot continue without external input

### Codex (background)
- Do NOT create or mutate official Codex goals for background runs
- The runtime controller owns detached continuation
- Return a short handoff summary after launch

### Claude Code
- After launch approval, set `/goal <condition>, or stop after N turns`
- The /goal evaluator checks after each turn — if not met, another turn fires
- Goal clears automatically on success or can be cleared on blocker

## Explicit Run Modes

- For new interactive runs, require an explicit choice: **foreground** or **background**
- **Foreground**: loop runs in current session, goal tools used when available
- **Background**: launch manifest persisted, detached runtime continues overnight
- They are mutually exclusive — never both active against same artifacts

## Hard Rules

1. **Ask before act for new launches.** Scan repo, ask 1-3 confirmation rounds, require run-mode choice.
2. **Never ask after launch.** Once user says "go", keep iterating. Apply best practices on ambiguity. The user may be asleep.
3. **One change per iteration.** Atomic experiments create causality.
4. **Mechanical verification only.** Run the command, parse the number. No "looks good."
5. **Automatic rollback.** `autoresearch decide --decision discard` handles `git revert HEAD --no-edit`.
6. **Never stage artifacts.** `autoresearch-results/` stays uncommitted.
7. **Simplicity wins.** <1% gain + complexity = discard. Flat metric + simpler code = keep.
8. **When stuck, escalate.** 3 discards → REFINE. 5 → PIVOT. 3 PIVOTs → stop and report.
9. **Never push/deploy without explicit pre-approval** during the wizard phase.
10. **Progress every 5 iterations.** Run `autoresearch progress` and include in output.

## Required Config

Infer from user prompt + repo context. Confirm before starting:

- `Goal` — what to improve (natural language)
- `Scope` — file globs the agent may modify
- `Metric` — what number to optimize
- `Direction` — higher or lower
- `Verify` — command that outputs metric on final non-empty line

Optional:
- `Guard` — command that must exit 0 (regression check)
- `Iterations` — turn cap (default: unlimited)
- `Run tag` — human-readable run identifier

## Binary CLI Reference

| Command | Purpose |
|---------|---------|
| `autoresearch init --verify "..." --direction higher` | Measure baseline, create artifacts |
| `autoresearch verify --command "..."` | Run verify → JSON `{metric, exit_code, duration_ms}` |
| `autoresearch guard --command "..."` | Run guard → JSON `{passed, duration_ms}` |
| `autoresearch decide --decision keep\|discard\|crash --metric X --description "..."` | Apply decision, revert if needed, track escalation |
| `autoresearch log --iteration N --status keep --metric X --description "..."` | Append TSV row (alternative to decide) |
| `autoresearch status` | Show full state JSON |
| `autoresearch progress` | Formatted progress summary |
| `autoresearch resume` | Detect resumable prior run |
| `autoresearch lessons --search "query" --last 5` | Query lessons for strategy |
| `autoresearch evals [path]` | Analyze results: trends, plateaus |
| `autoresearch handoff --source loop --status GOAL_MET` | Write chain handoff.json |
| `autoresearch screen --command "..."` | Safety screen for dangerous patterns |
| `autoresearch hook <name>` | Plugin hook dispatch (<5ms) |

## Results Artifacts

All under `autoresearch-results/` (never committed):

| File | Purpose |
|---|---|
| `results.tsv` | Every iteration: metric, delta, status, description |
| `state.json` | Machine-readable resume snapshot |
| `escalation.json` | REFINE/PIVOT counters |
| `lessons.md` | Cross-run learning |
| `handoff.json` | Chain handoff for downstream commands |

## Quick Start

```text
$autoresearch
I want to get rid of all the `any` types in my TypeScript code
```

Agent scans repo, asks to confirm, user says "go", goal is set, loop starts. You come back to a log of experiments and a better codebase.

## References

- `references/core-principles.md` — 8 foundational rules
- `references/runtime-protocol.md` — Closeout order, state machine, TSV format, verify/guard contracts
- `references/escalation.md` — REFINE → PIVOT → Web Search → Stop ladder
- `references/interaction-wizard.md` — How to collect config from one sentence
- `references/session-resume.md` — Detect and recover interrupted runs
- `references/security-checklist.md` — STRIDE + OWASP for security mode
- `references/predict-personas.md` — Expert personas for predict mode
- `references/reason-judge-protocol.md` — Adversarial debate protocol
