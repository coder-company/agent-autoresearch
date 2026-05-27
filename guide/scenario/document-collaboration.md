# Document Collaboration

## Context

Real-time document collaboration allows multiple users to edit simultaneously with operational transformation or CRDTs for conflict resolution. Offline editing creates divergent document states that must merge without data loss. Permission changes mid-edit, version history, and large document performance create additional complexity.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: Three users simultaneously edit a shared document while a fourth user is offline
Domain: web app
Scope: src/editor/** src/crdt/** src/sync/** src/permissions/**
Depth: standard
Focus: concurrency
```

## Generated Scenarios (sample output)

### Dimension 4: Concurrency
| # | Scenario | Severity |
|---|----------|----------|
| 1 | User A deletes paragraph while User B edits a sentence within it — delete wins and edit is silently lost | High |
| 2 | Two users format the same word simultaneously (bold vs italic) — final state depends on message ordering | Medium |
| 3 | User creates table, another user inserts row at same position — duplicate row indices, rendering breaks | High |
| 4 | Cursor position broadcast conflicts: User A sees User B's cursor at old position after User B's edit shifted content | Low |

### Dimension 5: State
| # | Scenario | Severity |
|---|----------|----------|
| 5 | Document version history shows 500 micro-edits from CRDT sync — impossible to find meaningful "versions" | Medium |
| 6 | Permission revoked for User B mid-edit — pending operations from B still in flight, applied after revocation | High |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 7 | Offline user comes online after 3 days — 2000 operations to merge, CRDT merge takes 30 seconds, UI frozen | High |
| 8 | User undoes their own change but it was already transformed against 50 other operations — undo produces unexpected result | High |
| 9 | Server crash during sync — some clients have operations the server lost — divergent document states | Critical |

### Dimension 6: Scale
| # | Scenario | Severity |
|---|----------|----------|
| 10 | Document reaches 500 pages — CRDT state is 200MB, new collaborator joining takes 45 seconds to sync | High |
| 11 | 50 simultaneous editors in same paragraph — operational transform throughput exceeds server capacity | Medium |

### Dimension 10: Data
| # | Scenario | Severity |
|---|----------|----------|
| 12 | User pastes 10MB image as inline base64 — CRDT treats it as text operation, replication bandwidth explodes | High |
| 13 | RTL and LTR text mixed in same paragraph — cursor movement and selection behave unpredictably during concurrent edits | Medium |

## Key Dimensions Explored

- **Concurrency** — CRDT/OT conflict resolution for structural edits, cursor sync
- **State** — Version history granularity, permission changes during active edits
- **Recovery** — Offline merge performance, undo after transformation, server crash divergence
- **Scale** — Large documents, many simultaneous editors, sync payload size
- **Data** — Large inline content, bidirectional text, operation size limits
