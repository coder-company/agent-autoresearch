<div align="center">

# autoresearch

**面向编码代理的自主目标驱动迭代引擎。Rust 编写。**

*"设定目标 → 代理运行循环 → 你醒来就有结果"*

[English](../../README.md) · **中文** · [日本語](README_JA.md) · [한국어](README_KO.md) · [Français](README_FR.md) · [Deutsch](README_DE.md) · [Español](README_ES.md) · [Português](README_PT.md) · [Русский](README_RU.md)

</div>

---

## 工作原理

```
你描述目标  →  代理确认配置  →  你说"开始"
                                    │
                           ┌────────┴────────┐
                           │    循环运行中     │
                           │                  │
                           │  1. 读取上下文    │
                           │  2. 提出假设      │
                           │  3. 修改一处      │
                           │  4. Git 提交      │
                           │  5. 运行验证      │
                           │  6. 有改善？      │
                           │     → 保留        │
                           │     → 回滚        │
                           │  7. 记录结果      │
                           │  8. 下一轮        │
                           └─────────────────┘
```

每次改善都会累积。每次失败都会自动回滚。进度以 TSV 格式记录。升级策略（细化 → 转向 → 网络搜索 → 停止）防止无限暴力重试。

---

## 命令

| 命令 | 功能 | 默认迭代次数 |
|------|------|-------------|
| `/autoresearch` | 核心迭代循环：修改 → 验证 → 保留/丢弃 | 25 |
| `/autoresearch:plan` | 交互式向导 → 验证后的配置 | 一次性 |
| `/autoresearch:debug` | 通过假设迭代追踪缺陷 | 15 |
| `/autoresearch:fix` | 逐一修复错误直至归零 | 20 |
| `/autoresearch:security` | STRIDE + OWASP 安全审计 | 15 |
| `/autoresearch:ship` | 8 阶段发布流程 | 线性 |
| `/autoresearch:scenario` | 跨 12 个维度生成边界用例 | 20 |
| `/autoresearch:predict` | 5 位专家角色辩论 | 一次性 |
| `/autoresearch:learn` | 侦察 → 生成文档 → 验证 → 修复 | 10 |
| `/autoresearch:reason` | 对抗性辩论与盲审评判 | 8 |
| `/autoresearch:probe` | 8 个角色审问需求 | 15 |
| `/autoresearch:improve` | 产品改进研究 | 20 |
| `/autoresearch:evals` | 分析迭代结果：趋势与瓶颈 | 一次性 |

---

## 快速开始

### Claude Code（插件安装）

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

重启会话。全部 13 个命令立即可用。

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

然后使用：`$autoresearch`

### OpenCode

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

使用：`/autoresearch` 或 `/autoresearch_debug`

### 从源码构建

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

需要 Rust 工具链（[rustup.rs](https://rustup.rs)）。生成约 2.5MB 的零依赖二进制文件。

---

## 核心规则

1. **每轮只改一处** — 原子实验才能建立因果关系
2. **先读再写** — 修改前先查看 git log 和结果 TSV
3. **机械验证** — 运行命令，解析数字
4. **自动回滚** — 失败时执行 `git revert HEAD --no-edit`
5. **简洁为王** — 指标相同 + 代码更少 = 保留

---

[完整文档（English）](../../README.md)
