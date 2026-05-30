# How Autoresearch Projects Compare

There are several autoresearch implementations. Here's how they differ and when to use each.

---

## The Origin

Andrej Karpathy shared a 630-line Python script that ran **700 experiments overnight** against a single metric — modify, check, keep or discard, repeat. It found 20 optimizations no human spotted. He called it "autoresearch."

The idea was simple enough that several people generalized it:

- **uditgoenka/autoresearch** — turned it into a full Claude Code command system with 12 modes (debug, fix, security, scenario, etc.)
- **codex-autoresearch** — ported it to OpenAI's Codex with support for background runs
- **this project** — combines the best of both into a compiled binary plus maintained Claude, OpenCode, Codex skill, and Codex plugin packages

---

## Quick Comparison

| | Karpathy's | uditgoenka | codex-autoresearch | **This** |
|---|---|---|---|---|
| What it does | ML training loops | Any metric, 12 commands | Any metric, background mode | Any metric, 12 commands + runtime control |
| Install | Clone + Python | Copy .md files | Skill installer | Binary, Claude plugin, Codex skill, local Codex plugin marketplace |
| Works with | Standalone | Claude Code | Codex CLI | **Claude Code, Codex, OpenCode, any agent** |
| Commands | 1 (the loop) | 12 | Codex loop modes | **12 modes + exec + runtime + parallel closeout** |
| When it gets stuck | You restart | Refine → Pivot → Stop | Re-anchor/resume | **Refine → Pivot → Web Search → Soft Blocker** |
| Remembers across runs | No | Yes (lessons.md) | Yes (cross-run learning) | **Yes (lessons.md)** |
| Health preflight | No | Markdown checklist | Helper scripts | **Native `autoresearch health` for git/artifact/disk/verify/guard/context** |
| Background runs | No | No | Yes (daemon) | **Yes (`runtime run/start/status/supervise/stop`)** |
| Parallel experiments | No | No | Yes | **Worktree workers + verified closeout** |
| Structured metrics | No | No | Limited | **`metrics_json`, primary key, acceptance and required-keep gates** |

---

## When to Use What

**Use Karpathy's script** if you're doing ML research and want the simplest possible loop against `train.py`.

**Use uditgoenka/autoresearch** if you're on Claude Code and want something that works right now with no compilation step. It's pure markdown — the agent reads the instructions and follows the protocol. Great command surface.

**Use codex-autoresearch** if you're on Codex CLI and want background/overnight runs. It has a daemon mode where you can sleep while it works.

**Use this project** if:
- You want to use it with **multiple agents** (Claude Code, Codex, or anything else)
- You want Codex install choices: `$skill-installer`, direct `.agents` skill copy, or `plugins/autoresearch` via `.agents/plugins/marketplace.json`
- You care about **hook speed** — the safety checks fire on every tool call, and they're fast enough to be invisible
- You want a **single binary** with no Python/Node.js dependency chain
- You want the full 12-command surface **plus** native runtime control, health preflight, structured metrics, parallel verified closeout, escalation, and cross-run learning

---

## What We Borrowed

From **Karpathy**: the core philosophy — one metric, one change at a time, mechanical verification, automatic rollback, git as the experiment log.

From **uditgoenka/autoresearch**: the 12-command surface (debug, fix, security, scenario, predict, learn, reason, probe, evals, ship, plan, improve), Codex package layout ideas, the escalation ladder, the lessons log, and structured outputs per mode.

From **codex-autoresearch**: Codex-first interaction guidance, exec mode for CI/CD, background runtime control, full-access launch guidance, and multi-agent design patterns.

---

## Lineage

```
Karpathy's autoresearch
     │
     ├── uditgoenka/autoresearch (generalized to any metric, 12 commands)
     │
     ├── codex-autoresearch (background mode, parallel experiments)
     │
     └── coder-company/agent-autoresearch (compiled binary, all agents, Codex plugin package)
```
