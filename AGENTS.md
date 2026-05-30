# AGENTS.md — Autoresearch

> Drop this file into your project root. Any AI agent can use Autoresearch immediately.

## What is Autoresearch?

Autonomous goal-directed iteration. One metric, constrained scope, fast verification, automatic rollback, git as memory.

**Core loop:** Modify → Verify → Keep/Discard → Repeat.

---

## Installation

### Claude Code

```
claude plugin add coder-company/agent-autoresearch
```

Restart session. Commands available as `/autoresearch` and `/autoresearch:<subcommand>`.

**Pro tip:** Use `/goal` for fully autonomous multi-turn execution:
```
/autoresearch
Goal: Increase test coverage from 72% to 90%
Scope: src/**/*.ts
Verify: npm test -- --coverage | tail -1
```

### Codex CLI

```text
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Then: `$autoresearch`

### Manual (any agent)

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

---

## Commands

| Command | Purpose |
|---------|---------|
| `autoresearch` | Autonomous iteration loop against a metric |
| `autoresearch:plan` | Interactive wizard: Goal → Scope, Metric, Verify config |
| `autoresearch:debug` | Bug hunting — hypothesize, test, falsify, repeat |
| `autoresearch:fix` | Error repair — one fix per iteration until zero |
| `autoresearch:security` | STRIDE + OWASP security audit |
| `autoresearch:ship` | Ship workflow — checklist, test, PR, deploy, verify |
| `autoresearch:scenario` | Edge case exploration across 12 dimensions |
| `autoresearch:predict` | Multi-persona expert debate before acting |
| `autoresearch:learn` | Auto-generate documentation |
| `autoresearch:reason` | Adversarial refinement with blind judges |
| `autoresearch:probe` | Requirement interrogation until saturation |
| `autoresearch:evals` | Analyze iteration results: trends, plateaus |

---

## Quick Start

### Claude Code (with /goal)
```
/autoresearch
Goal: Reduce any-types to zero
Scope: src/**/*.ts
Verify: grep -rc ':any' src/ | awk -F: '{s+=$2}END{print s}'
Iterations: 30
```

### Codex
```
$autoresearch
I want to get rid of all the `any` types in my TypeScript code
```

---

## Configuration

| Field | Required | Description |
|-------|----------|-------------|
| `Goal` | Yes | What to achieve (plain language) |
| `Scope` | Yes | Glob patterns for modifiable files |
| `Metric` | Yes | What number to optimize |
| `Verify` | Yes | Command that outputs a scalar metric or final-line JSON metrics |
| `Guard` | No | Safety command that must exit 0 |
| `Iterations` | No | Turn cap (default: 500) |
| `Direction` | No | `higher` or `lower` |
| `Acceptance criteria` | No | Metric thresholds for stopping |
| `Required keep criteria` | No | Metric thresholds every keep must satisfy |

---

## 8 Critical Rules

1. **One change per turn** — atomic experiments create causality.
2. **Read before write** — git log + results TSV before modifying.
3. **Mechanical verification only** — run the command, parse the number.
4. **Automatic rollback** — `git revert HEAD --no-edit` on failure.
5. **Simplicity wins** — equal metric + less code = KEEP.
6. **Git is memory** — experiments committed, failures reverted, TSV logs all.
7. **Never stage artifacts** — `autoresearch-results/` and `.codex-autoresearch/` stay uncommitted.
8. **When stuck, escalate** — REFINE → PIVOT → Web Search → Stop.

---

## Architecture

Written in Rust. Single 2.5MB binary. Sub-5ms hook execution. Zero runtime dependencies.

```
agent-autoresearch/
├── .claude-plugin/plugin.json    ← Claude Code plugin manifest
├── hooks/hooks.json              ← Hook definitions
├── skills/autoresearch/SKILL.md  ← Claude Code skill
├── commands/autoresearch.md      ← Root command
├── commands/autoresearch/*.md    ← Subcommands
├── SKILL.md                      ← Codex skill (repo root)
├── references/                   ← Shared protocol docs
├── src/                          ← Rust source
├── bin/autoresearch              ← Built binary (gitignored)
└── install.sh                    ← Build + install
```

---

## License

MIT
