<div align="center">

# autoresearch

**Motor de iteração autônoma orientado a objetivos para agentes de programação. Escrito em Rust.**

*"Defina o OBJETIVO → O agente executa o LOOP → Você acorda com resultados"*

[English](../../README.md) · [中文](README_ZH.md) · [日本語](README_JA.md) · [한국어](README_KO.md) · [Français](README_FR.md) · [Deutsch](README_DE.md) · [Español](README_ES.md) · **Português** · [Русский](README_RU.md)

</div>

---

## Como funciona

```
Você descreve o objetivo  →  Agente confirma a config  →  Você diz "vai"
                                                            │
                                                   ┌────────┴────────┐
                                                   │   Loop ativo     │
                                                   │                  │
                                                   │  1. Ler contexto │
                                                   │  2. Hipótese     │
                                                   │  3. Modificar UM │
                                                   │  4. Git commit   │
                                                   │  5. Verificar    │
                                                   │  6. Melhorou?    │
                                                   │     → manter     │
                                                   │     → reverter   │
                                                   │  7. Registrar    │
                                                   │  8. Próximo turno│
                                                   └─────────────────┘
```

Cada melhoria se acumula. Cada falha é revertida automaticamente. O progresso é registrado em formato TSV. A escada de escalação (Refinar → Pivotar → Busca web → Parar) impede tentativas infinitas.

---

## Comandos

| Comando | Função | Iterações padrão |
|---------|--------|-----------------|
| `/autoresearch` | Loop principal: modificar → verificar → manter/descartar | 25 |
| `/autoresearch:plan` | Assistente interativo → configuração validada | única |
| `/autoresearch:debug` | Caça a bugs por iteração de hipóteses | 15 |
| `/autoresearch:fix` | Corrigir erros um a um até zerar | 20 |
| `/autoresearch:security` | Auditoria STRIDE + OWASP com red-team | 15 |
| `/autoresearch:ship` | Fluxo de lançamento em 8 fases | linear |
| `/autoresearch:scenario` | Gerar casos-limite em 12 dimensões | 20 |
| `/autoresearch:predict` | Debate entre 5 especialistas | única |
| `/autoresearch:learn` | Explorar → gerar docs → validar → corrigir | 10 |
| `/autoresearch:reason` | Debate adversarial com juízes cegos | 8 |
| `/autoresearch:probe` | 8 personas interrogam requisitos | 15 |
| `/autoresearch:improve` | Pesquisa de melhorias de produto | 20 |
| `/autoresearch:evals` | Análise de resultados: tendências e platôs | única |

---

## Início rápido

### Claude Code (instalação via plugin)

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

Reinicie sua sessão. Todos os 13 comandos ficam disponíveis.

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Depois: `$autoresearch`

### OpenCode

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

Use: `/autoresearch` ou `/autoresearch_debug`.

### A partir do código-fonte

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Requer a toolchain Rust ([rustup.rs](https://rustup.rs)). Gera um binário de ~3 MB sem dependências de execução.

---

## Regras fundamentais

1. **Uma mudança por turno** — experimentos atômicos estabelecem causalidade
2. **Ler antes de escrever** — checar git log e TSV antes de modificar
3. **Apenas verificação mecânica** — executar o comando, extrair o número
4. **Rollback automático** — `git revert HEAD --no-edit` em caso de falha
5. **Simplicidade vence** — mesma métrica + menos código = manter

---

[Documentação completa (English)](../../README.md)
