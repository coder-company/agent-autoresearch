# Escalation Protocol

## Graduated Recovery

| Trigger | Action | What To Do |
|---------|--------|-----------|
| 3 consecutive discards | **REFINE** | Adjust within current strategy. Consult lessons. Change parameters or target files. |
| 5 consecutive discards | **PIVOT** | Abandon strategy entirely. Re-read everything. Choose fundamentally different approach. |
| 2 PIVOTs without keep | **Web Search** | Look for external solutions. Research how others solved similar problems. |
| 3 PIVOTs without keep | **Soft Blocker** | Stop. Report that human review or broader scope is needed. |

## Reset

A single `keep` resets ALL escalation counters to zero.

## After Each PIVOT

Extract a lesson to `autoresearch-results/lessons.md`:
```markdown
- 🔄 [2024-01-15 14:30] **Pivoted from: <failed strategy>** — <why it failed> (New direction: <what to try next>)
```

## After Each KEEP

Extract a positive lesson:
```markdown
- ✅ [2024-01-15 14:35] **<what worked>** — delta: +2.1 (context: <why it worked>)
```

## Lesson Consultation

Before each hypothesis, scan `lessons.md`:
- Prefer strategies that succeeded in similar contexts
- Avoid strategies that consistently failed
- Adapt successful strategies from related goals
