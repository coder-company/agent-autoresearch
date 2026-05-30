<div align="center">

# autoresearch

**Moteur d'itération autonome dirigé par objectifs pour agents de programmation. Écrit en Rust.**

*« Définir l'OBJECTIF → L'agent exécute la BOUCLE → Vous vous réveillez avec des résultats »*

[English](../../README.md) · [中文](README_ZH.md) · [日本語](README_JA.md) · [한국어](README_KO.md) · **Français** · [Deutsch](README_DE.md) · [Español](README_ES.md) · [Português](README_PT.md) · [Русский](README_RU.md)

</div>

---

## Fonctionnement

```
Vous décrivez l'objectif  →  L'agent confirme la config  →  Vous dites "go"
                                                              │
                                                     ┌────────┴────────┐
                                                     │  Boucle active   │
                                                     │                  │
                                                     │  1. Lire contexte│
                                                     │  2. Hypothèse    │
                                                     │  3. Modifier UN  │
                                                     │  4. Git commit   │
                                                     │  5. Vérifier     │
                                                     │  6. Amélioré ?   │
                                                     │     → garder     │
                                                     │     → annuler    │
                                                     │  7. Journaliser  │
                                                     │  8. Tour suivant │
                                                     └─────────────────┘
```

Chaque amélioration s'empile. Chaque échec est automatiquement annulé. La progression est enregistrée au format TSV. L'échelle d'escalade (Affiner → Pivoter → Recherche web → Arrêt) empêche les tentatives infinies.

---

## Commandes

| Commande | Fonction | Itérations par défaut |
|----------|----------|----------------------|
| `/autoresearch` | Boucle principale : modifier → vérifier → garder/rejeter | 25 |
| `/autoresearch:plan` | Assistant interactif → configuration validée | unique |
| `/autoresearch:debug` | Chasse aux bugs par itération d'hypothèses | 15 |
| `/autoresearch:fix` | Corriger les erreurs une par une jusqu'à zéro | 20 |
| `/autoresearch:security` | Audit STRIDE + OWASP avec red-team | 15 |
| `/autoresearch:ship` | Flux de livraison en 8 phases | linéaire |
| `/autoresearch:scenario` | Générer des cas limites sur 12 dimensions | 20 |
| `/autoresearch:predict` | Débat entre 5 experts | unique |
| `/autoresearch:learn` | Explorer → générer docs → valider → corriger | 10 |
| `/autoresearch:reason` | Débat contradictoire avec juges aveugles | 8 |
| `/autoresearch:probe` | 8 personas interrogent les exigences | 15 |
| `/autoresearch:improve` | Recherche d'améliorations produit | 20 |
| `/autoresearch:evals` | Analyse des résultats : tendances et plateaux | unique |

---

## Démarrage rapide

### Claude Code (installation plugin)

```
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh --yes --claude
```

Redémarrez votre session. Les 13 commandes sont disponibles.

### Codex CLI

```
$skill-installer install https://github.com/coder-company/agent-autoresearch
```

Puis : `$autoresearch`

### Depuis les sources

```bash
git clone https://github.com/coder-company/agent-autoresearch.git
cd agent-autoresearch
./install.sh
```

Nécessite la chaîne d'outils Rust ([rustup.rs](https://rustup.rs)). Produit un binaire d'environ 2,5 Mo sans aucune dépendance d'exécution.

---

## Règles essentielles

1. **Un seul changement par tour** — les expériences atomiques établissent la causalité
2. **Lire avant d'écrire** — consulter git log et le TSV avant de modifier
3. **Vérification mécanique uniquement** — exécuter la commande, extraire le nombre
4. **Rollback automatique** — `git revert HEAD --no-edit` en cas d'échec
5. **La simplicité l'emporte** — métrique identique + moins de code = garder

---

[Documentation complète (English)](../../README.md)
