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

# Release build (optimized, ~2.5MB binary)
cargo build --release
```

The release profile is configured with `opt-level = "z"`, LTO, single codegen unit, and symbol stripping for minimal binary size.

The built binary lands in `target/release/autoresearch` (or `target/debug/autoresearch` for debug builds). The `install.sh` script copies it to `bin/autoresearch` for the plugin system.

---

## Running Tests

```bash
# Run all tests (unit + integration)
cargo test

# Run a specific test
cargo test test_escalation_ladder

# Run tests with output
cargo test -- --nocapture
```

Unit tests live alongside their modules in `src/`. Integration tests live in `tests/`.

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

Hooks must respond in under 3 seconds (5 seconds for `Stop` hooks). Use `HookResponse::allow()`, `HookResponse::block(reason)`, or `HookResponse::inject(context)`.

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
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   cargo build --release
   ```
4. **Write tests** — New features need tests. Bug fixes need regression tests.
5. **Update docs** — If you change CLI behavior, update the relevant command file in `commands/`. If you change architecture, update this file.
6. **Open PR** — Describe what changed and why. Link related issues.
7. **CI must pass** — Format, lint, test, and build checks all must be green.

---

## Release Process

1. Bump version in `Cargo.toml`.
2. Update `CHANGELOG.md` (if present) with the new version's changes.
3. Ensure CI passes on `main`.
4. Tag the release: `git tag v0.x.y && git push --tags`.
5. GitHub Actions builds the release binary. Verify binary size stays under 5MB.

---

## Areas of Interest for Contributors

- **New mode implementations** — The `src/modes/` directory is empty and ready for Rust implementations of the 12 modes currently defined as command markdown files.
- **Hook improvements** — The `scout_block.rs` scope enforcement uses glob patterns but the matching is not yet implemented (marked with TODO).
- **Platform adapters** — `src/agents/claude.rs` and `src/agents/codex.rs` can be expanded with platform-specific optimizations.
- **Verification templates** — Pre-built verify command templates for common frameworks (Jest, pytest, Go test, etc.).
- **Metrics format extensions** — Support for additional output formats beyond scalar and JSON.
- **CI/CD integrations** — GitHub Actions, GitLab CI, and other pipeline integrations.
- **Performance benchmarks** — Real-world benchmarks comparing hook latency against the Node.js reference implementation.
- **Documentation** — Improving reference docs, adding examples, and writing guides.

---

## Questions?

Open an issue on [GitHub](https://github.com/coder-company/agent-autoresearch/issues) or start a discussion. We're happy to help you find the right place to contribute.
