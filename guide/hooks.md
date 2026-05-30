# Hooks Reference

Autoresearch protects your codebase automatically. Hooks fire on every session — blocking dangerous commands, hiding secrets, and keeping the agent focused. You don't configure them; they just work.

---

## What They Do

Three categories:

1. **Safety** — block dangerous commands, hide secrets, enforce scope boundaries
2. **Context** — keep the agent aware of loop progress even after long sessions
3. **Session** — detect resumable runs, handle interruptions gracefully

---

## Hook Inventory

| # | Hook | Event | Purpose |
|---|------|-------|---------|
| 1 | `scout-block` | PreToolUse | Blocks reads/writes outside declared scope during active runs |
| 2 | `privacy-block` | PreToolUse | Blocks exposure of secrets, API keys, SSH keys, credentials |
| 3 | `dangerous-cmd-block` | PreToolUse | Blocks force-push, `rm -rf`, `git reset --hard`, DROP TABLE |
| 4 | `iteration-context` | UserPromptSubmit | Injects recent TSV data + loop state after compaction |
| 5 | `subagent-context` | SubagentStart | Gives subagents awareness of active loop state |
| 6 | `dev-rules-reminder` | UserPromptSubmit | Re-injects protocol rules after compaction |
| 7 | `simplify-gate` | UserPromptSubmit | Warns when last 3+ keeps were marginal (<1%) |
| 8 | `session-init` | SessionStart | Detects resumable runs, sets up project context |
| 9 | `session-end` | SessionEnd | Emits terminal notification and optional webhook summary |
| 10 | `stop-check` | Stop | Reminds agent to continue loop if iteration incomplete |
| 11 | `compaction-reanchor` | PostCompact | Re-injects full protocol after context window compaction |

---

## Hook Response Protocol

Every hook returns JSON on stdout:

```json
{
  "additionalContext": "text to inject into prompt",
  "decision": "allow" | "block",
  "reason": "human-readable reason for blocking"
}
```

- **Allow** (default): tool call proceeds normally
- **Block**: tool call is rejected, reason shown to agent
- **Inject**: adds context to the next prompt without blocking

---

## Disabling Hooks

Set environment variables to disable specific hooks:

```bash
export AR_DISABLE_SCOUT_BLOCK=1
export AR_DISABLE_PRIVACY_BLOCK=1
export AR_DISABLE_DANGEROUS_CMD_BLOCK=1
export AR_DISABLE_ITERATION_CONTEXT=1
export AR_DISABLE_SUBAGENT_CONTEXT=1
export AR_DISABLE_DEV_RULES_REMINDER=1
export AR_DISABLE_SIMPLIFY_GATE=1
export AR_DISABLE_SESSION_INIT=1
export AR_DISABLE_SESSION_END=1
export AR_DISABLE_STOP_CHECK=1
export AR_DISABLE_COMPACTION_REANCHOR=1
```

---

## Privacy Block Details

Patterns detected and blocked:

| Pattern | Example |
|---------|---------|
| API keys | `api_key=sk-...`, `token=ghp_...` |
| AWS credentials | `AKIA...` (access key ID) |
| Private keys | `-----BEGIN RSA PRIVATE KEY-----` |
| GitHub PATs | `ghp_[A-Za-z0-9]{36}` |
| OpenAI keys | `sk-[A-Za-z0-9]{48}` |
| Generic secrets | `password=`, `secret=`, `credential=` |

---

## Dangerous Command Block

Always blocked during active runs:

- `rm -rf /`, `rm -rf ~`, `rm -rf .`
- Fork bombs, `mkfs`, `dd if=/dev/zero`
- `git push --force`, `git reset --hard`
- `drop database`, `drop table`, `truncate table`
- `kubectl delete namespace`

Context-sensitive (blocked only during active runs):

- `npm publish`, `cargo publish`
- `docker push`, `helm install`
- `terraform apply`, `terraform destroy`

---

## Scope Enforcement (.ckignore)

Create a `.ckignore` file at your project root to customize which directories the scout-block hook protects:

```gitignore
# Never read these during autoresearch
node_modules/
.git/
__pycache__/
dist/
build/
target/
.env*
*.key
*.pem
```

Uses gitignore syntax. If no `.ckignore` exists, sensible defaults apply.

---

## Testing Hooks

Run hooks directly for testing:

```bash
# Test the screen command
autoresearch screen --command "npm test"
autoresearch screen --command "rm -rf /"

# Invoke a hook with simulated input
echo '{"tool_name":"Bash","tool_input":{"cmd":"rm -rf /"}}' | autoresearch hook dangerous-cmd-block
```

---

## Internals

Hooks are compiled into the `autoresearch` binary and invoked by Claude Code's plugin system:

```
autoresearch hook <name>    # reads JSON from stdin, writes response to stdout
```

Each hook returns one of: `allow` (proceed), `block` (reject with reason), or `inject` (add context to the prompt).
