<div align="center">

<h2><b>Aim. Iterate. Arrive.</b></h2>

<p><i>Autonomous goal-driven experimentation for Claude Code and Codex.</i></p>

<p>
  <a href="https://github.com/coder-company/agent-autoresearch/actions"><img src="https://github.com/coder-company/agent-autoresearch/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/coder-company/agent-autoresearch"><img src="https://img.shields.io/github/stars/coder-company/agent-autoresearch?style=social" alt="Stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg" alt="MIT License"></a>
</p>

<p>
  <b>English</b> ·
  <a href="docs/i18n/README_ZH.md">🇨🇳 中文</a> ·
  <a href="docs/i18n/README_JA.md">🇯🇵 日本語</a> ·
  <a href="docs/i18n/README_KO.md">🇰🇷 한국어</a> ·
  <a href="docs/i18n/README_FR.md">🇫🇷 Français</a> ·
  <a href="docs/i18n/README_DE.md">🇩🇪 Deutsch</a> ·
  <a href="docs/i18n/README_ES.md">🇪🇸 Español</a> ·
  <a href="docs/i18n/README_PT.md">🇧🇷 Português</a> ·
  <a href="docs/i18n/README_RU.md">🇷🇺 Русский</a>
</p>

</div>

---

The idea: tell your agent what you want to improve, then walk away. It modifies your code, verifies the result, keeps or discards, and repeats. You come back to a log of experiments and a better codebase.

Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch), generalized beyond ML to anything you can verify mechanically: test coverage, type errors, latency, lint warnings, security findings, release readiness — if a command can tell whether it improved, the loop can iterate on it.

## Quick Start

Install:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

That builds the Rust binary, installs it on your `PATH`, and installs the Claude plugin hooks. If you already have the `autoresearch` binary installed, `claude plugin add coder-company/agent-autoresearch` also works.

Open your project and go:

```
You:   /autoresearch
       I want to get rid of all the `any` types in my TypeScript code

Agent: I found 47 `any` occurrences across src/**/*.ts.
       Results directory: ./autoresearch-results/
       Metric: `any` count (current: 47), direction: lower
       Verify: grep count + tsc --noEmit as guard

       Say "go" to start, or tell me what to change.

You:   Go. Run overnight.

Agent: Baseline: 47. Iterating.
```

Each improvement stacks. Each failure reverts. Everything is logged.

> **Codex users:** `$skill-installer install https://github.com/coder-company/agent-autoresearch`, then `$autoresearch`.
>
> **OpenCode users:** clone the repo and run `./install.sh --yes --opencode`. Commands install as `/autoresearch` and `/autoresearch_debug`, `/autoresearch_fix`, etc.
>
> **From source:** `git clone` + `./install.sh --yes --all`, or run `./install.sh` for the guided installer. See [Getting Started](guide/getting-started.md).

## How It Works

```
You say one sentence  →  Agent scans & confirms  →  You say "go"
                                                        │
                                                        v
                                              ┌───────────────────┐
                                              │    The Loop        │
                                              │                    │
                                              │  modify one thing  │
                                              │  trial commit      │
                                              │  run verify        │
                                              │  improved? keep    │
                                              │  worse? revert     │
                                              │  log the result    │
                                              │  repeat            │
                                              └───────────────────┘
```

That's it. The agent keeps going until the goal is reached, the iteration cap is hit, or you interrupt.

## What You Say vs What Happens

| You say | What happens |
|---------|-------------|
| "Improve my test coverage" | Iterates until target or interrupted |
| "Fix the 12 failing tests" | Repairs one by one until zero remain |
| "Why is the API returning 503?" | Hunts root cause with falsifiable hypotheses |
| "Is this code secure?" | STRIDE + OWASP audit, every finding backed by code evidence |
| "Ship it" | 8-phase checklist: test, lint, build, version, push |
| "I want to optimize but don't know what" | Scans repo, suggests metrics, generates config |
| "What could go wrong with this feature?" | Explores edge cases across 12 dimensions |
| "Should we use event sourcing here?" | Adversarial debate with blind judges until convergence |

Behind the scenes, the agent maps your sentence to the right mode. You never need to pick one.

## What It Figures Out

You don't write config. The agent infers everything from your sentence and your repo:

| What it needs | How it gets it | Example |
|--------------|----------------|---------|
| Goal | Your sentence | "get rid of all any types" |
| Scope | Scans repo structure | `src/**/*.ts` |
| Metric | Proposes based on goal + tooling | any count (current: 47) |
| Direction | Infers from "improve" / "reduce" / "eliminate" | lower |
| Verify | Matches to repo tooling | `grep` count + `tsc --noEmit` |
| Guard | Suggests a baseline-passing regression check | `npm test` |

Before starting, it always shows what it found and asks you to confirm. Then you say "go."

## When It Gets Stuck

Instead of blind retrying, the loop escalates:

| Trigger | Action |
|---------|--------|
| 3 consecutive failures | **REFINE** — adjust within current strategy |
| 5 consecutive failures | **PIVOT** — try a fundamentally different approach |
| 2 PIVOTs without progress | **Web search** — look for external solutions |
| 3 PIVOTs without progress | **Stop** — report that human input is needed |

One success resets all counters.

## Commands

| Command | Purpose |
|---------|---------|
| `/autoresearch` | The core loop — improve any metric |
| `/autoresearch:plan` | Don't know where to start? This figures it out |
| `/autoresearch:debug` | Find bugs — hypothesize, test, confirm |
| `/autoresearch:fix` | Kill errors one by one until zero remain |
| `/autoresearch:security` | Full security audit (STRIDE + OWASP) |
| `/autoresearch:ship` | Ship through 8 gates: test, lint, build, version, push |
| `/autoresearch:scenario` | "What could go wrong?" across 12 dimensions |
| `/autoresearch:predict` | Get 5 expert opinions before you act |
| `/autoresearch:learn` | Generate/update documentation automatically |
| `/autoresearch:reason` | Debate a subjective decision with blind judges |
| `/autoresearch:probe` | Interrogate requirements until nothing's ambiguous |
| `/autoresearch:improve` | Research ICP needs and generate product improvement PRDs |
| `/autoresearch:evals` | Analyze past runs: trends, plateaus, recommendations |

Just type the command. It asks for what it needs.

> **Codex:** Use `$autoresearch` then the mode as a keyword: `$autoresearch debug`.
>
> **OpenCode:** Underscore naming: `/autoresearch_debug`, `/autoresearch_fix`, etc.

## Results Log

Every iteration is recorded in `autoresearch-results/results.tsv`:

```
iteration  commit   metric  delta   status    description
0          a1b2c3d  47      0       baseline  initial any count
1          b2c3d4e  41      -6      keep      replace any in auth module
2          -        49      +8      discard   generic wrapper introduced new anys
3          d4e5f6g  38      -3      keep      type-narrow API response handlers
```

Failed experiments revert from git but stay in the log. The log is the real audit trail.

## More Features

Covered in detail in the [guide](guide/):

- **Cross-run learning** — lessons from past runs bias future hypothesis generation
- **Session resume** — interrupted runs pick up from the last consistent state
- **Background runtime control** — `autoresearch runtime run` preflights each Codex turn, manages `launch.json`, `runtime.json`, `runtime.log`, and relaunches until stop or needs-human; `start/status/supervise/stop` remain available for manual control
- **Live results tailing** — `autoresearch watch --lines 20` follows `autoresearch-results/results.tsv` from the workspace root or any repo subdirectory
- **Parallel batch closeout** — `autoresearch parallel template` creates worker result files; `autoresearch parallel closeout` compares workers, logs `5a`/`5b` audit rows, and updates retained state once for the batch
- **Chaining** — `debug --fix`, `probe --chain plan`, `predict --chain debug`
- **CI/CD mode** (`exec`) — non-interactive, JSON output, for automation pipelines
- **Dual-gate verification** — separate verify (did it improve?) and guard (did anything break?)
- **Safety hooks** — blocks dangerous commands, secrets exposure, and scope violations automatically

## FAQ

**It only makes small incremental changes. Can it try bigger ideas?**
By default the loop favors small, verifiable steps — that's by design. But it can go bigger: describe a larger hypothesis in your prompt (e.g., "try replacing the ORM with raw SQL queries and run the full benchmark"), and it will treat that as a single experiment to verify.

**Is this more for optimization than research?**
It's strongest when the goal and metric are clear — push coverage up, push errors down, push latency lower. For open-ended exploration where the direction itself is uncertain, use `/autoresearch:plan` first, then switch to the loop once you know what to measure.

**How do I stop it?**
Foreground: Ctrl+C. Background: `autoresearch runtime stop`. Or set `Iterations: N`. The agent commits before verifying, so your last successful state is always in git.

**Can it resume after interruption?**
Yes. It resumes from `autoresearch-results/state.json` automatically.

**Does it work with any language?**
Any language, any framework. If you can express success as a number and write a shell command that outputs it, autoresearch can optimize toward it.

**What if I don't know what to measure?**
`/autoresearch:plan` scans your repo, looks at your tooling, and suggests metrics with ready-to-run verify commands.

**Will it break my code?**
No. Every change is committed before verification. If it makes things worse, it reverts. If you set a Guard (e.g., `npm test`), no change persists unless all tests still pass.

## Documentation

| Doc | What it covers |
|-----|---------------|
| [Installation](docs/INSTALL.md) | Claude Code, Codex, OpenCode, source install |
| [Guide](docs/GUIDE.md) | Command map, binary operations, artifact contract |
| [Examples](docs/EXAMPLES.md) | Copy-paste configs for common goals and parallel closeout |
| [System Architecture](docs/system-architecture.md) | Binary, skill packages, artifacts, runtime flow |
| [Project Changelog](docs/project-changelog.md) | Release history entrypoint and current development track |
| [Getting Started](guide/getting-started.md) | Install, first run, what to expect |
| [Examples by Domain](guide/examples-by-domain.md) | Ready configs: coverage, types, bundle, latency, security |
| [Chains & Combinations](guide/chains-and-combinations.md) | Piping commands together |
| [Hooks](guide/hooks.md) | Safety system reference |
| [Full Guide Index](guide/) | Per-command deep dives |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |

## Acknowledgments

Built on ideas from [Karpathy's autoresearch](https://github.com/karpathy/autoresearch). Command surface inspired by [uditgoenka/autoresearch](https://github.com/uditgoenka/autoresearch). Background patterns from [codex-autoresearch](https://github.com/leo-lilinxiao/codex-autoresearch).

## License

MIT — see [LICENSE](LICENSE).
