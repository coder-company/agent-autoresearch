# Autoresearch for Codex

Codex uses the same protocol and binary-backed closeout as the Claude and OpenCode distributions. The entry point is the Codex skill mention syntax:

```text
$autoresearch
```

Mode names are passed as keywords:

| Task | Codex invocation |
|------|------------------|
| Core loop | `$autoresearch` |
| Planning wizard | `$autoresearch plan` |
| Debug | `$autoresearch debug` |
| Fix errors | `$autoresearch fix` |
| Security audit | `$autoresearch security` |
| Ship workflow | `$autoresearch ship` |
| Scenario exploration | `$autoresearch scenario` |
| Prediction debate | `$autoresearch predict` |
| Documentation generation | `$autoresearch learn` |
| Adversarial reasoning | `$autoresearch reason` |
| Requirement probing | `$autoresearch probe` |
| Product improvement | `$autoresearch improve` |
| Results analysis | `$autoresearch evals` |
| CI/CD JSON loop | `$autoresearch exec` |

## Install

Recommended full install:

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --codex
```

This builds the `autoresearch` binary and installs the Codex skill package. Start Codex from your project, then invoke `$autoresearch`.

Skill-only install:

```text
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Project-local skill install from the target project:

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --codex --local
```

Local plugin package:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
codex plugin marketplace add .agents/plugins/marketplace.json
codex plugin install autoresearch@autoresearch-local
```

From a local clone:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --codex
```

The installer copies the maintained Codex skill package from `.agents/skills/autoresearch/` and refuses unsafe install targets before replacing the skill directory.
Use `./install.sh --yes --codex-plugin` instead when you want the local plugin marketplace flow. The plugin marketplace path installs `plugins/autoresearch/`, which is generated from the same `.agents` skill package and includes Codex plugin metadata.

## First Run

```text
$autoresearch
I want to reduce the number of TypeScript any types
```

Codex scans the repo, proposes a metric and verify command, then waits for a clear launch confirmation. After you say "go", it should stop asking setup questions and iterate against the confirmed metric.

## Codex Access Mode

For the smoothest foreground and background runs, start Codex from your project with full workspace access:

```bash
codex --dangerously-bypass-approvals-and-sandbox
```

Background runtime launches default to `danger_full_access` for detached `codex exec` turns. Use sandboxed `workspace_write` only when you intentionally want to test the restricted path.

## Foreground vs Background

Foreground runs stay in the current Codex session. This is best when you want to watch decisions closely.

Background runs are managed by the binary runtime:

```bash
autoresearch runtime run --cwd /path/to/workspace
autoresearch runtime status --cwd /path/to/workspace
autoresearch runtime stop --cwd /path/to/workspace
```

`runtime run` preflights health, launches non-interactive Codex turns, supervises each exit, and relaunches until the supervisor returns `stop` or `needs_human`.

## Monitoring

Use these from another terminal:

```bash
autoresearch progress --cwd /path/to/workspace
autoresearch watch --lines 20 --cwd /path/to/workspace
autoresearch lessons --last 5 --cwd /path/to/workspace
```

`watch` tails the active `autoresearch-results/results.tsv` file. Add `--once` for a single snapshot.

## Artifact Contract

Codex uses the same workspace-owned artifacts as other agents:

```text
autoresearch-results/
  results.tsv
  state.json
  context.json
  lessons.md
  launch.json
  runtime.json
  runtime.log
```

Never stage `autoresearch-results/` or `.codex-autoresearch/`. They are run memory, not source.

## Common Commands

```text
$autoresearch debug
Symptom: Checkout intermittently returns 503
Iterations: 15

$autoresearch fix
Verify: npm test
Iterations: 20

$autoresearch security
Scope: src/api/**/*.ts
Depth: deep

$autoresearch exec
Config: <fully specified JSON RunConfig>

$autoresearch evals
```

`exec` is the non-interactive path for CI and scripted use. Provide all config up front, expect structured JSON lines, and use the exit code instead of conversational prompts.

## Related Guides

- [Getting Started](getting-started.md)
- [Core Loop](autoresearch.md)
- [Advanced Patterns](advanced-patterns.md)
- [Chains & Combinations](chains-and-combinations.md)
