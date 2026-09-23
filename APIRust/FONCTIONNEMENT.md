# Fonctionnement interne

Ce document décrit comment `edt-unilim-api` transforme les PDF publiés par le
département informatique de l'IUT du Limousin en JSON. Le [README](README.md)
décrit l'usage ; celui-ci décrit la mécanique.

## 1. Vue d'ensemble

Le département publie un PDF par année (`A1`, `A2`, `A3`) et par semaine, à
l'adresse `https://edt-iut-info.unilim.fr/edt/A{année}/A{année}_S{semaine}.pdf`.
Ces PDF sont des images vectorielles : aucune table, aucun balisage, seulement
des rectangles de couleur et des fragments de texte posés par-dessus. Tout le
travail du service consiste à reconstruire la sémantique à partir de la
géométrie.

```
requête HTTP                 http/          axum : routage, filtres, JSON
      │
      ▼
   Store            store.rs        mémoire → disque → réseau
      │                                 │
      │                                 ▼
      │                            upstream.rs      GET conditionnel (ETag)
      ▼
   pdf::parse       pdf/content.rs   primitives (rectangles + textes)
                    pdf/grid.rs      repère (échelle horaire, jours, groupes)
                    pdf/timetable.rs créneaux (Course)
      │
      ▼
   Timetable        model.rs         modèle métier sérialisé en JSON
```

Découpage des modules :

| Module | Rôle |
| --- | --- |
| `main.rs` | démarrage, journalisation, tâche de rafraîchissement, arrêt gracieux |
| `config.rs` | configuration lue dans l'environnement (`EDT_*`) |
| `http/` | routeur axum, extracteurs, handlers, filtres |
| `store.rs` | cache mémoire + cache disque, orchestration du téléchargement |
| `upstream.rs` | client HTTP vers le serveur du département |
| `pdf/` | analyse du PDF (le cœur du projet) |
| `model.rs` | types exposés dans le JSON |
| `catalog.rs` | référentiel des ressources/SAÉ, embarqué dans le binaire |
| `error.rs` | erreurs de l'API et leur rendu JSON |

## 2. Cycle de vie d'une requête

`GET /api/v1/years/1/weeks/3?group=G1a&day=lundi` :

1. **Routage** (`http/mod.rs`) — axum résout la route ; les couches `tower-http`
   ajoutent la trace, la compression gzip, le CORS permissif et un délai maximal
   de 30 s. `PathParams` remplace les rejets d'axum par le format d'erreur JSON
   de l'API.
2. **Store** (`store.rs`) — `timetable(year, week)` rend l'entrée en cache si
   elle existe ; sinon elle est téléchargée puis analysée.
3. **Filtrage** (`http/timetable.rs`) — les créneaux sont filtrés par jour
   (insensible à la casse) et par groupe.
4. **Réponse** — `WeekResponse` sérialise l'année, la semaine, la période, la
   source (URL amont + date de récupération), le nombre de créneaux et les
   créneaux eux-mêmes, avec `Cache-Control: public, max-age=300`.

Le filtre de groupe (`matches_group`) compare les deux libellés sur leur préfixe
commun : `G1` correspond donc à `G1a` et `G1b`, `G1a` correspond aussi aux cours
du groupe entier `G1`, mais `G1a` ne correspond pas à `G1b`. C'est ce qui permet
à un étudiant de demander son demi-groupe et de recevoir sa journée complète.

L'abonnement `GET /api/v1/years/{year}/calendar/{group}.ics` suit le même
chemin pour chaque semaine publiée (`store.weeks` puis `store.timetable`), et
confie le tout à `ics::subscription`, qui partage avec l'export joint aux
messages privés (`ics::build`) l'écriture des créneaux. Les `UID` sont stables
d'une version à l'autre : l'agenda qui relit le calendrier met à jour un cours
déplacé au lieu de le dupliquer. Une semaine en erreur est journalisée et omise.

## 3. Récupération et caches

Trois niveaux, du plus rapide au plus lent (`store.rs`) :

1. **Mémoire** — `HashMap<(année, semaine), Arc<Entry>>` sous `RwLock`. Une
   entrée contient l'emploi du temps analysé, la date de récupération et les
   validateurs HTTP.
2. **Disque** — `cache/A{année}/A{année}_S{semaine}.pdf`. Servi en repli quand
   le serveur amont est injoignable ou incohérent : le service continue de
   répondre pendant une panne du département.
3. **Réseau** — `upstream.rs`.

La liste des semaines publiées est obtenue en lisant l'index Apache du
répertoire `A{année}/` et en y relevant les liens `A{année}_S{n}.pdf`
(`parse_week_links`).

Une tâche de fond (`refresh_loop` dans `main.rs`) s'exécute toutes les
`EDT_REFRESH_SECONDS` (900 s par défaut) : elle rafraîchit l'index des semaines,
puis revalide chaque emploi du temps déjà en cache par requête conditionnelle
(`If-None-Match` / `If-Modified-Since`). Un `304 Not Modified` ne coûte aucun
transfert ; un `200` déclenche l'écriture disque et une réanalyse. C'est
nécessaire parce que le département republie un PDF dès qu'un créneau bouge, à
la même URL.

Cas particuliers gérés au premier téléchargement :

- `404` → `unknown_week` (404 côté API) ;
- panne réseau ou timeout → repli sur le PDF en cache disque, sinon
  `upstream_unavailable` (502) ;
- `304` alors qu'aucun validateur n'a été envoyé → réponse incohérente, repli
  disque, sinon `upstream_unexpected` (502).

## 4. Lecture du PDF

C'est la partie non triviale. `pdf::parse` enchaîne trois étapes.

### 4.1 Primitives (`pdf/content.rs`)

Un interpréteur de flux de contenu PDF parcourt les opérateurs de la première
page et n'en retient que deux choses :

- les **aplats de couleur** (`FilledArea`) : opérateurs de tracé (`m`, `l`, `c`,
  `re`, …) suivis d'un opérateur de remplissage (`f`, `B`, …). Seule la boîte
  englobante du tracé est conservée ;
- les **fragments de texte** (`TextFragment`) : opérateurs `Tj`, `TJ`, `'`, `"`,
  avec leur point d'ancrage.

L'interpréteur suit l'état graphique (`q`/`Q`, `cm`), les espaces de couleur
(`rg`, `g`, `k`, `sc`, `scn`, avec conversion CMJN → RVB) et la matrice de texte
(`Tm`, `Td`, `TD`, `T*`, `TL`, `Tf`, `Tc`). Les XObjects de type `Form` (`Do`)
sont visités récursivement, avec une profondeur maximale de 8 et un ensemble des
objets en cours de visite pour couper les références circulaires.

Deux détails comptent :

- **Encodage** (`pdf/encoding.rs`) — les polices sont des Type1 standard en
  `WinAnsiEncoding` ; un décodage Latin-1 corrigé de la plage `0x80..=0x9F`
  (Windows-1252) suffit pour obtenir `Monédière T.` ou `l'année`.
- **Largeur du texte** (`pdf/metrics.rs`) — un PDF ne stocke que le point de
  départ d'une chaîne. Or les étiquettes horaires sont centrées sur leur
  graduation : sans leur largeur, la graduation est décalée d'une demi-étiquette,
  soit environ cinq minutes. On mesure donc les chiffres, le `:` et l'espace en
  métriques Helvetica-Bold, et on renonce à mesurer le reste (`None`) plutôt que
  d'inventer des valeurs.

Les fragments successifs sans repositionnement sont accumulés dans le même mot ;
un `Tm`, un saut de ligne ou un `ET` clôt le fragment.

### 4.2 Grille (`pdf/grid.rs`)

Aucune constante de mise en page n'est codée en dur : le générateur change
d'échelle d'une semaine à l'autre, et selon les semaines le document est produit
en paysage puis pivoté.

**Canonicalisation** — les étiquettes horaires servent de boussole. Si elles sont
plus dispersées en `y` qu'en `x`, la page est transposée ; si l'heure la plus
tardive se trouve avant la plus précoce, elle est mise en miroir. Après cette
étape, le temps croît toujours avec `x`.

**Échelle horaire** — les étiquettes `8:00`, `13:30`, … sont converties en
minutes, dédoublonnées (elles sont imprimées en haut et en bas de la grille), et
l'écart médian entre deux voisines donne la largeur d'une case. Chaque étiquette
étant centrée dans la demi-heure qu'elle ouvre, son centre est ramené au bord
gauche de sa case, puis une régression linéaire donne la conversion affine
`x → minutes` (`TimeScale`).

**Découpage vertical** — les étiquettes de jour (`LUNDI`, …) et de groupe (`G1`,
`G2`, …) sont relevées avec leur ordonnée. L'écart médian entre étiquettes de
groupe voisines donne le pas de grille `row_pitch` ; le sens dans lequel les
groupes se succèdent donne `reading_sign` (qui sert aussi à ordonner les lignes
de texte dans une case). Chaque ligne de groupe est bornée par les médiatrices
avec ses voisines, puis rattachée au jour dont l'étiquette est la plus proche.
Les bandes sont enfin remises dans l'ordre de la semaine.

### 4.3 Créneaux (`pdf/timetable.rs`)

Chaque rectangle coloré devient un cours.

- **Nature** — la couleur de fond est comparée à la palette du générateur
  (`model.rs`) : `#b3ffff` → `tp`, `#9fff9f` → `sae`, `#ffbab3` → `td`,
  `#ffff0c` → `amphi`, `#f23ea7` → `controle`. Un rectangle dont la couleur n'est
  pas dans la palette est ignoré (fonds, filets, en-têtes). Les cases peintes
  deux fois sont dédoublonnées sur (couleur, coordonnées arrondies).
- **Horaires** — les bords gauche et droit passent par `TimeScale`, puis sont
  arrondis au multiple de 5 minutes le plus proche : les bords sont
  géométriques, donc entachés d'une erreur de quelques secondes.
- **Jour et groupes** (`locate`) — les lignes de groupe recouvertes à au moins
  60 % sont retenues ; une case débordant sur deux jours revient à celui qu'elle
  couvre le plus. Si aucune ligne n'est suffisamment couverte, la case vise un
  demi-groupe : la moitié qu'elle occupe donne le suffixe `a` ou `b` (`G1a`),
  selon le sens de lecture.
- **Contenu** (`assign_texts`, `parse_block`) — un texte peut déborder sous sa
  case et retomber dans celle du dessous. Ce qui les départage est l'alignement :
  un bloc est calé sur le bord gauche de sa case, donc un texte revient à la case
  la plus à droite qui commence avant lui, la proximité verticale ne servant
  qu'à trancher les ex æquo (tolérance de 30 % du pas de grille).
  Les lignes sont ensuite interprétées : une petite case tient sur une ligne
  (`R1.10 - JP - R52`) ; une grande s'étale en code + intitulé, éventuellement
  poursuivi, puis enseignant (une ligne contenant une espace et finissant par un
  point) et salle (court jeton alphanumérique). Le générateur répète le code
  avant de l'introduire par un tiret (`R1.03 R1.03 - Architecture des`), ce que
  `split_head` absorbe.
- **Référentiel** — le code trouvé (`R1.01`, `S3.01A`, `R5A.07`, `R4.Real.08`)
  est cherché dans `data/resources.csv`, embarqué dans le binaire par
  `include_str!` et analysé une seule fois (`OnceLock`). L'intitulé officiel
  complète alors le champ `resource`.
- **Période** — la ligne de titre du document
  (`… Semaine 1 (36) : du 31/08/2026 au 05/09/2026 …`) donne le numéro de
  semaine ISO et les dates de début et de fin, ce qui permet de dater chaque
  créneau : `date = début + index du jour`.

Les créneaux sont enfin triés par date, puis heure de début, puis groupes.

## 5. Erreurs

Toutes les erreurs passent par `ApiError` et sont rendues en JSON
`{ "error": …, "message": … }`. La cause technique n'apparaît que dans les
journaux.

| Variante | Code | Statut |
| --- | --- | --- |
| `UnknownWeek` | `unknown_week` | 404 |
| `UnknownYear` | `unknown_year` | 404 |
| `UnknownRoute` | `unknown_route` | 404 |
| `BadRequest` | `bad_request` | 400 |
| `UpstreamUnavailable` | `upstream_unavailable` | 502 |
| `UpstreamUnexpected` | `upstream_unexpected` | 502 |
| `Unreadable` | `unreadable_timetable` | 422 |

## 6. Tests

- Tests unitaires dans les modules : palette et horaires (`model.rs`), index des
  semaines (`upstream.rs`), étiquettes de grille (`pdf/grid.rs`), blocs de texte
  et codes ressources (`pdf/timetable.rs`), encodage, métriques, filtres HTTP.
- Tests d'intégration : `tests/parsing.rs` analyse de vrais PDF
  (`tests/fixtures/A1_S3.pdf`, `A3_S2.pdf`) ; `tests/http.rs` exerce le routeur
  via `axum-test`. Les attentes ont été vérifiées contre les PDF que le
  département publie par demi-groupe, qui donnent la même semaine sous une autre
  forme.

```bash
cargo test
```
