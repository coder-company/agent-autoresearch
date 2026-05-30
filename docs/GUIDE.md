# Guide

Autoresearch is a loop controller for agents: define a measurable goal, modify one thing, verify mechanically, keep or discard, and repeat.

## Core Commands

| Need | Use |
|------|-----|
| Improve a metric | `/autoresearch` or `$autoresearch` |
| Pick a metric from a vague goal | `/autoresearch:plan` or `$autoresearch plan` |
| Find a root cause | `/autoresearch:debug` or `$autoresearch debug` |
| Reduce errors to zero | `/autoresearch:fix` or `$autoresearch fix` |
| Run a security audit | `/autoresearch:security` or `$autoresearch security` |
| Ship through gates | `/autoresearch:ship` or `$autoresearch ship` |
| Analyze prior runs | `/autoresearch:evals` or `$autoresearch evals` |

## Binary Operations

The agent-facing protocols delegate stateful work to the `autoresearch` binary:

```bash
autoresearch init --verify "cat metric.txt" --direction lower
autoresearch verify --command "cat metric.txt"
autoresearch decide --decision auto --metric 4 --commit abc1234 --description "improved"
autoresearch progress
autoresearch watch --lines 20
autoresearch parallel prepare --workers 3
autoresearch parallel run --manifest autoresearch-results/parallel-manifest.json
autoresearch parallel template --workers 3 --output autoresearch-results/parallel-workers.json
autoresearch parallel closeout --batch-file autoresearch-results/parallel-workers.json
autoresearch parallel cleanup --manifest autoresearch-results/parallel-manifest.json
autoresearch evals --format json
```

Use `autoresearch runtime run` for supervised background Codex sessions and `autoresearch runtime status` / `autoresearch runtime stop` for control.

## Run Artifacts

All run state lives under `autoresearch-results/`:

```text
results.tsv
state.json
context.json
lessons.md
handoff.json
launch.json
runtime.json
runtime.log
```

Do not commit `autoresearch-results/` or `.codex-autoresearch/`.

## Detailed Guides

- [Docs Index](README.md)
- [Getting Started](../guide/getting-started.md)
- [Examples](EXAMPLES.md)
- [System Architecture](system-architecture.md)
- [Project Changelog](project-changelog.md)
- [Core Loop](../guide/autoresearch.md)
- [Codex](../guide/autoresearch-codex.md)
- [Examples by Domain](../guide/examples-by-domain.md)
- [Chains & Combinations](../guide/chains-and-combinations.md)
- [Advanced Patterns](../guide/advanced-patterns.md)
- [Hooks](../guide/hooks.md)
- [Full Guide Index](../guide/README.md)
