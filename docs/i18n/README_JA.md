<div align="center">

# autoresearch

**コーディングエージェント向け自律型目標駆動イテレーションエンジン。Rust 製。**

*「目標を設定 → エージェントがループを実行 → 目覚めたら結果が出ている」*

[English](../../README.md) · [中文](README_ZH.md) · **日本語** · [한국어](README_KO.md) · [Français](README_FR.md) · [Deutsch](README_DE.md) · [Español](README_ES.md) · [Português](README_PT.md) · [Русский](README_RU.md)

</div>

---

## 仕組み

```
目標を記述  →  エージェントが設定を確認  →  「開始」と伝える
                                              │
                                     ┌────────┴────────┐
                                     │  ループ実行中     │
                                     │                  │
                                     │  1. コンテキスト読取 │
                                     │  2. 仮説を立てる    │
                                     │  3. 1箇所を変更    │
                                     │  4. Git コミット   │
                                     │  5. 検証を実行     │
                                     │  6. 改善した？     │
                                     │     → 保持        │
                                     │     → 元に戻す    │
                                     │  7. 結果を記録     │
                                     │  8. 次のターン     │
                                     └─────────────────┘
```

改善は積み重なり、失敗は自動的にリバートされます。進捗は TSV 形式で記録されます。エスカレーション（改良 → 方針転換 → Web 検索 → 停止）により無限リトライを防止します。

---

## コマンド

| コマンド | 機能 | デフォルト反復回数 |
|---------|------|------------------|
| `/autoresearch` | コアループ：変更 → 検証 → 保持/破棄 | 25 |
| `/autoresearch:plan` | 対話型ウィザード → 検証済み設定 | 1回 |
| `/autoresearch:debug` | 仮説ベースのバグ追跡 | 15 |
| `/autoresearch:fix` | エラーをゼロになるまで1つずつ修正 | 20 |
| `/autoresearch:security` | STRIDE + OWASP セキュリティ監査 | 15 |
| `/autoresearch:ship` | 8フェーズのリリースフロー | 線形 |
| `/autoresearch:scenario` | 12次元のエッジケース生成 | 20 |
| `/autoresearch:predict` | 5人の専門家ペルソナによる議論 | 1回 |
| `/autoresearch:learn` | 偵察 → ドキュメント生成 → 検証 → 修正 | 10 |
| `/autoresearch:reason` | ブラインド審査付き対立的議論 | 8 |
| `/autoresearch:probe` | 8つのペルソナが要件を徹底質問 | 15 |
| `/autoresearch:improve` | プロダクト改善リサーチ | 20 |
| `/autoresearch:evals` | 反復結果の分析：傾向とプラトー | 1回 |

---

## クイックスタート

### Claude Code（プラグインインストール）

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

セッションを再起動。13個すべてのコマンドが利用可能になります。

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

使い方：`$autoresearch`

### OpenCode

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

使用：`/autoresearch` または `/autoresearch_debug`

### ソースからビルド

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Rust ツールチェーンが必要です（[rustup.rs](https://rustup.rs)）。ランタイム依存ゼロの約 2.5MB バイナリが生成されます。

---

## 重要ルール

1. **1ターン1変更** — 原子的な実験で因果関係を確立
2. **書く前に読む** — 変更前に git log と結果 TSV を確認
3. **機械的検証のみ** — コマンド実行、数値パース
4. **自動ロールバック** — 失敗時は `git revert HEAD --no-edit`
5. **シンプルさが勝つ** — 同じメトリクス + コード削減 = 保持

---

[完全なドキュメント（English）](../../README.md)
