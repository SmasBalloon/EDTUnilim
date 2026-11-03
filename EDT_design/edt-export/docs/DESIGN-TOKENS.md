# Design tokens

Source unique : `css/tokens.css`. Pour changer une couleur du site, modifie la variable ici et nulle part ailleurs.

## Couleurs de base (thème sombre)

| Variable | Valeur | Usage |
|---|---|---|
| `--edt-bg` | `#0E0F13` | Fond de page |
| `--edt-surface-1` | `#15171D` | Cartes, panneaux |
| `--edt-surface-2` | `#1C1F27` | Boutons, entrée de menu active |
| `--edt-surface-3` | `#262A35` | Segment actif, tags neutres |
| `--edt-line` | `#2A2E39` | Bordures, séparateurs, lignes d'heures |
| `--edt-text` | `#F3F4F7` | Texte principal |
| `--edt-muted` | `#A0A7B8` | Texte secondaire, libellés |

## Accent

| Variable | Valeur | Usage |
|---|---|---|
| `--edt-accent` | `#FF5252` | Bouton principal, icône active, logo |
| `--edt-on-accent` | `#1A0708` | Texte sur l'accent |
| `--edt-accent-soft` | 14 % de l'accent | Option sélectionnée, tag salle |
| `--edt-accent-text` | `#FF9A9A` | Texte accent sur fond sombre |
| `--edt-accent-hot` | `#FF8A8A` | « Aujourd'hui », prochain cours |
| `--edt-accent-ring` | `rgba(255,82,82,.75)` | Contour « cours suivis ensemble » |

L'accent d'origine du site était `#ff4b4b` (`theme-color`) ; la maquette utilise `#FF5252`, légèrement plus clair pour rester lisible sur fond sombre.

## Types de cours

| Type | Texte | Fond carte | Bordure carte | Fond tag |
|---|---|---|---|---|
| SAÉ | `#6EE7A0` | `rgba(74,222,128,.12)` | `rgba(74,222,128,.35)` | `rgba(74,222,128,.14)` |
| CM | `#FBC94A` | `rgba(251,191,36,.14)` | `rgba(251,191,36,.45)` | `rgba(251,191,36,.16)` |
| TD | `#FFA366` | `rgba(255,140,66,.15)` | `rgba(255,140,66,.48)` | `rgba(255,140,66,.16)` |
| TP | `#8CBBFF` | `rgba(96,165,250,.16)` | `rgba(96,165,250,.48)` | `rgba(96,165,250,.18)` |

Variables : `--edt-sae-text`, `--edt-sae-bg`, `--edt-sae-line`, `--edt-sae-tag` (idem `cm`, `td`, `tp`).

## États et marques

| Variable | Valeur | Usage |
|---|---|---|
| `--edt-ok` | `#6EE7A0` | Salle libre, « libres tous les deux » |
| `--edt-ok-soft` / `--edt-ok-line` | vert 13 % / 35 % | Fond et bordure des salles libres |
| `--edt-discord` | `#4752C4` | Bouton Discord (blanc dessus : 6,4:1) |
| `--edt-scrim` | `rgba(6,7,10,.7)` | Fond derrière la modale |
| `--edt-y1-*`, `--edt-y2-*`, `--edt-y3-*` | bleu, ambre, violet | Années BUT 1 / 2 / 3 quand il faut les distinguer |

## Contrastes (thème sombre)

Calculés sur les valeurs ci-dessus : `--edt-text` sur `--edt-bg` 17:1 ; `--edt-muted` sur `--edt-bg` 8:1 (7,4:1 sur une carte) ; libellés de type sur leur carte colorée entre 7:1 (TP, TD) et 9:1 (SAÉ) ; texte secondaire sur carte colorée au moins 5,5:1 ; `--edt-on-accent` sur `--edt-accent` 6,1:1. Tout est au-dessus du seuil de 4,5:1. Thème clair : texte 16:1, secondaire 5,6:1, textes de type 7:1 ou plus, blanc sur accent 4,7:1.

## Typographie

| Variable | Police | Usage |
|---|---|---|
| `--edt-font-display` | Bricolage Grotesque (500–700) | Titres, codes de cours, dates |
| `--edt-font-body` | Instrument Sans (400–700) | Texte, boutons, formulaires |
| `--edt-font-mono` | Police monospace système | Réservée (non utilisée dans la Proposition 1) |

Tailles courantes : titre de page 36 px ; code de cours dans une carte 15–16 px ; texte courant 15 px ; texte secondaire 12–13 px ; libellés de section 12 px en capitales (`.lbl`, espacement 0,08 em). Rayons : cartes 18 px, boutons 12 px, cartes de cours 11–13 px, options et pastilles 999 px. Zones tactiles : 44 px minimum.

## Thème clair (point de départ)

Défini dans `tokens.css` sous `:root[data-theme="light"]` : fond `#F6F5F2`, cartes `#FFFFFF`, texte `#17181C`, accent `#D9303A`, textes de type assombris (SAÉ `#11592F`, CM `#704800`, TD `#8A360A`, TP `#1A4594`). Non maquetté : à valider visuellement avant mise en ligne.

## Variantes

Les palettes des Propositions 2 (clair chaud) et 3 (style Apple) sont dans `css/tokens-alternatives.css`, à titre de référence.
