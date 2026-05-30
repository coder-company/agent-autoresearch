# Installation

Autoresearch ships as a Rust binary plus agent-specific skill or command packages.

## Claude Code

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

This builds `autoresearch`, installs it on your `PATH`, and installs the Claude Code plugin hooks.

If the binary is already installed:

```bash
claude plugin add coder-company/agent-autoresearch
```

Restart Claude Code after installing the plugin.

## Codex

```text
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Then invoke:

```text
$autoresearch
```

For a local clone:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --codex
```

The installer copies `.agents/skills/autoresearch/` and validates the target path before replacing the installed skill directory.

For a project-local Codex skill install, run the installer from your target project while pointing at this clone:

```bash
/path/to/agent-autoresearch/install.sh --yes --codex --local
```

That installs to `./.codex/skills/autoresearch` in the current project. Use `--global` for the default user-wide target, or `--codex-dir` for an explicit destination.

To install the local Codex plugin package through the installer:

```bash
./install.sh --yes --codex-plugin
```

Local plugin package:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
codex plugin marketplace add .agents/plugins/marketplace.json
codex plugin install autoresearch@autoresearch-local
```

The marketplace entry points at `plugins/autoresearch/`, which packages the same maintained Codex skill plus plugin metadata.

## OpenCode

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

OpenCode commands install as `/autoresearch`, `/autoresearch_debug`, `/autoresearch_fix`, and the other underscore-mode names.

For a project-local OpenCode install, run:

```bash
/path/to/agent-autoresearch/install.sh --yes --opencode --local
```

That installs to `./.opencode` in the current project. Use `--global` for the default user-wide target, or `--opencode-dir` for an explicit OpenCode config root. The installer refuses empty, home, and parent config paths before replacing `skills/autoresearch`.

## From Source

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --all
```

Use `./install.sh` without flags for the guided installer.

## Verify The Install

```bash
autoresearch --help
autoresearch screen --command "npm test"
```

For repository contributors:

```bash
./scripts/validate_distribution.sh
./scripts/run_skill_e2e.sh binary-smoke --clean
./scripts/run_contributor_gate.sh
```

See [Getting Started](../guide/getting-started.md) and [Codex usage](../guide/autoresearch-codex.md) for first-run examples.
