# Development Roadmap

## v0.1.0 — Foundation (current)

- [x] Core iteration engine (init, verify, guard, decide, log)
- [x] State machine with typed transitions
- [x] TSV results + JSON state persistence
- [x] Git rollback (revert + hard-reset)
- [x] Noise-aware scalar verification repeats with aggregation
- [x] 12 subcommands with full reference docs
- [x] Exec mode for CI/CD
- [x] 11 hook handlers
- [x] Claude Code plugin + Codex skill
- [x] Codex plugin package + local marketplace entry
- [x] Thin Codex skill router with detailed binary operations in references
- [x] Escalation protocol (refine → pivot → search → stop)
- [x] Lessons log with search

## v0.2.0 — Background Mode + Parallel Experiments

- [x] Background runtime artifacts + detached Codex launch control (`autoresearch runtime start/status/supervise/stop`)
- [x] Background supervisor recommendation (`autoresearch runtime supervise`) with iteration cap, criteria, stop-condition, soft-blocker, and stagnation decisions
- [x] Background supervisor relaunch loop that automatically executes recommended relaunches (`autoresearch runtime run`)
- [x] Parallel batch templates (`autoresearch parallel template`) for editable worker result JSON
- [x] Parallel worker preparation (`autoresearch parallel prepare`) with branch-backed git worktrees, prompts, manifest, and batch file
- [x] Parallel worker launch (`autoresearch parallel run`) for prepared `codex exec` workers, including timeout/crash recording
- [x] Parallel batch closeout (`autoresearch parallel closeout`) with cherry-pick, post-merge verify/guard, fallback, worker audit rows, and one authoritative retained-state update
- [x] Parallel cleanup (`autoresearch parallel cleanup`) for worker worktrees and branches
- [x] Experiment branching — each trial on its own git branch
- [x] Branch merge strategy selection (fast-forward, squash, rebase)
- [x] `autoresearch watch` — tail results in real-time
- [x] Progress websocket for real-time monitoring
- [x] Improved evals: statistical significance testing on parallel results

## v0.3.0 — Web Search + MCP Integration

- [x] Built-in web search escalation (configurable provider command)
- [x] MCP tool server mode — expose autoresearch as an MCP tool
- [x] MCP client mode — call external MCP tools during iteration
- [x] Structured search queries from escalation context
- [x] Search result caching to avoid redundant queries
- [x] `autoresearch search` — standalone web search for the current problem

## v0.4.0 — Multi-Repo + Workspace Support

- [x] Workspace-owned artifacts (`autoresearch-results/`) and repo-local pointers for managed repos
- [x] Companion repo registration through `--companion-repo-scope PATH=SCOPE`
- [x] Companion repo preflight, health, and runtime dirty-worktree safeguards
- [x] Cross-repo change execution and rollback across companion repos
- [x] Workspace-aware scope expansion (monorepo package boundaries)
- [x] Cross-repo guard command presets
- [x] Native environment probe command for CPU, disk, container, toolchain context, and init metadata
- [x] Shared lessons across repos in a workspace

## v1.0.0 — Stable API + Ecosystem

- [x] Stable CLI API — semver guarantees on commands, flags, and output formats
- [x] Native plan command for repo-aware launch config suggestions
- [x] Native PRD generator for selected improve-mode ideas
- [x] Native scenario generator for 12-dimension edge-case artifacts
- [x] Native predict generator for five-persona review artifacts
- [x] Native reason generator for adversarial candidate debate artifacts
- [x] Native probe generator for eight-persona constraint artifacts
- [x] Adaptive eval checkpoint command for long-running loops
- [x] Native protocol re-anchor command for long-running Codex sessions
- [x] Plugin system — loadable mode definitions (TOML or YAML)
- [x] Plugin marketplace — community-contributed modes
- [x] Configuration file (`.autoresearch.toml`) for project-level defaults
- [x] Shell completions (bash, zsh, fish, elvish, PowerShell)
- [x] Man pages generation
- [x] Pre-built binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows
- [x] Homebrew formula and cargo-binstall support
- [x] Comprehensive documentation site
- [x] GitHub Action for autoresearch in CI
- [x] Metric history graphing (sparklines in terminal)
- [x] Cost tracking — estimate token/API spend per iteration
- [x] A/B experiment mode — compare two approaches head-to-head
- [x] Interactive TUI dashboard for monitoring runs
- [x] VS Code extension for run visualization with source installer support

## Future Ideas (unscheduled)

- Re-check upstream autoresearch projects before the next feature milestone
