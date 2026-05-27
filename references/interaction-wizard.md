# Interaction Wizard

User says one sentence. Agent figures out the rest through guided conversation.

## Rules

1. Accept natural language. "improve my test coverage" is a valid input.
2. Scan the repo BEFORE asking anything — read structure, configs, test commands.
3. Ask 1-3 rounds of guided questions. Each must be specific and repo-grounded.
4. Propose concrete defaults with every question. Let user confirm or correct.
5. Present a structured confirmation summary before launching.
6. Never expose internal field names (Goal, Scope, Metric, Verify) to the user.
7. After "go" — never ask again. Apply best practices on ambiguity.

## Protocol

### Step 1: Scan

Read the repo:
- Directory structure, key config files
- Test runners (package.json scripts, pytest.ini, Makefile)
- Build tools, linters, type checkers
- Existing test coverage, error counts
- Recent git history

### Step 2: Guided Questions (1-3 rounds)

| What you need | Good question |
|---|---|
| Scope | "I see `src/models/` and `src/api/` — should I touch both or just models?" |
| Metric | "Test suite reports 58% line coverage. Track that, or branch coverage?" |
| Target | "Coverage is 58%. Target 80%? 90%? As high as possible?" |
| Verify | "I can run `pytest --cov=src` — does that work?" |
| Guard | "Should `tsc --noEmit` still pass after each change?" |
| Duration | "Run 10 iterations as a test, or keep going until you stop me?" |

### Step 3: Confirmation Summary

```
Confirmed:
- Target: eliminate `any` types in src/**/*.ts
- Metric: any-type count (current: 47), direction: lower
- Verify: grep -rc ':any' src/ | awk -F: '{s+=$2}END{print s}'
- Guard: tsc --noEmit must pass
- Iterations: 30

Next: reply "go" to start, or tell me what to change.
```

### Step 4: Launch

When user says "go" / "start" / "launch":

1. Run `autoresearch init --verify "<cmd>" --direction <dir>`
2. Set the goal:

**Claude Code:**
```
/goal any-type count lower from 47 toward 0 as measured by the verify command, with each turn making one atomic change. Stop after 30 turns or when count reaches 0.
```

**Codex (foreground):**
- Call `get_goal` — reuse matching active goal
- Or call `create_goal` with objective: "Reduce any-type count to 0 in src/**/*.ts"
- Iterate in current session

**Codex (background):**
- Do not create goals — runtime controller owns continuation
- Persist launch config, hand off

3. Begin iterating immediately. No more questions.

## Internal Field Mapping

The wizard maps conversation to:

| Field | Source |
|---|---|
| Goal | User's description |
| Scope | Inferred from repo + confirmed by user |
| Metric | Proposed by agent, confirmed |
| Direction | Inferred ("improve" = higher, "reduce" = lower) |
| Verify | Agent proposes command from repo tooling |
| Guard | Suggested if regression risk exists |
| Iterations | Asked only if user wants bounded run |

## Mini-Wizard (Session Resume)

When `autoresearch resume` detects a prior run:

1. Show: prior run tag, iteration, metric, status
2. Ask ONE question: "Resume, or start fresh?"
3. If resume: condensed confirmation from saved config → launch
4. If fresh: archive → full wizard

## Binary Commands Used

```bash
# Scan + detect prior run
autoresearch resume --cwd .

# Safety screen before launch
autoresearch screen --command "pytest --cov=src"

# Initialize (baseline + artifacts)
autoresearch init --verify "pytest --cov=src | tail -1" --direction higher

# Per-iteration (called by agent each turn)
autoresearch verify --command "pytest --cov=src | tail -1"
autoresearch guard --command "tsc --noEmit"
autoresearch decide --decision keep --metric 62 --commit abc1234 --description "add auth tests"
autoresearch progress

# End of run
autoresearch handoff --source loop --status GOAL_MET
```
