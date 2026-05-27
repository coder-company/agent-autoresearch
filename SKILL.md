---
name: autoresearch
description: "Autonomous goal-directed iteration for any coding agent. Modify → Verify → Keep/Discard → Repeat. Use when the user wants to run an unattended improve-verify loop toward a measurable outcome."
metadata:
  short-description: "Run an unattended improve-verify loop"
---

# autoresearch

Autonomous goal-directed iteration. One metric, constrained scope, fast verification, automatic rollback, git as memory.

## When Activated

1. Classify request as `loop`, `plan`, `debug`, `fix`, `security`, `ship`, or `exec`.
2. Load `references/core-principles.md`.
3. For active execution modes, also load `references/runtime-protocol.md`.
4. Use the bundled binary (`bin/autoresearch`) for mechanical operations.

## Core Loop

1. Read the relevant context (git log, results TSV, lessons).
2. Define a mechanical success metric.
3. Establish baseline with `autoresearch verify --command "<cmd>"`.
4. Make ONE focused change within scope.
5. Trial commit: `git add -- <files>; git commit -m "experiment: <desc>"`.
6. Verify with the command.
7. Keep (improved) or discard (`git revert HEAD --no-edit`).
8. Log: `autoresearch log --iteration <N> --metric <val> --status <status> --description "<text>"`.
9. Repeat.

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

## Required Config

Infer from user prompt + repo context, confirm before starting:

- `Goal` — what to improve
- `Scope` — file globs
- `Metric` — what number to optimize
- `Direction` — higher or lower
- `Verify` — command that outputs metric on last line

Optional: `Guard`, `Iterations`, `Run tag`

## Hard Rules

1. Ask before starting (confirm config). After "go" — never ask again.
2. One change per iteration.
3. Mechanical verification only.
4. Automatic rollback on failure.
5. Never stage autoresearch artifacts.
6. Never push/deploy without explicit pre-approval.
7. Simplicity wins: <1% gain + complexity = discard.
8. When stuck 3+ discards → REFINE. 5+ → PIVOT. 3 PIVOTs → stop.

## Foreground vs Background

- **Foreground**: loop runs in current session.
- **Background**: `autoresearch loop` binary runs detached.

Ask the user which mode before starting.

## Binary CLI

| Command | Purpose |
|---------|---------|
| `autoresearch verify --command "..."` | Run verify, return JSON |
| `autoresearch log --iteration N ...` | Append to results.tsv |
| `autoresearch status` | Show state.json |
| `autoresearch loop` | Run full loop from stdin config |

## Results

All artifacts under `autoresearch-results/` (never committed):
- `results.tsv` — every iteration
- `state.json` — resume snapshot  
- `lessons.md` — cross-run learning

## Quick Start

```text
$autoresearch
I want to get rid of all the `any` types in my TypeScript code
```

Agent scans repo, asks to confirm, user says "go", loop starts.

## References

- `references/core-principles.md`
- `references/runtime-protocol.md`
- `references/escalation.md`
