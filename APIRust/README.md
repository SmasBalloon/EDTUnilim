# edt-unilim-api

API JSON des emplois du temps du département informatique de l'IUT du Limousin.

Le département publie un PDF par année et par semaine. Ce service les télécharge,
en extrait les créneaux et les sert en JSON, avec les dates réelles, les groupes
et demi-groupes concernés, et les intitulés du référentiel BUT.

## Démarrer

```bash
cargo run --release
# le service écoute sur http://localhost:3000
```

## Routes

| Route | Description |
| --- | --- |
| `GET /` | présentation des routes |
| `GET /health` | état du service |
| `GET /api/v1/years` | années publiées et semaines disponibles |
| `GET /api/v1/years/{year}/weeks` | semaines disponibles pour une année |
| `GET /api/v1/years/{year}/weeks/{week}` | emploi du temps d'une semaine |
| `GET /api/v1/resources` | référentiel des ressources et SAÉ |

L'emploi du temps accepte deux filtres cumulables :

- `?group=G1a` — un groupe ou un demi-groupe. Demander `G1a` rend aussi les
  cours du groupe entier, et demander `G1` rend aussi ses TP en demi-groupe.
- `?day=lundi` — un jour de la semaine.

```bash
curl 'http://localhost:3000/api/v1/years/1/weeks/3?group=G1a&day=lundi'
```

```json
{
  "year": 1,
  "week": 3,
  "period": {
    "week_number": 38,
    "start": "2026-09-14",
    "end": "2026-09-19",
    "label": "EMPLOI DU TEMPS - Semaine 3 (38) : du 14/09/2026 au 19/09/2026 …"
  },
  "source": {
    "url": "https://edt-iut-info.unilim.fr/edt/A1/A1_S3.pdf",
    "fetched_at": "2026-09-04T12:57:56Z"
  },
  "count": 4,
  "courses": [
    {
      "day": "LUNDI",
      "date": "2026-09-14",
      "start": "10:00",
      "end": "12:00",
      "duration_minutes": 120,
      "kind": "tp",
      "groups": ["G1a"],
      "resource": { "code": "R1.10", "name": "Anglais" },
      "teacher": "JP",
      "room": "R52"
    }
  ]
}
```

Les créneaux sont typés par `kind` : `amphi`, `td`, `tp`, `sae`, `controle`.

Toutes les erreurs sont renvoyées en JSON :

```json
{ "error": "unknown_week", "message": "aucun emploi du temps pour l'année 1, semaine 42" }
```

## Configuration

Tout est optionnel ; les valeurs par défaut suffisent en développement.

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `EDT_BIND` | `0.0.0.0:3000` | adresse d'écoute |
| `EDT_UPSTREAM` | `https://edt-iut-info.unilim.fr/edt` | racine des PDF publiés |
| `EDT_CACHE_DIR` | `cache` | cache disque des PDF |
| `EDT_REFRESH_SECONDS` | `900` | périodicité de la revalidation |
| `EDT_UPSTREAM_TIMEOUT_SECONDS` | `15` | délai accordé au serveur amont |
| `EDT_DISCORD_WEBHOOK` | — | webhook Discord : signale les créneaux modifiés, ajoutés ou retirés, et les semaines publiées |
| `EDT_LOG` | `info` | filtre de journalisation (`tracing`) |

## Fonctionnement

Le PDF ne contient aucune structure : le générateur dessine des rectangles de
couleur et pose du texte par-dessus. La lecture se fait en trois temps.

1. **Primitives** (`src/pdf/content.rs`) — un interpréteur de flux de contenu
   relève les aplats de couleur et les fragments de texte avec leurs
   coordonnées, en suivant les transformations et les XObjects.
2. **Grille** (`src/pdf/grid.rs`) — le repère est entièrement déduit des
   étiquettes imprimées : régression sur les heures pour l'échelle horaire,
   position des jours et des groupes pour le découpage vertical. Aucune
   constante de mise en page n'est codée en dur, car l'échelle change d'une
   semaine à l'autre.
3. **Créneaux** (`src/pdf/timetable.rs`) — chaque rectangle coloré devient un
   cours : sa couleur donne la nature, sa largeur les horaires, sa hauteur les
   groupes (une case de demi-hauteur vise un demi-groupe), et les textes qu'il
   contient la ressource, l'enseignant et la salle.

Les emplois du temps analysés restent en mémoire, les PDF téléchargés sur
disque — ce qui permet de continuer à répondre si le serveur du département est
indisponible. Une tâche de fond revalide périodiquement le cache par requête
conditionnelle (`If-None-Match`), sans retélécharger ce qui n'a pas changé.

[FONCTIONNEMENT.md](FONCTIONNEMENT.md) détaille chacune de ces étapes.

## Tests

```bash
cargo test
```

Les tests d'intégration analysent de vrais emplois du temps (`tests/fixtures`).
Leurs attentes ont été vérifiées contre les PDF que le département publie par
demi-groupe, qui donnent la même semaine sous une autre forme.
