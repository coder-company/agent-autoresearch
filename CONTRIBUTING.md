# Contributing to Autoresearch

Thanks for your interest in contributing! Autoresearch is a Rust binary that powers autonomous goal-directed iteration for coding agents. This guide covers everything you need to get started.

---

## Prerequisites

- **Rust toolchain** — Install via [rustup.rs](https://rustup.rs). We target stable Rust.
- **Git** — Required for the `git2` crate and for testing git-related functionality.
- **A Unix shell** — Tests and hooks execute commands via `sh -c`. Linux and macOS are fully supported.

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch

# Debug build (fast compile, larger binary)
cargo build

# Release build (optimized, about 3MB binary)
cargo build --release
```

The release profile is configured with `opt-level = "z"`, LTO, single codegen unit, and symbol stripping for minimal binary size.

The built binary lands in `target/release/autoresearch` (or `target/debug/autoresearch` for debug builds). The `install.sh` script copies the release binary to the selected PATH directory; the Claude plugin uses the tracked `bin/autoresearch` wrapper to find the installed binary at hook runtime.

---

## Running Tests

```bash
# Run the full contributor gate used before pushing
./scripts/run_contributor_gate.sh

# Run all tests (unit + integration)
cargo test

# Run a specific test
cargo test test_escalation_ladder

# Run tests with output
cargo test -- --nocapture

# Optional manual hook smoke test (requires jq and target/debug/autoresearch)
cargo build
./tests/test-hooks.sh
```

Unit tests live alongside their modules in `src/`. Integration tests live in `tests/`.
The contributor gate also checks formatting, installer shell syntax, generated distribution sync, and whitespace errors.

---

## Regenerating Agent Distribution Assets

OpenCode command files are generated from the canonical `commands/` tree, the
OpenCode `docs-manager` helper agent is maintained directly in `.opencode/agents/`,
and Codex reference/plugin packages are generated from the maintained `.agents`
skill package plus canonical `references/`:

```bash
./scripts/transform.sh
```

The script rewrites `.opencode/commands/` with underscore command names,
refreshes `.opencode/skills/autoresearch/`, preserves `.opencode/agents/`, syncs
`.agents/skills/autoresearch/references/` and skill-local agent metadata, and rebuilds
`plugins/autoresearch/skills/autoresearch/` from `.agents/skills/autoresearch/`.
Edit `.agents/skills/autoresearch/SKILL.md` directly when changing the Codex
entrypoint; edit `references/` for shared protocol docs.

Validate the generated and maintained distributions without rewriting files:

```bash
./scripts/validate_distribution.sh
```

This checks required package files, Claude and Codex marketplace metadata, `$autoresearch` invocation examples, local install docs, and closed/synced reference links in `.agents/`, `plugins/autoresearch/`, and `.opencode/`.

Run the lightweight end-to-end binary smoke:

```bash
./scripts/run_skill_e2e.sh binary-smoke --clean
```

This creates a disposable git repo and exercises `init`, `decide`, `status`, `watch`, and `evals` through the built `autoresearch` binary.
Use `./scripts/run_skill_e2e.sh multi-repo-smoke --clean` after changing workspace, companion repo, health, handoff, or runtime launch behavior.

---

## Project Structure

```
agent-autoresearch/
├── src/
│   ├── main.rs                  # CLI entry point (clap-based subcommands)
│   ├── lib.rs                   # Public module exports
│   ├── core/                    # Core engine
│   │   ├── config.rs            # RunConfig, Direction, Mode, VerifyFormat
│   │   ├── state.rs             # RunState machine + IterationStatus
│   │   ├── verify.rs            # Command execution + safety screening
│   │   ├── metrics.rs           # Metric parsing (scalar + JSON)
│   │   ├── results.rs           # TSV results logging
│   │   ├── git.rs               # Git operations via git2
│   │   ├── context.rs           # context.json + repo pointer writing
│   │   ├── health.rs            # Native git/artifact/disk/verify preflight
│   │   ├── runtime.rs           # Background runtime manifests + supervisor
│   │   └── mod.rs
│   ├── hooks/                   # Claude Code plugin hooks
│   │   ├── scout_block.rs       # Scope enforcement
│   │   ├── privacy_block.rs     # Sensitive data detection
│   │   ├── dangerous_cmd.rs     # Dangerous command blocking
│   │   ├── iteration_context.rs # TSV context injection
│   │   ├── session_init.rs      # Session startup
│   │   ├── session_end.rs       # Session cleanup
│   │   ├── simplify_gate.rs     # LOC complexity gate
│   │   ├── stop_check.rs        # Stop condition evaluation
│   │   ├── compaction_reanchor.rs # Post-compaction re-anchoring
│   │   ├── subagent_context.rs  # Subagent loop awareness
│   │   ├── dev_rules_reminder.rs # Dev rules re-injection
│   │   └── mod.rs               # HookResponse protocol + dispatch
│   ├── escalation/              # Stuck recovery
│   │   ├── pivot.rs             # Escalation ladder (Refine→Pivot→WebSearch→Stop)
│   │   ├── lessons.rs           # Lessons log (markdown-based)
│   │   └── mod.rs
│   └── agents/                  # Agent platform adapters
│       ├── claude.rs            # Claude Code specifics
│       ├── codex.rs             # Codex CLI specifics
│       └── mod.rs
├── .agents/                     # Maintained Codex skill + local marketplace root
├── .opencode/                   # OpenCode command, skill, and helper-agent package
├── plugins/autoresearch/        # Generated Codex plugin package
├── commands/                    # Slash command definitions
│   ├── autoresearch.md          # Root command
│   └── autoresearch/            # Subcommands (debug, fix, security, etc.)
├── hooks/
│   └── hooks.json               # Claude Code hook wiring
├── references/                  # Protocol documentation
├── skills/                      # Skill definitions
├── .claude-plugin/
│   └── plugin.json              # Claude Code plugin manifest
├── tests/                       # Integration tests
├── Cargo.toml
└── install.sh
```

---

## How to Add a New Mode

Autoresearch modes are self-contained workflows (e.g., `debug`, `fix`, `security`). To add a new mode:

1. **Create the mode implementation** — Add `src/modes/your_mode.rs` with the mode logic.

2. **Register in mod.rs** — Add `pub mod your_mode;` to `src/modes/mod.rs`.

3. **Add the Mode variant** — Add your mode to the `Mode` enum in `src/core/config.rs`:
   ```rust
   pub enum Mode {
       // ...existing variants...
       YourMode,
   }
   ```
   Update `default_iterations()` and `as_str()` for the new variant.

4. **Create the command file** — Add `commands/autoresearch/your_mode.md` with the slash command definition. Follow the format of existing command files (~100 lines, self-contained).

5. **Add a guide** — Create `references/your-mode-workflow.md` with the detailed protocol documentation.

6. **Add a reference** (if needed) — If the mode requires specialized reference data (like `security-checklist.md` or `predict-personas.md`), add it to `references/`.

---

## How to Add a Hook

Hooks intercept tool calls and prompt submissions in the Claude Code plugin system. To add a new hook:

1. **Create the hook implementation** — Add `src/hooks/your_hook.rs`:
   ```rust
   use super::{HookInput, HookResponse};

   pub fn run(input: Option<&HookInput>) -> HookResponse {
       let input = match input {
           Some(i) => i,
           None => return HookResponse::allow(),
       };
       // Your logic here
       HookResponse::allow()
   }
   ```

2. **Register in mod.rs** — Add `pub mod your_hook;` to `src/hooks/mod.rs` and add a match arm in `dispatch()`:
   ```rust
   "your-hook" => your_hook::run(input.as_ref()),
   ```

3. **Wire in hooks.json** — Add the hook to the appropriate event in `hooks/hooks.json`:
   ```json
   {
     "type": "command",
     "command": "${CLAUDE_PLUGIN_ROOT}/bin/autoresearch hook your-hook",
     "timeout": 3
   }
   ```

Hooks must finish within the timeout configured in `hooks/hooks.json` (currently 5 seconds). Keep handlers small and fast because several run on every tool call. Use `HookResponse::allow()`, `HookResponse::block(reason)`, or `HookResponse::inject(context)`.

---

## Code Style

- **Format** — Run `cargo fmt` before committing. CI enforces `cargo fmt --check`.
- **Lint** — Run `cargo clippy -- -D warnings`. CI treats warnings as errors.
- **Doc comments** — All public types, functions, and modules must have `///` doc comments explaining their purpose.
- **Error handling** — Use `anyhow::Result` for fallible operations. Use `thiserror` for custom error types. Prefer `.context()` over `.unwrap()`.
- **Naming** — Snake case for files and functions. Hook files use underscores (`scout_block.rs`), hook names in dispatch use hyphens (`"scout-block"`).
- **Tests** — Add `#[cfg(test)] mod tests` in each module for unit tests. Add integration tests in `tests/` for CLI-level behavior.

---

## Pull Request Process

1. **Fork and branch** — Create a feature branch from `main`.
2. **Make your changes** — Keep commits atomic. One logical change per commit.
3. **Run checks locally**:
   ```bash
   ./scripts/run_contributor_gate.sh
   ```
4. **Write tests** — New features need tests. Bug fixes need regression tests.
5. **Update docs** — If you change CLI behavior, update the relevant command file in `commands/`. If you change architecture, update this file.
6. **Open PR** — Describe what changed and why. Link related issues.
7. **CI must pass** — Format, lint, test, and build checks all must be green.

---

## Release Process

1. Run `./scripts/release.sh <version>` from a clean worktree.
2. Review the generated `docs/changelog.md` entry and amend the release commit if needed.
3. Ensure CI passes on `main`.
4. Push the release commit and tag: `git push origin main --tags`.
5. Create the GitHub release and upload `target/release/autoresearch`; the release script and CI gate both enforce the 5MB binary ceiling.

---

## Areas of Interest for Contributors

- **Mode implementations** — Expand the `src/modes/` validation/state helpers when command markdown protocols need stronger mechanical support.
- **Hook improvements** — Expand `scout_block.rs` regression coverage for more shell path forms, symlinks, and multi-root scope policies.
- **Platform adapters** — `src/agents/claude.rs` and `src/agents/codex.rs` can be expanded with platform-specific optimizations.
- **Verification templates** — Pre-built verify command templates for common frameworks (Jest, pytest, Go test, etc.).
- **Metrics format extensions** — Support for additional output formats beyond scalar and JSON.
- **CI/CD integrations** — GitHub Actions, GitLab CI, and other pipeline integrations.
- **Performance benchmarks** — Real-world benchmarks comparing hook latency against the Node.js reference implementation.
- **Documentation** — Improving reference docs, adding examples, and writing guides.

---

## Questions?

Open an issue on [GitHub](https://github.com/coder-company/agent-autoresearch/issues) or start a discussion. We're happy to help you find the right place to contribute.
