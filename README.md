# autoresearch

Autonomous goal-directed iteration engine for coding agents. Written in Rust.

**One metric. One command. Walk away.**

```
/autoresearch
Goal: Increase test coverage from 72% to 90%
Scope: src/**/*.ts
Verify: npm test -- --coverage | tail -1
```

You come back to a log of experiments and a better codebase.

---

## Install

### Claude Code

```
claude plugin add coder-company/agent-autoresearch
```

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

### From source

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Requires Rust toolchain (`rustup.rs`). Produces a 2.5MB binary with zero runtime dependencies.

---

## How It Works

```
You describe the goal  →  Agent confirms config  →  You say "go"
                                                       │
                                              ┌────────┴────────┐
                                              │   /goal active   │
                                              │                  │
                                              │  read context    │
                                              │  form hypothesis │
                                              │  modify ONE file │
                                              │  trial commit    │
                                              │  run verify      │
                                              │  improved? keep  │
                                              │  worse? revert   │
                                              │  log result      │
                                              │  next turn       │
                                              └──────────────────┘
```

On Claude Code, the native `/goal` command drives continuation — a separate evaluator checks your condition after each turn. On Codex, the skill manages its own loop with foreground/background modes.

---

## Commands

| Command | Purpose |
|---------|---------|
| `autoresearch` | Iterate against a metric |
| `autoresearch:plan` | Interactive wizard → config |
| `autoresearch:debug` | Hunt bugs with hypotheses |
| `autoresearch:fix` | Crush errors to zero |
| `autoresearch:security` | STRIDE + OWASP audit |
| `autoresearch:ship` | 8-phase ship workflow |
| `autoresearch:scenario` | Edge case exploration |
| `autoresearch:predict` | Multi-persona debate |
| `autoresearch:learn` | Auto-generate docs |
| `autoresearch:reason` | Adversarial refinement |
| `autoresearch:probe` | Requirement interrogation |
| `autoresearch:evals` | Analyze results |

---

## Why Rust?

| | Node.js (reference) | Rust (this) |
|---|---|---|
| Hook latency | ~80ms cold start | **<5ms** |
| Binary size | N/A (needs runtime) | **2.5MB** |
| Dependencies | Node.js 18+ | **Zero** |
| Type safety | Runtime crashes | **Compile-time** |
| State machine | Implicit (JSON) | **Enum (invalid states unrepresentable)** |
| GC pauses (overnight runs) | Yes | **No** |

Hooks fire on every tool call. 6 hooks × 75ms saved = 450ms per tool call. Over a 50-iteration run with ~200 tool calls, that's **90 seconds of wall time saved**.

---

## Architecture

```
src/
├── core/           State machine, metrics, git ops, verify, results
├── hooks/          Pre-tool-use safety + context injection
├── modes/          12 mode implementations
├── agents/         Claude Code + Codex adapters
└── escalation/     REFINE → PIVOT → Web Search → Stop
```

The binary is both:
- **Claude Code hook handler**: `autoresearch hook <name>` (called from hooks.json)
- **CLI for mechanical ops**: `autoresearch verify`, `autoresearch log`, `autoresearch status`

---

## Credits

Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch). Built on ideas from [uditgoenka/autoresearch](https://github.com/uditgoenka/autoresearch) and [codex-autoresearch](https://github.com/leo-lilinxiao/codex-autoresearch).

## License

MIT
