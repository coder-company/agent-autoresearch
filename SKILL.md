---
name: autoresearch
description: "Autonomous goal-directed iteration for coding agents. Use when the user wants to run an unattended improve-verify loop toward a measurable outcome — especially for overnight runs. Also covers debugging, fixing, security auditing, and ship-readiness workflows. Do not use for one-shot coding help or casual Q&A."
metadata:
  short-description: "Run an unattended improve-verify loop"
---

# autoresearch

Autonomous goal-directed iteration. Modify → Verify → Keep/Discard → Repeat.

## When Activated

1. Classify the request as `loop`, `plan`, `debug`, `fix`, `security`, `ship`, `scenario`, `predict`, `learn`, `reason`, `probe`, `improve`, `evals`, or `exec`.
2. Load `references/core-principles.md` and `references/runtime-protocol.md`.
3. Load `references/interaction-wizard.md` for every new interactive launch.
4. Load `references/session-resume.md` if prior artifacts are detected.
5. Use the bundled binary (`bin/autoresearch`) for all mechanical operations.
6. Run `autoresearch health --strict` before unattended/background launch or resume when warnings should block launch.

## Core Loop

1. Read context: `git log --oneline -10`, last 10 rows of `autoresearch-results/results.tsv`, `autoresearch-results/lessons.md`, and `autoresearch-results/context.json` when present.
2. Define ONE specific, testable hypothesis.
3. Make ONE focused change within scope.
4. Trial commit: `git add -- <scoped-files>; git commit -m "experiment: <desc>"`
5. Verify: `autoresearch verify --format metrics_json --key "<metric-key>" --command "<cmd>"` when the command emits structured metrics; otherwise use `autoresearch verify --command "<cmd>"`.
6. Guard (if configured): `autoresearch guard --command "<cmd>"`
7. Decide: `autoresearch decide --decision auto --metric <val> --metrics-json '<json>' --commit <sha> --description "<text>"`
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
| `scenario` | Explore edge cases across 12 dimensions |
| `predict` | Debate a decision with expert personas |
| `learn` | Generate or refresh documentation |
| `reason` | Refine subjective decisions with blind judging |
| `probe` | Interrogate requirements until constraints saturate |
| `improve` | Research ICP needs and generate product improvement PRDs |
| `evals` | Analyze iteration results, plateaus, and parallel worker significance |
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
- **Background**: `autoresearch runtime run` owns the supervised Codex exec loop; `runtime start` is the lower-level detached launch primitive
- They are mutually exclusive — never both active against same artifacts

## Hard Rules

1. **Ask before act for new launches.** Scan repo, ask 1-3 confirmation rounds, require run-mode choice.
2. **Never ask after launch.** Once user says "go", keep iterating. Apply best practices on ambiguity. The user may be asleep.
3. **One change per iteration.** Atomic experiments create causality.
4. **Mechanical verification only.** Run the command, parse the number. No "looks good."
5. **Automatic rollback.** `autoresearch decide --decision discard` handles `git revert HEAD --no-edit`.
6. **Never stage artifacts.** `autoresearch-results/` and `.codex-autoresearch/` stay uncommitted.
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
- `Iterations` — turn cap (default: 25 for the root loop; subcommands document their own defaults; use `unlimited` for unbounded)
- `Run tag` — human-readable run identifier
- `Acceptance criteria` — metric thresholds to report goal readiness
- `Required keep criteria` — metric thresholds that must pass before a keep decision

## Binary CLI Reference

| Command | Purpose |
|---------|---------|
| `autoresearch init --verify "..." --direction higher --acceptance-criteria "coverage >= 90"` | Measure baseline, create artifacts, save run config/context |
| `autoresearch init --companion-repo-scope "../frontend=src/**/*.ts"` | Register a clean companion repo, persist it in context, and write its repo-local pointer |
| `autoresearch health --strict` | Preflight git/artifact/disk/verify/context state before launch or resume; fail on warnings |
| `autoresearch verify --command "..."` | Run verify → JSON `{metric, metrics, exit_code, duration_ms}` |
| `autoresearch verify --format metrics_json --key coverage --command "..."` | Parse structured metrics and select the optimization key |
| `autoresearch guard --command "..."` | Run guard → JSON `{passed, duration_ms}` |
| `autoresearch decide --decision auto --metric X --metrics-json '{...}' --description "..."` | Evaluate keep/discard, criteria gates, rollback, and escalation |
| `autoresearch parallel prepare --workers 3` | Create branch-backed worker worktrees, prompts, manifest, and batch file |
| `autoresearch parallel run --manifest autoresearch-results/parallel-manifest.json --timeout-seconds 1200` | Launch prepared worker prompts with `codex exec`; record crashes/timeouts |
| `autoresearch parallel template --workers 3 --output autoresearch-results/parallel-workers.json` | Generate an editable worker batch JSON file |
| `autoresearch parallel closeout --batch-file workers.json --merge-strategy rebase` | Merge, verify, and retain one worker; log audit rows and update retained state once |
| `autoresearch parallel cleanup --manifest autoresearch-results/parallel-manifest.json` | Remove worker worktrees and branches |
| `autoresearch log --iteration N --status keep --metric X --description "..."` | Append TSV row (alternative to decide) |
| `autoresearch status --summary` | Show compact run counters |
| `autoresearch progress` | Formatted progress summary |
| `autoresearch watch --lines 20 --format jsonl` | Tail the active results.tsv for live monitoring |
| `autoresearch lessons --add "..." --context "..."` | Append reusable strategy lessons |
| `autoresearch search --from-state --log` | Build a run-aware search query, cache provider results, and log a search row; `decide` auto-runs this on Web Search escalation when `AUTORESEARCH_SEARCH_CMD` is configured |
| `autoresearch resume` | Detect resumable prior run |
| `autoresearch runtime run` | Launch Codex exec turns, supervise after each exit, and relaunch until stop or needs_human |
| `autoresearch runtime start --dry-run` | Persist background launch/runtime artifacts; omit `--dry-run` to spawn detached Codex |
| `autoresearch runtime status` | Show saved runtime state |
| `autoresearch runtime supervise --after-run` | Recommend `relaunch`, `stop`, or `needs_human` after a detached turn |
| `autoresearch runtime stop` | Mark a background runtime stopped |
| `autoresearch lessons --search "query" --last 5` | Query lessons for strategy |
| `autoresearch evals [path]` | Analyze results: trends, plateaus |
| `autoresearch api --format json` | Emit the stable CLI command/flag manifest and semver policy |
| `autoresearch scope expand --format json` | Resolve primary and companion repo scopes with package-root annotations |
| `autoresearch guard-presets --format json` | Suggest cross-repo guard commands for primary and companion repositories |
| `autoresearch handoff --source loop --status GOAL_MET` | Write chain handoff.json |
| `autoresearch screen --command "..."` | Safety screen for dangerous patterns |
| `autoresearch hook <name>` | Plugin hook dispatch (<5ms) |

## Results Artifacts

All under `autoresearch-results/` (never committed):

| File | Purpose |
|---|---|
| `results.tsv` | Every iteration: metric, delta, status, description |
| `state.json` | Machine-readable resume snapshot |
| `context.json` | Canonical run config, repo, baseline, and artifact pointers |
| `escalation.json` | REFINE/PIVOT counters |
| `lessons.md` | Cross-run learning |
| `handoff.json` | Chain handoff for downstream commands |
| `launch.json` | Background runtime launch manifest |
| `runtime.json` | Background runtime status |
| `runtime.log` | Background runtime log |

Additionally `.codex-autoresearch/pointer.json` points tools to the canonical context artifact and must stay uncommitted.

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
