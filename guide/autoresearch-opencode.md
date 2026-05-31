# Autoresearch for OpenCode

OpenCode uses slash commands installed into the OpenCode config directory. The main entry point is:

```text
/autoresearch
```

Mode commands use underscore names:

| Task | OpenCode invocation |
|------|---------------------|
| Core loop | `/autoresearch` |
| Planning wizard | `/autoresearch_plan` |
| Debug | `/autoresearch_debug` |
| Fix errors | `/autoresearch_fix` |
| Security audit | `/autoresearch_security` |
| Ship workflow | `/autoresearch_ship` |
| Scenario exploration | `/autoresearch_scenario` |
| Prediction debate | `/autoresearch_predict` |
| Documentation generation | `/autoresearch_learn` |
| Adversarial reasoning | `/autoresearch_reason` |
| Requirement probing | `/autoresearch_probe` |
| Product improvement | `/autoresearch_improve` |
| Results analysis | `/autoresearch_evals` |

## Install

Recommended full install:

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --opencode
```

This builds the `autoresearch` binary and installs the OpenCode commands, skill package, and hidden `docs-manager` helper agent.

Project-local install from the target project:

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --opencode --local
```

That writes to `./.opencode` in the current project. Use this when you want Autoresearch available only inside one repository.

From a local clone:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

Use `--opencode-dir PATH` when you need an explicit OpenCode config root.

## First Run

```text
/autoresearch
Goal: Reduce the number of TypeScript any types
```

OpenCode scans the repo, proposes a metric and verify command, then waits for a clear launch confirmation. After you say "go", it should stop asking setup questions and iterate against the confirmed metric.

## Local Vs Global

Global install is the default. It writes commands and skills under your OpenCode config directory, usually `~/.config/opencode`.

Project-local install uses `--local` and writes to `./.opencode` in the current project. The installer refuses unsafe targets such as empty paths, home directories, and parent config paths before replacing `skills/autoresearch`.

## Monitoring

Use these from another terminal:

```bash
autoresearch progress --cwd /path/to/workspace
autoresearch watch --lines 20 --cwd /path/to/workspace
autoresearch lessons --last 5 --cwd /path/to/workspace
```

`watch` tails the active `autoresearch-results/results.tsv` file. Add `--once` for a single snapshot.

## Related Guides

- [Getting Started](getting-started.md)
- [Core Loop](autoresearch.md)
- [Advanced Patterns](advanced-patterns.md)
