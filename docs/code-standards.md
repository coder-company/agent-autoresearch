# Code Standards

Rust conventions for the autoresearch codebase.

## Error Handling

- Use `anyhow::Result` for all fallible functions.
- Use `thiserror` for custom error types in library code that callers need to match on.
- Use `.context("descriptive message")` on every `?` — errors should be traceable.
- Never `unwrap()` in library code. `unwrap()` is acceptable only in tests.
- `expect()` is acceptable for provably infallible operations (e.g., regex compilation).

## Serialization

- All persistent types derive `Serialize, Deserialize`.
- Use `#[serde(rename_all = "snake_case")]` for enum variants.
- Use `#[serde(tag = "phase")]` for internally tagged enums (like `RunPhase`).
- Use `#[serde(default)]` for optional fields added in later versions (forward compat).
- Use `#[serde(skip_serializing_if = "Option::is_none")]` to keep JSON clean.

## Documentation

- Every public type and function has a `///` doc comment.
- Module-level `//!` doc comments describe the module's role.
- Use `# Examples` in doc comments for non-obvious APIs.

## Testing

- Unit tests live in `#[cfg(test)] mod tests` at the bottom of each file.
- Integration tests live in `tests/`.
- E2E fixtures live in `tests/e2e/fixtures/`.
- Every new CLI subcommand gets a test in `tests/cli_test.rs`.
- Every state transition gets a test in `tests/state_test.rs`.
- Target: 80%+ line coverage on `src/core/`.

## Style

- Run `cargo clippy -- -D warnings` before every commit. Zero warnings.
- Run `cargo fmt` before every commit.
- Run `./scripts/run_contributor_gate.sh` before opening a PR.
- Max line length: 100 characters (soft), 120 characters (hard).
- Prefer `match` over `if let` chains for exhaustive enum handling.
- Prefer `&str` over `String` in function parameters when ownership isn't needed.

## Performance

- Hooks must complete in <5ms. No network calls, no heavy I/O in hook handlers.
- Use `Decimal` (not `f64`) for all metric values — financial-grade precision.
- Release builds use `opt-level = "z"`, LTO, strip, `panic = "abort"`.

## Naming

- Types: `PascalCase` (e.g., `RunState`, `ResultRow`)
- Functions: `snake_case` (e.g., `record_keep`, `run_verify`)
- CLI subcommands: lowercase single words (e.g., `init`, `decide`, `evals`)
- Constants: `SCREAMING_SNAKE_CASE`
- Files: `snake_case.rs`

## Dependencies

- Minimize dependency count. Current deps are intentional:
  - `clap` — CLI parsing
  - `serde` + `serde_json` — serialization
  - `tokio` — async runtime (for exec mode)
  - `rust_decimal` — precise metric values
  - `chrono` — timestamps
  - `git2` — libgit2 bindings
  - `regex` — pattern matching
  - `anyhow` + `thiserror` — error handling
  - `glob` — file pattern matching
- Do not add dependencies without justification in the PR description.
