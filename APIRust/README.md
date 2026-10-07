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
| `GET /api/v1/years/{year}/calendar/{group}.ics` | abonnement iCalendar (webcal) d'un demi-groupe, toutes semaines publiées |
| `GET /api/v1/resources` | référentiel des ressources et SAÉ |
| `GET /api/v1/teachers` | enseignants connus, et leurs autres écritures |
| `GET/POST /api/v1/homework` | devoirs visibles, et nouveau devoir |
| `PUT/DELETE /api/v1/homework/{id}` | corriger, effacer (son auteur seul) |
| `POST /api/v1/homework/{id}/confirm` | confirmer un devoir de classe |
| `PUT /api/v1/homework/{id}/done` | cocher « fait », pour soi |
| `POST /api/v1/discord/interactions` | commandes slash et boutons du bot |

L'emploi du temps accepte deux filtres cumulables :

- `?group=G1a` — un groupe ou un demi-groupe. Demander `G1a` rend aussi les
  cours du groupe entier, et demander `G1` rend aussi ses TP en demi-groupe.
- `?day=lundi` — un jour de la semaine.

```bash
curl 'http://localhost:3000/api/v1/years/1/weeks/3?group=G1a&day=lundi'
```

L'abonnement `…/calendar/G1a.ics` (l'extension est facultative) rassemble toutes
les semaines publiées de l'année en un seul calendrier `text/calendar`, que les
agendas relisent d'eux-mêmes (`REFRESH-INTERVAL` de 3 h) : c'est l'adresse à
ouvrir en `webcal://` ou à coller dans Google Agenda. Le site la propose dans ses
réglages. Une semaine illisible est omise plutôt que de faire échouer le tout ;
un groupe qui n'est pas un demi-groupe (`G1`) est refusé en `400`.

```bash
curl 'http://localhost:3000/api/v1/years/1/calendar/G1a.ics'
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
| `EDT_PUBLIC_URL` | `http://localhost:8080` | adresse publique du site, base de l'URL de retour OAuth |
| `EDT_DISCORD_CLIENT_ID`, `EDT_DISCORD_CLIENT_SECRET` | — | application Discord (OAuth2) |
| `EDT_DISCORD_BOT_TOKEN` | — | bot qui envoie les messages privés |
| `EDT_DISCORD_GUILD_ID` | — | serveur où les personnes connectées sont ajoutées |
| `EDT_DISCORD_KNOWN_GUILD_IDS` | — | serveurs où se trouve déjà le bot, séparés par des virgules : leurs membres ne sont pas ajoutés |
| `EDT_DISCORD_CONFLICT_RECIPIENTS` | — | comptes Discord prévenus des salles réservées par plusieurs cours en même temps, séparés par des virgules ; vide, rien n'est signalé |
| `EDT_SESSION_SECRET` | — | clé de signature des sessions (32 caractères au moins) |
| `EDT_STATE_DIR` | `state` | comptes et classes suivies (`users.json`) |
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

### Les enseignants

Le PDF nomme la même personne de deux façons : « Dubreuil L. » quand la case
est grande, « LD » quand elle est étroite. `data/teachers.csv` les réunit, et
l'API rend alors toujours le même `teacher` et le même `teacher_id` — c'est par
cet identifiant que se filtre l'emploi du temps d'un enseignant.

Le fichier reste court : les initiales « prénom puis nom » sont déduites du nom
(« Mérillou N. » → « NM »), et ne le sont plus dès que deux personnes se les
disputent. On n'y écrit donc que ce qui ne se devine pas — « BX » pour
Bombeaux C. —, et ce qui n'est pas quelqu'un : les libellés de groupe qui
tombent dans la case de l'enseignant sur certaines SAÉ.

Une écriture qu'il ne connaît pas n'est pas perdue, elle est rendue telle
quelle. Pour lister celles qui attendent un nom :

```bash
python3 scripts/enseignants-inconnus.py http://localhost:3000
# Nom ?;JP     —  49 créneaux : R1.10, R3.12, R5A.14 (BUT 1/BUT 2/BUT 3)
```

### Les devoirs

Un devoir appartient à quelqu'un — toutes ces routes demandent une session.

**Sa portée se choisit** : pour soi seul, pour son demi-groupe (le TP), pour
son groupe (le TD) ou pour toute l'année (l'amphi). On ne note un devoir que
pour sa propre classe : `G1a` peut viser `G1a`, `G1` ou l'année, jamais `G2`.

**Un devoir de classe engage les autres**, donc il n'est officiel qu'une fois
qu'une deuxième personne l'a confirmé — son auteur ne peut pas se confirmer
lui-même. En attendant, la classe le voit marqué `pending` : il faut bien que
quelqu'un puisse le lire pour l'accepter.

**« Fait » ne vaut que pour soi** : chacun coche sa propre liste, sur un devoir
que toute la classe partage.

Un devoir se rattache à un créneau (`course`), ou pas encore : quand la semaine
n'est pas publiée, on note d'abord la matière, et le rattachement se fait plus
tard depuis le cours. Les devoirs dont la date de rendu est passée depuis un
mois s'effacent d'eux-mêmes.

### Le bot

Le bot n'est pas un programme à part : **c'est ce service**. Il n'ouvre aucune
connexion permanente vers Discord, donc il apparaît *hors ligne* dans la liste
des membres — c'est normal, et ça ne l'empêche ni d'écrire ni de répondre. Il
appelle Discord en HTTP pour les messages privés ; et pour les commandes, c'est
Discord qui l'appelle, sur `POST /api/v1/discord/interactions`.

Quatre conditions, donc, pour voir les commandes apparaître :

1. `EDT_DISCORD_PUBLIC_KEY` renseignée — sans elle, les commandes ne sont même
   pas déclarées : une commande visible dont la réponse ne peut être ni reçue
   ni vérifiée vaut moins que pas de commande du tout. Le démarrage le dit.
2. L'« Interactions Endpoint URL » déclarée dans le portail Discord :
   `EDT_PUBLIC_URL` + `/api/v1/discord/interactions`. Discord la vérifie en la
   sondant, y compris avec de fausses signatures, que la route refuse par `401`.
3. Cette adresse **joignable depuis l'extérieur** : c'est Discord qui appelle.
   `http://localhost:8080` ne peut pas marcher — il faut un nom de domaine, ou
   un tunnel pour essayer.
4. L'application autorisée avec le champ `applications.commands`, en plus de
   `bot`, sur le serveur où on veut les taper.

Les commandes sont déclarées **globalement**, et non sur le serveur d'accueil :
c'est la seule façon qu'elles existent aussi dans le message privé du bot, là
où arrivent déjà les notifications. Discord met quelques minutes à les
propager la première fois.

| Commande | Effet |
| --- | --- |
| `/devoirs [matiere] [etat]` | liste ce qui est à faire, avec un bouton « fait » |
| `/devoir ajouter intitule:… [matiere] [pour] [portee] [details]` | note un devoir |

Les réponses sont éphémères : elles ne s'affichent que pour la personne qui a
tapé la commande. Les commandes se redéclarent à chaque démarrage du service,
si bien que le bot ne reste jamais avec une commande d'une version passée.

### La présence

Le bot apparaît **en ligne**, « Regarde les emplois du temps · 3 devoirs », et
le texte se rafraîchit toutes les dix minutes. C'est la seule chose qui demande
une connexion permanente : une présence ne s'envoie pas par une requête, Discord
ne la lit que sur la passerelle (`src/presence.rs`, d'où `tokio-tungstenite`).

Le bot n'y demande **aucune intention** (`intents: 0`) : il ne reçoit ni
messages ni événements de serveur, il se montre, c'est tout. La connexion se
rétablit seule après une coupure, en patientant de plus en plus longtemps ; un
refus d'authentification, lui, est dit une fois et la boucle s'arrête — ça ne
s'arrange pas en réessayant.

### Quand ça ne marche pas

Les journaux (`docker compose logs api`) nomment la cause :

| Message | Ce qu'il faut faire |
| --- | --- |
| `présence Discord impossible : le jeton du bot est refusé` (code 4004) | le jeton de `EDT_DISCORD_BOT_TOKEN` n'est plus valide : *Bot* → *Reset Token* dans le portail, puis recopier. Les messages privés sont hors service eux aussi. |
| `commandes Discord refusées … 401` | même cause : le jeton. |
| « EDTUnilim n'a pas répondu à temps » dans Discord | Discord abandonne au bout de trois secondes. Tout ce qui est long — prévenir une classe, deux requêtes par personne — part désormais de son côté, après la réponse ; si le message revient, c'est que le service est injoignable ou trop lent à démarrer. |
| `commandes Discord refusées … 403 Missing Access` | l'application n'a pas le champ `applications.commands` : réinviter le bot avec `scope=bot%20applications.commands`. |
| `commandes slash éteintes : EDT_DISCORD_PUBLIC_KEY est absente` | renseigner la clé publique de l'application. |
| rien du tout au démarrage | la configuration Discord est incomplète : c'est tout ou rien (voir plus bas). |

Les messages privés suivent la vie d'un devoir : proposition à confirmer (avec
le bouton qui va bien), confirmation, puis rappel la veille du rendu de ce qui
n'est pas coché — vers 19 h, heure de Paris.

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
