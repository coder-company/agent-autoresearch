<div align="center">

# autoresearch

**Autonome zielgerichtete Iterations-Engine für Coding-Agenten. In Rust geschrieben.**

*„Ziel festlegen → Agent führt die Schleife aus → Du wachst mit Ergebnissen auf"*

[English](../../README.md) · [中文](README_ZH.md) · [日本語](README_JA.md) · [한국어](README_KO.md) · [Français](README_FR.md) · **Deutsch** · [Español](README_ES.md) · [Português](README_PT.md) · [Русский](README_RU.md)

</div>

---

## Funktionsweise

```
Du beschreibst das Ziel  →  Agent bestätigt Konfiguration  →  Du sagst "los"
                                                                │
                                                       ┌────────┴────────┐
                                                       │  Schleife aktiv  │
                                                       │                  │
                                                       │  1. Kontext lesen│
                                                       │  2. Hypothese    │
                                                       │  3. EINE Änderung│
                                                       │  4. Git Commit   │
                                                       │  5. Verifizieren │
                                                       │  6. Verbessert?  │
                                                       │     → behalten   │
                                                       │     → rückgängig │
                                                       │  7. Protokoll    │
                                                       │  8. Nächste Runde│
                                                       └─────────────────┘
```

Jede Verbesserung addiert sich. Jeder Fehlschlag wird automatisch zurückgesetzt. Der Fortschritt wird im TSV-Format protokolliert. Die Eskalationsleiter (Verfeinern → Schwenken → Websuche → Stopp) verhindert endlose Wiederholungen.

---

## Befehle

| Befehl | Funktion | Standard-Iterationen |
|--------|----------|---------------------|
| `/autoresearch` | Kern-Schleife: ändern → verifizieren → behalten/verwerfen | 25 |
| `/autoresearch:plan` | Interaktiver Assistent → validierte Konfiguration | einmalig |
| `/autoresearch:debug` | Bug-Jagd durch Hypothesen-Iteration | 15 |
| `/autoresearch:fix` | Fehler einzeln bis auf null korrigieren | 20 |
| `/autoresearch:security` | STRIDE + OWASP Sicherheitsaudit | 15 |
| `/autoresearch:ship` | 8-Phasen-Release-Workflow | linear |
| `/autoresearch:scenario` | Grenzfälle über 12 Dimensionen generieren | 20 |
| `/autoresearch:predict` | Debatte zwischen 5 Experten-Personas | einmalig |
| `/autoresearch:learn` | Erkunden → Doku generieren → validieren → korrigieren | 10 |
| `/autoresearch:reason` | Kontradiktorische Debatte mit Blind-Richtern | 8 |
| `/autoresearch:probe` | 8 Personas hinterfragen Anforderungen | 15 |
| `/autoresearch:improve` | Recherche zu Produktverbesserungen | 20 |
| `/autoresearch:evals` | Ergebnisanalyse: Trends und Plateaus | einmalig |

---

## Schnellstart

### Claude Code (Plugin-Installation)

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

Session neu starten. Alle 12 Befehle sind sofort verfügbar.

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Dann: `$autoresearch`

### Aus dem Quellcode

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Benötigt die Rust-Toolchain ([rustup.rs](https://rustup.rs)). Erzeugt eine ca. 2,5 MB große Binärdatei ohne Laufzeitabhängigkeiten.

---

## Wichtigste Regeln

1. **Eine Änderung pro Runde** — atomare Experimente schaffen Kausalität
2. **Erst lesen, dann schreiben** — git log und TSV vor der Änderung prüfen
3. **Nur mechanische Verifikation** — Befehl ausführen, Zahl auswerten
4. **Automatischer Rollback** — `git revert HEAD --no-edit` bei Fehlschlag
5. **Einfachheit gewinnt** — gleiche Metrik + weniger Code = behalten

---

[Vollständige Dokumentation (English)](../../README.md)
