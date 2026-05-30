# Autoresearch — Product Design Review

## Problem Statement

AI coding agents (Claude Code, Codex CLI, Cursor, etc.) need autonomous iteration to improve codebases against measurable metrics. Today, agents either:

1. **Ask after every change** — breaking flow, requiring human attention for mechanical decisions
2. **Use heavyweight orchestration** — Python/Node scripts with complex dependency chains, slow startup, runtime dependencies
3. **Have no memory across turns** — repeat failed experiments, lose context on compaction

There is no lightweight, compiled infrastructure that gives agents a tight modify→verify→keep/discard loop with git as memory, automatic rollback on failure, and escalation when stuck.

## Solution

A single compiled Rust binary (about 3MB) that provides:

- **Hook handler** — sub-5ms responses for Claude Code's plugin hook system (PreToolUse, PostToolUse, UserPromptSubmit, Stop, etc.)
- **CLI operations** — `init`, `verify`, `guard`, `decide`, `resume`, `health`, `progress`, `watch`, `lessons`, `handoff`, `exec`, plus `runtime run/start/status/supervise/stop` and `parallel prepare/run/closeout/cleanup`
- **Agent packages** — Claude plugin commands, Codex `.agents` skill/plugin package, OpenCode skill package, and shared markdown protocols for iteration loops, security audits, debugging, shipping, product improvement research, and more

The binary handles the mechanical infrastructure. The agent handles the intelligence. Clean separation.

## Target Users

| User | Integration |
|------|-------------|
| Claude Code users | Installer builds the binary and installs the plugin hooks |
| Codex CLI users | `$skill-installer` skill plus local `.agents/plugins/marketplace.json` plugin package |
| OpenCode users | Generated `.opencode/` skill and underscore command names |
| Any LLM agent | CLI called directly, skill markdown parsed by agent |

## Architecture

```
┌─────────────────┐     ┌──────────────┐     ┌───────────────┐
│ Agent (Claude/   │────▶│ autoresearch │────▶│ Git repo      │
│ Codex/other)     │     │ binary       │     │ (experiments) │
└─────────────────┘     └──────────────┘     └───────────────┘
        │                       │
        │ reads                 │ writes
        ▼                       ▼
┌─────────────────┐     ┌──────────────────────┐
│ SKILL.md /       │     │ autoresearch-results/ │
│ commands/*.md    │     │ ├── results.tsv       │
│ agent packages   │     │ ├── state.json        │
└─────────────────┘     │ ├── context.json      │
                        │ ├── lessons.md        │
                        │ ├── handoff.json      │
                        │ ├── launch.json       │
                        │ ├── runtime.json      │
                        │ └── runtime.log       │
                        └──────────────────────┘
```

## Key Metrics

| Metric | Target | Rationale |
|--------|--------|-----------|
| Hook response latency | <5ms p99 | Hooks fire on every tool use; must be invisible |
| Binary size | <5MB | Single-file distribution, no extraction needed |
| Runtime dependencies | Zero | No Node, Python, Docker. Just the binary. |
| Cold start | <10ms | First invocation must feel instant |
| Memory usage | <5MB RSS | Runs alongside the agent, not competing for resources |

## Non-Goals

- **Not a replacement for the agent itself** — the binary doesn't make decisions about what to change. It handles verification, logging, rollback, and state management.
- **Not a CI/CD system** — it runs locally alongside the agent. The `exec` mode supports CI but is not a pipeline orchestrator.
- **Not a test framework** — it calls your existing test/lint/build commands and parses their output.
- **Not a package manager** — it doesn't manage dependencies, just detects dangerous ones during security audits.

## Modes

| Mode | Purpose |
|------|---------|
| Core loop | Iterate against any numeric metric |
| Debug | Scientific bug hunting with hypothesis testing |
| Fix | Crush errors one-by-one until zero |
| Security | STRIDE + OWASP audit with red-team personas |
| Scenario | Edge case generation across 12 dimensions |
| Predict | Multi-persona expert debate |
| Reason | Adversarial refinement with blind judges |
| Probe | Requirements interrogation until saturation |
| Learn | Auto-generate documentation |
| Ship | 8-phase ship workflow |
| Improve | Research ICP needs and generate product improvement PRDs |
| Evals | Analyze iteration results |
| Plan | Convert goal → validated config |

## Success Criteria

1. Agent can iterate 25+ times without human intervention
2. Failed experiments are automatically reverted (zero pollution)
3. Cross-session memory via lessons.md survives compaction
4. Hook latency is imperceptible to the agent/user
5. Background `autoresearch runtime run` can relaunch Codex turns without corrupting artifacts
6. Parallel worker closeout produces one authoritative retained result after verification
7. Installation is one command for Claude, Codex, and OpenCode paths
