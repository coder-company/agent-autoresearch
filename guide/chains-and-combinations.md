# Chains and Combinations

Chain commands together with `--chain` to build multi-phase pipelines. Output from one command feeds into the next via `handoff.json`.

---

## How Chaining Works

When a command completes, it writes `autoresearch-results/handoff.json`:

```json
{
  "version": "0.1.0",
  "source": "debug",
  "status": "COMPLETE",
  "findings": [...],
  "config": {"goal": "...", "verify": "..."}
}
```

The next command in the chain reads this file and uses it as input context.

---

## Common Chains

### Debug then Fix

```
/autoresearch:debug --fix
```

Hunts bugs via hypothesis iteration, then switches to fix mode to crush found errors.

### Predict then Debug

```
/autoresearch:predict --chain debug
```

5 experts analyze the codebase first, their findings inform a targeted debug session.

### Full Quality Pipeline

```
/autoresearch:predict --chain scenario,debug,fix
```

Expert analysis → edge case generation → bug hunting → error crushing.

### Reason then Implement

```
/autoresearch:reason --chain plan,fix
```

Debate an architecture decision → generate config → fix implementation gaps.

### Probe then Loop

```
/autoresearch:probe --chain plan,autoresearch
```

Interrogate requirements until saturated → convert to config → iterate against metric.

### Probe then Reason

```
/autoresearch:probe --chain reason
```

Surface hidden constraints → debate the best approach with blind judges.

### Debug then Improve

```
/autoresearch:debug --improve
```

Hunt bugs → research improvements for your ICP based on findings.

---

## Chain Flags

| Flag | Purpose |
|------|---------|
| `--chain <targets>` | Comma-separated list of downstream commands |
| `--evals` | Propagated to all chained commands |
| `--evals-interval N` | Override checkpoint frequency in chain |

---

## Building Custom Chains

Any command that writes `handoff.json` can chain to any other command. The receiving command reads:

- `findings` — structured list of discoveries from the source
- `config` — pre-filled Goal/Scope/Metric/Verify for loop-type commands
- `status` — whether the source completed successfully

### Chain Semantics

| Source Status | Behavior |
|---------------|----------|
| `COMPLETE` | Next command runs normally |
| `GOAL_MET` | Next command runs with success context |
| `BOUNDED` | Next command runs (cap reached but not done) |
| `BLOCKED` | Chain stops, reports blocker to user |
| `ERROR` | Chain stops, reports error |

---

## Terminal Emitters

Some commands are natural chain endpoints:

- `/autoresearch:improve` — generates PRDs, consumed by external tools
- `/autoresearch:ship` — ships code, nothing follows

These still write `handoff.json` for logging but don't expect further chaining.

---

## Tips

1. **Start broad, end specific:** `predict → debug → fix` narrows focus progressively
2. **Use probe for unknowns:** When requirements are fuzzy, probe first
3. **Reason for subjective calls:** Before architecture decisions, let judges converge
4. **Loop is the workhorse:** Most chains end with the core loop or fix
5. **Evals travel:** `--evals` propagates through the entire chain for visibility
