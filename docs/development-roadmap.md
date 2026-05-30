# Development Roadmap

## v0.1.0 — Foundation (current)

- [x] Core iteration engine (init, verify, guard, decide, log)
- [x] State machine with typed transitions
- [x] TSV results + JSON state persistence
- [x] Git rollback (revert + hard-reset)
- [x] 12 subcommands with full reference docs
- [x] Exec mode for CI/CD
- [x] 11 hook handlers
- [x] Claude Code plugin + Codex skill
- [x] Escalation protocol (refine → pivot → search → stop)
- [x] Lessons log with search

## v0.2.0 — Background Mode + Parallel Experiments

- [x] Background runtime artifacts + detached Codex launch control (`autoresearch runtime start/status/supervise/stop`)
- [x] Background supervisor recommendation (`autoresearch runtime supervise`) with iteration cap, criteria, stop-condition, soft-blocker, and stagnation decisions
- [ ] Background supervisor relaunch loop that automatically executes recommended relaunches
- [ ] Parallel experiments runtime — run N trials concurrently, keep the best
- [ ] Experiment branching — each trial on its own git branch
- [ ] Branch merge strategy selection (fast-forward, squash, rebase)
- [ ] Progress websocket for real-time monitoring
- [ ] `autoresearch watch` — tail results in real-time
- [ ] Improved evals: statistical significance testing on parallel results

## v0.3.0 — Web Search + MCP Integration

- [ ] Built-in web search escalation (Tavily, Exa, or configurable)
- [ ] MCP tool server mode — expose autoresearch as an MCP tool
- [ ] MCP client mode — call external MCP tools during iteration
- [ ] Structured search queries from escalation context
- [ ] Search result caching to avoid redundant queries
- [ ] `autoresearch search` — standalone web search for the current problem

## v0.4.0 — Multi-Repo + Workspace Support

- [ ] Multi-repo iteration (modify repo A, verify in repo B)
- [ ] Workspace-aware scope (monorepo package boundaries)
- [ ] Cross-repo guard commands
- [ ] Shared lessons across repos in a workspace

## v1.0.0 — Stable API + Ecosystem

- [ ] Stable CLI API — semver guarantees on commands, flags, and output formats
- [ ] Plugin system — loadable mode definitions (TOML or YAML)
- [ ] Plugin marketplace — community-contributed modes
- [ ] Configuration file (`.autoresearch.toml`) for project-level defaults
- [ ] Shell completions (bash, zsh, fish)
- [ ] Man pages generation
- [ ] Pre-built binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows
- [ ] Homebrew formula and cargo-binstall support
- [ ] Comprehensive documentation site

## Future Ideas (unscheduled)

- Interactive TUI dashboard for monitoring runs
- GitHub Action for autoresearch in CI
- VS Code extension for run visualization
- Metric history graphing (sparklines in terminal)
- A/B experiment mode — compare two approaches head-to-head
- Cost tracking — estimate token/API spend per iteration
