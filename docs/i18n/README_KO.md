<div align="center">

# autoresearch

**코딩 에이전트를 위한 자율 목표 지향 반복 엔진. Rust로 작성.**

*"목표를 설정 → 에이전트가 루프를 실행 → 결과를 확인"*

[English](../../README.md) · [中文](README_ZH.md) · [日本語](README_JA.md) · **한국어** · [Français](README_FR.md) · [Deutsch](README_DE.md) · [Español](README_ES.md) · [Português](README_PT.md) · [Русский](README_RU.md)

</div>

---

## 작동 방식

```
목표를 설명  →  에이전트가 설정을 확인  →  "시작"이라고 말함
                                            │
                                   ┌────────┴────────┐
                                   │   루프 실행 중     │
                                   │                  │
                                   │  1. 컨텍스트 읽기  │
                                   │  2. 가설 수립     │
                                   │  3. 한 곳만 수정   │
                                   │  4. Git 커밋      │
                                   │  5. 검증 실행     │
                                   │  6. 개선됨?       │
                                   │     → 유지        │
                                   │     → 롤백        │
                                   │  7. 결과 기록     │
                                   │  8. 다음 턴       │
                                   └─────────────────┘
```

모든 개선은 누적됩니다. 모든 실패는 자동으로 되돌려집니다. 진행 상황은 TSV 형식으로 기록됩니다. 에스컬레이션 사다리(정제 → 전환 → 웹 검색 → 중지)가 무한 재시도를 방지합니다.

---

## 명령어

| 명령어 | 기능 | 기본 반복 횟수 |
|--------|------|--------------|
| `/autoresearch` | 핵심 반복 루프: 수정 → 검증 → 유지/폐기 | 25 |
| `/autoresearch:plan` | 대화형 마법사 → 검증된 설정 | 1회 |
| `/autoresearch:debug` | 가설 반복을 통한 버그 추적 | 15 |
| `/autoresearch:fix` | 오류를 하나씩 제로까지 수정 | 20 |
| `/autoresearch:security` | STRIDE + OWASP 보안 감사 | 15 |
| `/autoresearch:ship` | 8단계 배포 워크플로우 | 선형 |
| `/autoresearch:scenario` | 12개 차원에서 엣지 케이스 생성 | 20 |
| `/autoresearch:predict` | 5명의 전문가 페르소나 토론 | 1회 |
| `/autoresearch:learn` | 탐색 → 문서 생성 → 검증 → 수정 | 10 |
| `/autoresearch:reason` | 블라인드 심사가 있는 적대적 토론 | 8 |
| `/autoresearch:probe` | 8개 페르소나가 요구사항 심문 | 15 |
| `/autoresearch:improve` | 제품 개선 리서치 | 20 |
| `/autoresearch:evals` | 반복 결과 분석: 추세와 정체기 | 1회 |

---

## 빠른 시작

### Claude Code (플러그인 설치)

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

세션을 재시작하세요. 13개 명령어가 모두 사용 가능합니다.

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

사용법: `$autoresearch`

### 소스에서 빌드

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Rust 툴체인이 필요합니다([rustup.rs](https://rustup.rs)). 런타임 의존성 없는 약 2.5MB 바이너리가 생성됩니다.

---

## 핵심 규칙

1. **턴당 하나의 변경** — 원자적 실험으로 인과 관계를 확립
2. **쓰기 전에 읽기** — 수정 전 git log와 결과 TSV 확인
3. **기계적 검증만** — 명령 실행, 숫자 파싱
4. **자동 롤백** — 실패 시 `git revert HEAD --no-edit`
5. **단순함이 이긴다** — 동일한 메트릭 + 더 적은 코드 = 유지

---

[전체 문서 (English)](../../README.md)
