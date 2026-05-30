<div align="center">

# autoresearch

**Motor de iteración autónoma dirigido por objetivos para agentes de programación. Escrito en Rust.**

*«Define el OBJETIVO → El agente ejecuta el BUCLE → Despiertas con resultados»*

[English](../../README.md) · [中文](README_ZH.md) · [日本語](README_JA.md) · [한국어](README_KO.md) · [Français](README_FR.md) · [Deutsch](README_DE.md) · **Español** · [Português](README_PT.md) · [Русский](README_RU.md)

</div>

---

## Cómo funciona

```
Describes el objetivo  →  El agente confirma la config  →  Dices "adelante"
                                                             │
                                                    ┌────────┴────────┐
                                                    │   Bucle activo   │
                                                    │                  │
                                                    │  1. Leer contexto│
                                                    │  2. Hipótesis    │
                                                    │  3. Modificar UNO│
                                                    │  4. Git commit   │
                                                    │  5. Verificar    │
                                                    │  6. ¿Mejoró?    │
                                                    │     → conservar  │
                                                    │     → revertir   │
                                                    │  7. Registrar    │
                                                    │  8. Siguiente    │
                                                    └─────────────────┘
```

Cada mejora se acumula. Cada fallo se revierte automáticamente. El progreso se registra en formato TSV. La escalera de escalamiento (Refinar → Pivotar → Búsqueda web → Detener) previene reintentos infinitos.

---

## Comandos

| Comando | Función | Iteraciones por defecto |
|---------|---------|------------------------|
| `/autoresearch` | Bucle principal: modificar → verificar → conservar/descartar | 25 |
| `/autoresearch:plan` | Asistente interactivo → configuración validada | única |
| `/autoresearch:debug` | Caza de bugs mediante iteración de hipótesis | 15 |
| `/autoresearch:fix` | Corregir errores uno a uno hasta llegar a cero | 20 |
| `/autoresearch:security` | Auditoría STRIDE + OWASP con red-team | 15 |
| `/autoresearch:ship` | Flujo de lanzamiento en 8 fases | lineal |
| `/autoresearch:scenario` | Generar casos límite en 12 dimensiones | 20 |
| `/autoresearch:predict` | Debate entre 5 expertos | única |
| `/autoresearch:learn` | Explorar → generar docs → validar → corregir | 10 |
| `/autoresearch:reason` | Debate adversarial con jueces ciegos | 8 |
| `/autoresearch:probe` | 8 personas interrogan los requisitos | 15 |
| `/autoresearch:improve` | Investigación de mejoras de producto | 20 |
| `/autoresearch:evals` | Análisis de resultados: tendencias y mesetas | única |

---

## Inicio rápido

### Claude Code (instalación de plugin)

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

Reinicia tu sesión. Los 13 comandos están disponibles.

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Luego: `$autoresearch`

### OpenCode

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

Usa: `/autoresearch` o `/autoresearch_debug`.

### Desde el código fuente

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Requiere la cadena de herramientas de Rust ([rustup.rs](https://rustup.rs)). Genera un binario de ~3 MB sin dependencias en tiempo de ejecución.

---

## Reglas fundamentales

1. **Un solo cambio por turno** — los experimentos atómicos establecen causalidad
2. **Leer antes de escribir** — revisar git log y TSV antes de modificar
3. **Solo verificación mecánica** — ejecutar el comando, extraer el número
4. **Rollback automático** — `git revert HEAD --no-edit` ante fallos
5. **La simplicidad gana** — misma métrica + menos código = conservar

---

[Documentación completa (English)](../../README.md)
