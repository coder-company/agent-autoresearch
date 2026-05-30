# Getting Started

Five minutes from install to your first autonomous run.

---

## Install

**Claude Code:**

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

This builds the Rust binary, installs it on your `PATH`, and installs the Claude plugin hooks. If the `autoresearch` binary is already on your `PATH`, you can install only the plugin:

```
claude plugin add coder-company/agent-autoresearch
```

Restart your session after either install path.

**Codex:**

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

**OpenCode:**

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

Commands are installed with underscore names such as
`/autoresearch_debug`, `/autoresearch_fix`, and `/autoresearch_security`.

**From source:**

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --all
```

Run `./install.sh` without flags for the guided installer.

---

## Your First Run

Open your project in Claude Code or Codex and type:

```
/autoresearch
Goal: Get rid of all the any types in my TypeScript code
```

That's it. The agent figures out the rest — what files to look at, how to measure progress, what "better" means.

It'll show you what it found and ask you to confirm:

```
Confirmed:
  Goal:    Eliminate `any` types in src/**/*.ts
  Metric:  any count (current: 47), direction: lower
  Verify:  grep -rc ':any' src/ | awk -F: '{s+=$2}END{print s}'
  Guard:   tsc --noEmit

Say "go" to start, or tell me what to change.
```

Say "go". Then go do something else.

---

## What Happens Next

The agent works through your codebase one change at a time:

```
experiment: narrow auth module types              ✓ kept   47 → 41
experiment: generic API wrapper                   ✗ revert 41 → 43
experiment: type-narrow error handlers            ✓ kept   41 → 38
experiment: replace any[] with unknown[]          ✓ kept   38 → 35
experiment: refactor middleware types              ✗ revert 35 → 37
experiment: typed response interceptors           ✓ kept   35 → 31
```

Good changes stay. Bad changes disappear. Everything is logged.

---

## Check Your Results

Look at `autoresearch-results/` in your project:

**results.tsv** — every experiment, what changed, whether it helped:

```
iteration  commit   metric  delta  guard  status   description
0          a1b2c3d  47      0      -      baseline initial state
1          b2c3d4e  41      -6     pass   keep     narrow auth types
2          -        43      +2     -      discard  generic wrapper attempt
3          c3d4e5f  38      -3     pass   keep     type-narrow error handlers
```

**state.json** — where things stand right now (resumable if interrupted).

**lessons.md** — what the agent learned. Future runs start smarter.

---

## When You Don't Know What to Measure

Use the plan wizard:

```
/autoresearch:plan
Goal: Make the app faster
```

It scans your project, looks at what tools you have (test runners, linters, bundlers), and suggests metrics with ready-to-run verify commands. You pick one.

---

## Adding a Safety Net

Don't want it to break existing tests while optimizing?

```
/autoresearch
Goal: Reduce bundle size
Verify: npm run build && stat -c%s dist/main.js
Guard: npm test
```

`Guard` is a command that must pass for any change to be kept. If the metric improves but the guard fails, the change is reverted.

---

## Common Gotchas

**"The verify command doesn't work"** — The last line of your verify command's stdout must be a number. Nothing else on that line. Use `| tail -1` or `| awk '{print $4}'` to extract it.

**"Too many discards"** — This is normal. The agent handles it: after 3 discards it adjusts strategy, after 5 it tries something completely different. If it's still stuck after multiple pivots, it stops and tells you what it needs.

**"Dirty git state"** — Autoresearch needs to commit and revert freely. Commit or stash your work-in-progress first.

---

## Next Steps

- **Ready-to-use configs**: [Examples by Domain](examples-by-domain.md) — TypeScript, Python, bundle size, API latency, security
- **Hunt bugs**: `/autoresearch:debug` — hypothesize, test, confirm
- **Fix everything**: `/autoresearch:fix` — auto-detects broken tests/types/lint and fixes them
- **Chain commands**: `/autoresearch:debug --fix` — find bugs then fix them in one shot
- **Overnight run**: Add `Iterations: unlimited` and check back in the morning
