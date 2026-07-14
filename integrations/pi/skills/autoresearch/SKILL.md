---
name: autoresearch
description: Autonomous goal-directed iteration for Pi: modify, verify, keep, or discard.
version: 0.1.0
---

# Autoresearch for Pi

Use Autoresearch to run a bounded, evidence-driven loop: modify one focused
change, verify it mechanically, retain an improvement, or discard it.

Pi can select this skill automatically. A user can force it with
`/skill:autoresearch [mode]`.

## Prerequisites

The Pi package provides this workflow; the native `autoresearch` binary
performs its mechanical operations.

1. Before starting a run, check that `autoresearch` is available on `PATH`.
2. If it is unavailable, explain that no run can start until the binary is
   installed. Offer a documented installation command such as `cargo binstall
   autoresearch` or the source installer. Do not silently install a dependency.
3. Do not use `autoresearch runtime run` from Pi: it launches Codex workers.
   Keep Pi-driven work in the active Pi session.

## Modes

| Mode | Purpose |
| --- | --- |
| `loop` | Improve a measurable metric |
| `plan` | Convert a goal into a launch-ready configuration |
| `debug` | Investigate a fault through falsifiable hypotheses |
| `fix` | Reduce errors one at a time |
| `security` | Perform a STRIDE + OWASP audit |
| `ship` | Assess release readiness and execute approved release steps |
| `scenario` | Explore edge cases across 12 dimensions |
| `predict` | Review a proposal with expert personas |
| `learn` | Generate or refresh documentation |
| `reason` | Refine a subjective decision with blind judging |
| `probe` | Interrogate requirements until constraints saturate |
| `improve` | Research ICP needs and create product-improvement artifacts |
| `evals` | Analyze run trends, plateaus, and comparisons |
| `exec` | Produce a non-interactive CI/CD run configuration |

If the user does not name a mode, infer one; default to `loop` for a
measurable improvement request.

## Required References

Always load `references/core-principles.md`, `references/modes.md`, and
`references/structured-output-spec.md`.

For a new or resumable run, also load `references/session-resume.md`,
`references/interaction-wizard.md`, and `references/environment-awareness.md`.

For active execution, also load `references/runtime-hard-invariants.md`,
`references/runtime-protocol.md`, and `references/results-logging.md`.

Load only the mode-specific references needed for the request.
`references/binary-operations.md` is the native CLI catalog.

## Launch Protocol

1. Inspect the repository, prior `autoresearch-results/` artifacts, and
   recent Git history.
2. Propose and confirm the goal, scope, metric, direction, verify command,
   iteration cap, and optional guard. Require explicit approval before launch.
3. Initialize or resume the run through the binary, then run
   `autoresearch health --strict` before an unattended or resumed loop when
   warnings should block it.
4. Work one iteration at a time: read the latest state, choose one testable
   hypothesis, make one scoped change, create a scoped trial commit, verify,
   run the guard after an improvement, and use the binary to decide and log
   keep/discard/crash.
5. Continue only until the confirmed target, iteration bound, or a real
   blocker. Follow REFINE → PIVOT → Web Search → Stop on repeated discards.

## Safety Invariants

1. Never push, publish, deploy, or execute external ship actions without
   explicit user approval.
2. Keep runs bounded by default. Use unlimited iterations only when the user
   explicitly asks.
3. Read before writing and make one focused change per iteration.
4. Use mechanical verification only; do not claim improvement without command
   output.
5. Never stage `autoresearch-results/` or `.codex-autoresearch/` artifacts.
6. Never revert unrelated user work.
7. Treat `autoresearch-results/state.json`, `results.tsv`, and
   `context.json` as the authoritative run memory.

## Results

Keep all run artifacts under `autoresearch-results/`. At closeout, report the
verified metric change, retained commits, discarded experiments, guard status,
and any blocker or recommended next action.
