# EDTUnilim

Emplois du temps du département informatique de l'IUT du Limousin, lus dans les
PDF publiés par le département et servis en JSON, avec une interface web.

| Dossier | Rôle |
| --- | --- |
| [`APIRust/`](APIRust) | l'API : téléchargement des PDF, extraction des créneaux, JSON ([fonctionnement](APIRust/FONCTIONNEMENT.md)) |
| [`frontend/`](frontend) | l'interface SolidJS : semaine par colonnes de jours, en grille ou en liste |
| [`api-edt-unilim/`](api-edt-unilim) | la version NestJS précédente |

**Couverture des tests (API Rust) :** <!-- coverage:start -->![Couverture des tests](https://img.shields.io/badge/couverture_API-91.3%25-brightgreen)<!-- coverage:end -->
mesurée par `cargo llvm-cov`, mise à jour automatiquement à chaque push sur `master`.

## Tout lancer avec Docker

```bash
docker compose up -d --build
```

L'interface est alors sur <http://localhost:8080> ; l'API n'est pas exposée sur
l'hôte, nginx lui relaie les requêtes `/api/`. Les réglages se font par
variables d'environnement — copier `.env.example` en `.env` pour les changer :

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `EDT_PORT` | `8080` | port de l'interface sur l'hôte |
| `EDT_UPSTREAM` | `https://edt-iut-info.unilim.fr/edt` | racine des PDF publiés |
| `EDT_REFRESH_SECONDS` | `900` | périodicité de la revalidation |
| `EDT_UPSTREAM_TIMEOUT_SECONDS` | `15` | délai accordé au serveur amont |
| `EDT_PUBLIC_URL` | `http://localhost:8080` | adresse publique du site, base de l'URL de retour OAuth |
| `EDT_DISCORD_CLIENT_ID`, `EDT_DISCORD_CLIENT_SECRET` | — | application Discord (OAuth2) |
| `EDT_DISCORD_BOT_TOKEN` | — | bot qui envoie les messages privés |
| `EDT_DISCORD_GUILD_ID` | — | serveur où les personnes connectées sont ajoutées |
| `EDT_SESSION_SECRET` | — | clé de signature des sessions (32 caractères au moins) |
| `EDT_STATE_DIR` | `state` | comptes et classes suivies (`users.json`) |
| `EDT_LOG` | `info` | filtre de journalisation de l'API |

Les PDF téléchargés vivent dans le volume `edt-cache` : après un redémarrage,
le service répond immédiatement, et continue de répondre même si le serveur du
département est indisponible.

```bash
docker compose logs -f api      # journaux de l'API
docker compose down             # arrêt (le cache est conservé)
docker compose down -v          # arrêt et suppression du cache
```

## Développement, sans Docker

```bash
cd APIRust && cargo run          # API sur http://localhost:3000
cd frontend && npm install && npm run dev   # interface sur http://localhost:5173
```

Le serveur de développement Vite proxyfie `/api` vers `localhost:3000`.
