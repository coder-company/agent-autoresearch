# How Autoresearch Projects Compare

There are several autoresearch implementations. Here's how they differ and when to use each.

---

## The Origin

Andrej Karpathy shared a 630-line Python script that ran **700 experiments overnight** against a single metric — modify, check, keep or discard, repeat. It found 20 optimizations no human spotted. He called it "autoresearch."

The idea was simple enough that several people generalized it:

- **uditgoenka/autoresearch** — turned it into a full Claude Code command system with 13 commands (loop plus plan, debug, fix, security, scenario, improve, etc.)
- **codex-autoresearch** — ported it to OpenAI's Codex with support for background runs
- **this project** — combines the best of both into a compiled binary plus maintained Claude marketplace/plugin, OpenCode, Codex skill, and Codex plugin packages

---

## Quick Comparison

| | Karpathy's | uditgoenka | codex-autoresearch | **This** |
|---|---|---|---|---|
| What it does | ML training loops | Any metric, 13 commands | Any metric, background mode | Any metric, 13 commands + runtime control |
| Install | Clone + Python | Copy/plugin install | Skill installer | Binary, Claude marketplace/plugin, OpenCode assets, Codex skill, local Codex plugin marketplace, GitHub Action, VS Code package |
| Works with | Standalone | Claude Code | Codex CLI | **Claude Code, Codex, OpenCode, VS Code, CI, any agent** |
| Commands | 1 (the loop) | 13 | Codex loop modes | **13 command protocols + exec + runtime + dashboard + parallel closeout** |
| Token footprint | Small script | Thin routing commands | Thin skill + helper scripts | **Thin Codex router + reference-loaded protocols + native binary** |
| When it gets stuck | You restart | Refine → Pivot → Stop | Re-anchor/resume | **Refine → Pivot → Web Search → Soft Blocker** |
| Long-session drift | Manual restart | Protocol text | Protocol fingerprint check | **Native `reanchor` command + compaction hook** |
| Remembers across runs | No | Yes (lessons.md) | Yes (cross-run learning) | **Yes (lessons.md)** |
| Health preflight | No | Markdown checklist | Helper scripts | **Native `autoresearch health` for git/artifact/disk/verify/guard/context** |
| Goal planning | Manual prompt | `/autoresearch:plan` | Wizard guidance | **`/autoresearch:plan` + native `autoresearch plan --goal` suggestions** |
| Debug output | Manual notes | Hypothesis investigation reports | Protocol guidance | **`autoresearch debug` writes investigation bundles with TSV and handoff** |
| Fix output | Manual repair loop | Error-crushing protocol | Protocol guidance | **`autoresearch fix` writes repair-plan bundles under `autoresearch-results/fix`** |
| Improve research | Manual notes | Research findings + plans | Protocol guidance | **`autoresearch improve` writes findings, plan, TSV, summary, handoff, depth, and eval metadata** |
| PRD output | No | Improve-mode PRDs | No | **`autoresearch prd` writes selected-improvement PRD artifacts** |
| Security output | No | STRIDE/OWASP reports | Protocol guidance | **`autoresearch security` writes audit bundles with coverage, gates, depth, chain, and handoff** |
| Ship output | Manual release | Checklist + ship logs | Protocol guidance | **`autoresearch ship` writes 8-phase checklist bundles** |
| Scenario output | No | Scenario markdown reports | Protocol guidance | **`autoresearch scenario` writes 12-dimension edge-case artifacts** |
| Predict output | No | Persona debate reports | Protocol guidance | **`autoresearch predict` writes five-persona review artifacts** |
| Reason output | No | Adversarial reasoning reports | Protocol guidance | **`autoresearch reason` writes candidate debate artifacts** |
| Probe output | No | Requirement interrogation reports | Protocol guidance | **`autoresearch probe` writes eight-persona constraint artifacts** |
| Learn output | No | Documentation reports | Protocol guidance | **`autoresearch learn` writes summary, validation, TSV, handoff, profile, and chain metadata** |
| Background runs | No | No | Yes (daemon) | **Yes (`runtime run/start/status/supervise/stop`)** |
| Parallel experiments | No | No | Yes | **Worktree workers + verified closeout + `parallel compare` A/B batches** |
| Structured metrics | No | No | Limited | **`metrics_json`, primary key, acceptance and required-keep gates** |
| Run visualization | Terminal logs | Markdown/evals output | Background status | **Terminal dashboard, WebSocket watch, VS Code status/dashboard/watch commands** |
| Cost visibility | External GPU bill | None | None | **`autoresearch cost` projects completed, remaining, and total run spend** |
| CI automation | Manual | Manual/plugin-driven | Exec helpers | **`exec` mode plus checked-in `.github/actions/autoresearch` composite action** |

---

## When to Use What

**Use Karpathy's script** if you're doing ML research and want the simplest possible loop against `train.py`.

**Use uditgoenka/autoresearch** if you're on Claude Code and want something that works right now with no compilation step. It's pure markdown — the agent reads the instructions and follows the protocol. Great command surface.

**Use codex-autoresearch** if you're on Codex CLI and want background/overnight runs. It has a daemon mode where you can sleep while it works.

**Use this project** if:
- You want to use it with **multiple agents** (Claude Code, Codex, or anything else)
- You want install choices: Claude marketplace/plugin, OpenCode global/local assets, `$skill-installer`, direct `.agents` skill copy, or `plugins/autoresearch` via `.agents/plugins/marketplace.json`
- You care about **hook speed** — the safety checks fire on every tool call, and they're fast enough to be invisible
- You want a **single binary** with no Python/Node.js dependency chain
- You want the full 13-command surface **plus** native runtime control, health preflight, structured metrics, debug/fix/security/ship/scenario/predict/reason/probe/learn artifacts, `parallel compare`, cost estimates, dashboards, CI action packaging, and cross-run learning

---

## What We Borrowed

From **Karpathy**: the core philosophy — one metric, one change at a time, mechanical verification, automatic rollback, git as the experiment log.

From **uditgoenka/autoresearch**: the 13-command surface (loop, debug, fix, security, scenario, predict, learn, reason, probe, evals, ship, plan, improve), Codex package layout ideas, the escalation ladder, the lessons log, and structured outputs per mode.

From **codex-autoresearch**: Codex-first interaction guidance, exec mode for CI/CD, background runtime control, full-access launch guidance, and multi-agent design patterns.

---

## Lineage

```
Karpathy's autoresearch
     │
     ├── uditgoenka/autoresearch (generalized to any metric, 13 commands)
     │
     ├── codex-autoresearch (background mode, parallel experiments)
     │
     └── coder-company/agent-autoresearch (compiled binary, all agents, Claude, Codex, and OpenCode packages)
```
