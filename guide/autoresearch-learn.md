# Learn — `autoresearch:learn`

Auto-generate documentation by scouting the codebase, identifying gaps, writing docs, and validating them against the code.

## When to Use

- Codebase has undocumented modules
- Docs exist but are stale
- You want to validate existing documentation against code
- You need a quick architecture overview (summarize mode)

## Syntax

```
/autoresearch:learn
Mode: init
Scope: src/**/*.ts
```

## Real Examples

### Generate Docs from Scratch

```
/autoresearch:learn
Mode: init
Scope: src/**/*.ts
--depth comprehensive
Iterations: 15
```

Scouts the codebase, identifies every undocumented file, generates docs one by one, validates each against the code.

### Update Stale Docs

```
/autoresearch:learn
Mode: update
Scope: src/**/*.ts
```

Finds docs that no longer match the code and refreshes them.

### Validate Only

```
/autoresearch:learn
Mode: check
Scope: src/**/*.ts
--no-fix
```

Reports documentation issues without modifying anything.

### Quick Overview

```
/autoresearch:learn
Mode: summarize
Scope: src/**/*.ts
```

One-shot mode — produces a structured summary without iterating. No loop.

## Modes

| Mode | Purpose | Iterates? |
|------|---------|-----------|
| `init` | Generate docs from scratch | Yes |
| `update` | Refresh existing docs | Yes |
| `check` | Validate docs against code | Yes |
| `summarize` | Quick architecture overview | No (one-shot) |

## Iteration Loop (init/update/check)

Each iteration:
1. **Scout** — Find next documentation gap (undocumented → outdated → incomplete)
2. **Generate/Update** — Write or refresh docs for one file/module
3. **Validate** — Check docs against code (descriptions accurate? examples valid? links work?)
4. **Fix** — Correct validation issues (unless `--no-fix`)
5. **Commit** — `docs: document {module}`

Stops when all gaps are filled or iteration cap reached.

## Output

```
autoresearch/learn-250527-1430/
├── learn-results.tsv       # Per-file documentation status
├── summary.md              # Documentation overview
└── validation-report.md    # Issues found/fixed
```

## Flags

| Flag | Purpose |
|------|---------|
| `Mode: <type>` | init, update, check, summarize |
| `--depth <level>` | overview, standard, comprehensive |
| `--file <path>` | Document a specific file only |
| `--scan` | Force fresh codebase scout |
| `--topics <list>` | Focus: architecture, API, database, testing |
| `--no-fix` | Validate only, don't auto-fix issues |
| `--format <type>` | markdown (default), json, rst |
| `--evals` | Periodic progress reports |

## Tips

- Start with `summarize` to get an overview before committing to full documentation
- `init` prioritizes: no docs at all → outdated docs → incomplete docs
- Each doc commit is atomic — one module per commit, easy to review
- Validation catches: wrong function signatures, dead links, outdated examples
- Chain `learn → ship` to generate docs and ship them in one flow
- Use `--topics architecture` to focus on high-level docs, skip implementation details
