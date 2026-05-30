<div align="center">

# autoresearch

**Автономный целенаправленный итерационный движок для кодинг-агентов. Написан на Rust.**

*«Задай ЦЕЛЬ → Агент крутит ЦИКЛ → Просыпаешься с результатами»*

[English](../../README.md) · [中文](README_ZH.md) · [日本語](README_JA.md) · [한국어](README_KO.md) · [Français](README_FR.md) · [Deutsch](README_DE.md) · [Español](README_ES.md) · [Português](README_PT.md) · **Русский**

</div>

---

## Как это работает

```
Описываешь цель  →  Агент подтверждает конфигурацию  →  Говоришь "поехали"
                                                          │
                                                 ┌────────┴────────┐
                                                 │   Цикл активен   │
                                                 │                  │
                                                 │  1. Читать контекст│
                                                 │  2. Гипотеза      │
                                                 │  3. Изменить ОДНО │
                                                 │  4. Git коммит    │
                                                 │  5. Проверить     │
                                                 │  6. Улучшилось?   │
                                                 │     → оставить    │
                                                 │     → откатить    │
                                                 │  7. Записать      │
                                                 │  8. Следующий ход │
                                                 └─────────────────┘
```

Каждое улучшение накапливается. Каждая неудача автоматически откатывается. Прогресс записывается в формате TSV. Лестница эскалации (Уточнить → Сменить подход → Веб-поиск → Стоп) предотвращает бесконечные повторы.

---

## Команды

| Команда | Функция | Итераций по умолчанию |
|---------|---------|----------------------|
| `/autoresearch` | Основной цикл: изменить → проверить → оставить/отбросить | 25 |
| `/autoresearch:plan` | Интерактивный мастер → валидированная конфигурация | разово |
| `/autoresearch:debug` | Поиск багов через итерацию гипотез | 15 |
| `/autoresearch:fix` | Исправление ошибок по одной до нуля | 20 |
| `/autoresearch:security` | Аудит STRIDE + OWASP с red-team | 15 |
| `/autoresearch:ship` | 8-фазный процесс выпуска | линейно |
| `/autoresearch:scenario` | Генерация граничных случаев по 12 измерениям | 20 |
| `/autoresearch:predict` | Дебаты 5 экспертных персон | разово |
| `/autoresearch:learn` | Разведка → генерация документации → валидация → исправление | 10 |
| `/autoresearch:reason` | Состязательные дебаты со слепыми судьями | 8 |
| `/autoresearch:probe` | 8 персон допрашивают требования | 15 |
| `/autoresearch:improve` | Исследование улучшений продукта | 20 |
| `/autoresearch:evals` | Анализ результатов: тренды и плато | разово |

---

## Быстрый старт

### Claude Code (установка плагина)

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

Перезапустите сессию. Все 13 команд доступны.

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Затем: `$autoresearch`

### OpenCode

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --opencode
```

Используйте: `/autoresearch` или `/autoresearch_debug`.

### Сборка из исходников

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Требуется Rust toolchain ([rustup.rs](https://rustup.rs)). На выходе — бинарник ~3 МБ без runtime-зависимостей.

---

## Ключевые правила

1. **Одно изменение за ход** — атомарные эксперименты устанавливают причинность
2. **Читай перед записью** — проверь git log и TSV перед изменением
3. **Только механическая верификация** — выполнить команду, извлечь число
4. **Автоматический откат** — `git revert HEAD --no-edit` при неудаче
5. **Простота побеждает** — та же метрика + меньше кода = оставить

---

[Полная документация (English)](../../README.md)
