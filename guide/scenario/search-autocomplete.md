# Search Autocomplete

## Context

Search autocomplete serves suggestions in <100ms as users type, handling typo tolerance, empty results gracefully, debouncing rapid keystrokes, and invalidating cached suggestions when underlying data changes. High concurrency from many simultaneous users typing creates thundering herd potential on popular prefixes.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: User types in search box with autocomplete suggestions from product catalog of 2M items
Domain: web app
Scope: src/search/** src/autocomplete/** src/cache/**
Depth: standard
Focus: scale
```

## Generated Scenarios (sample output)

### Dimension 6: Scale
| # | Scenario | Severity |
|---|----------|----------|
| 1 | 10K users type "iph" simultaneously after product launch — cache miss on new prefix triggers 10K identical backend queries | High |
| 2 | Product catalog bulk update invalidates 500K cached suggestion lists — cache rebuild takes 30 minutes, stale results served | Medium |
| 3 | Single character prefix "a" matches 800K products — response payload 50MB if untruncated | High |

### Dimension 10: Data
| # | Scenario | Severity |
|---|----------|----------|
| 4 | User types emoji "🎧" — search index has no emoji tokenizer, returns zero results instead of "headphones" | Medium |
| 5 | Product name contains HTML entities ("M&M's") — autocomplete displays "M&amp;M&#39;s" | Low |
| 6 | Catalog has duplicate products with different casing ("iPhone", "IPHONE", "iphone") — all three shown as separate suggestions | Medium |

### Dimension 4: Concurrency
| # | Scenario | Severity |
|---|----------|----------|
| 7 | User types fast: requests for "s", "sh", "shi", "ship" arrive out of order — "s" response (slower) overwrites "ship" response | High |
| 8 | Debounce set to 300ms but user pastes full query — single request fires but response treated as partial | Medium |

### Dimension 11: UX
| # | Scenario | Severity |
|---|----------|----------|
| 9 | Zero results for valid query due to typo — no "did you mean?" and no indication why results are empty | Medium |
| 10 | Suggestion list shows while user is still deciding — arrow-key navigation selects wrong item on re-render | Medium |
| 11 | Screen reader announces every suggestion list update on each keystroke — unusable for accessibility users | High |

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 12 | Search backend returns 503 — autocomplete shows stale cached results from 2 days ago with discontinued products | Medium |
| 13 | Network timeout on suggestion fetch — loading spinner persists indefinitely, no fallback to local history | Medium |

## Key Dimensions Explored

- **Scale** — Thundering herd on popular prefixes, cache invalidation storms
- **Data** — Unicode handling, HTML entities, deduplication
- **Concurrency** — Out-of-order response handling, debounce edge cases
- **UX** — Empty states, accessibility, keyboard navigation during updates
- **Failure** — Stale cache serving, timeout handling without fallback
