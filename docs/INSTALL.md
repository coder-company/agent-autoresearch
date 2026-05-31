# Installation

Autoresearch ships as a Rust binary plus agent-specific skill or command packages.

## Agent-Driven Install

The primary path is to give this prompt to the agent that should use Autoresearch:

```text
Install Autoresearch in this environment.

Use the installer from:
https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh

Pick the install flag for the current agent:
- Claude Code: --claude
- Codex: --codex
- OpenCode: --opencode
- If you cannot infer the agent, use --all.

Run the installer non-interactively with bash, verify `autoresearch --help`, then tell me the command I should use to start Autoresearch in this agent.
Use a global install unless I explicitly asked for a project-local install.
```

## Raw GitHub Installer

Use the raw installer when you want the source build plus agent package without cloning first:

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --claude
```

Swap the final component flag for the agent package you want:

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --codex
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --opencode
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --all
```

The raw script downloads a source archive, builds the Rust binary, and runs the same `install.sh` used by a local clone. Set `AUTORESEARCH_INSTALL_REF` to use a different branch, `AUTORESEARCH_INSTALL_REPO` to use a fork, or `AUTORESEARCH_INSTALL_ARCHIVE_URL` to provide an explicit archive URL.

## Pre-Built Binaries

Tagged releases publish `.tar.gz` archives for Linux x86_64, Linux aarch64, macOS x86_64, macOS aarch64, and Windows x86_64. Download the archive for your platform from the GitHub release, verify the adjacent `.sha256` file, and place the `autoresearch` binary on your `PATH`.

## Cargo Binstall

`Cargo.toml` includes `cargo-binstall` metadata for the same target-named release archives:

```bash
cargo binstall autoresearch
```

## Homebrew

Homebrew tap maintainers can render `packaging/homebrew/autoresearch.rb.template` with the release version and SHA-256 values from GitHub release assets, then publish it as `Formula/autoresearch.rb` in a tap.

## Claude Code

```bash
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --claude
```

This builds `autoresearch`, installs it on your `PATH`, and installs the Claude Code plugin hooks.

If the binary is already installed:

```bash
claude plugin add coder-company/agent-autoresearch
```

Restart Claude Code after installing the plugin.

Manual local Claude package:

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch

# From the project where you want local commands/skills:
mkdir -p /path/to/project/.claude
cp -R .claude/commands /path/to/project/.claude/commands
cp -R .claude/skills/autoresearch /path/to/project/.claude/skills/autoresearch
```

The `.claude/` package is generated from the same canonical command and reference files as the plugin package.

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
curl -fsSL https://raw.githubusercontent.com/coder-company/agent-autoresearch/main/install.sh | bash -s -- --yes --opencode
```

OpenCode commands install as `/autoresearch`, `/autoresearch_debug`, `/autoresearch_fix`, and the other underscore-mode names.
The package also installs the hidden `docs-manager` helper agent for focused documentation updates.

For a project-local OpenCode install, run:

```bash
/path/to/agent-autoresearch/install.sh --yes --opencode --local
```

That installs to `./.opencode` in the current project. Use `--global` for the default user-wide target, or `--opencode-dir` for an explicit OpenCode config root. The installer refuses empty, home, and parent config paths before replacing `skills/autoresearch`.

## VS Code

```bash
./install.sh --yes --vscode
```

That copies `integrations/vscode` into your VS Code extensions directory and keeps the installed extension delegated to the `autoresearch` binary on `PATH`. Use `--vscode-dir` for an explicit extensions directory.

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
autoresearch completions zsh >/tmp/_autoresearch
autoresearch manpages --output-dir /tmp/autoresearch-manpages
autoresearch config template >/tmp/autoresearch.toml
autoresearch config validate --path /tmp/autoresearch.toml
```

For repository contributors:

```bash
./scripts/validate_distribution.sh
./scripts/run_skill_e2e.sh binary-smoke --clean
./scripts/run_skill_e2e.sh runtime-smoke --clean
./scripts/run_skill_e2e.sh parallel-smoke --clean
./scripts/run_contributor_gate.sh
```

See [Getting Started](../guide/getting-started.md) and [Codex usage](../guide/autoresearch-codex.md) for first-run examples.
