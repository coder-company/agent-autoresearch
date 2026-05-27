# Social Media Content Moderation

## Context

Content moderation evaluates user-generated text, images, and video against policy rules. Edge cases arise from context-dependent content (satire vs hate speech), adversarial evasion techniques, appeal processes for false positives, and the tension between over-moderation (user frustration) and under-moderation (policy violations). Multi-language support and cultural context add complexity.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: User posts content that is flagged by automated moderation and appeals the decision
Domain: web app
Scope: src/moderation/** src/appeals/** src/ml-pipeline/**
Depth: deep
Focus: security
```

## Generated Scenarios (sample output)

### Dimension 8: Security
| # | Scenario | Severity |
|---|----------|----------|
| 1 | User embeds banned text in image using steganography — text classifier passes, image classifier misses | High |
| 2 | Adversarial Unicode: "h​a​t​e" with zero-width joiners between characters bypasses keyword filter | High |
| 3 | User posts policy-violating content, gets flagged, deletes post, reposts with minor edit to reset moderation state | Medium |

### Dimension 10: Data
| # | Scenario | Severity |
|---|----------|----------|
| 4 | Post in mixed script (Arabic + English) — sentiment analyzer trained on monolingual data misclassifies | High |
| 5 | Video contains policy violation at frame 847 of 3600 — thumbnail and first 10 seconds are clean | Medium |
| 6 | Repost of news article about violence — content is educational but matches violence detection keywords exactly | High |

### Dimension 11: UX
| # | Scenario | Severity |
|---|----------|----------|
| 7 | User receives "content removed" notification but no explanation of which policy was violated — cannot write compliant appeal | High |
| 8 | Appeal approved but content already deleted from CDN — restoration shows broken media | Medium |
| 9 | Moderation decision takes 72 hours — time-sensitive content (event announcement) is useless when restored | Medium |

### Dimension 3: Permissions
| # | Scenario | Severity |
|---|----------|----------|
| 10 | Moderator reviews appeal for content posted by their alt account — no conflict-of-interest detection | High |
| 11 | Bulk moderation action removes 5K posts — 200 were false positives, no batch undo available | High |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 12 | Appeal granted: post restored but engagement metrics (likes, shares) lost during removal period | Medium |
| 13 | User account suspended, posts removed, then appeal succeeds — which posts get restored? All or just the disputed one? | High |

## Key Dimensions Explored

- **Security** — Adversarial evasion (steganography, Unicode manipulation, state resets)
- **Data** — Multi-language, mixed-media, context-dependent classification
- **UX** — Transparency of decisions, appeal experience, time-sensitivity
- **Permissions** — Moderator conflict of interest, bulk action accountability
- **Recovery** — Content restoration fidelity, engagement metric preservation
