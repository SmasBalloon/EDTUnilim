# edt-unilim-front

Interface de consultation des emplois du temps du département informatique de
l'IUT du Limousin. Elle lit l'API JSON [`edt-unilim-api`](../APIRust) et affiche
la semaine **par colonnes de jours**, au choix en grille horaire ou en liste.

## Démarrer

```bash
npm install
npm run dev      # http://localhost:5173
```

L'API doit tourner à côté (`cargo run` dans `../APIRust`, port 3000) : en
développement Vite proxyfie `/api` vers elle. Pour viser une autre adresse :

```bash
EDT_API_TARGET=http://192.168.1.20:3000 npm run dev
```

## Construire

```bash
npm run build    # typecheck + bundle dans dist/
npm run preview
```

Le build est un site statique. En production, il est servi derrière la même
origine que l'API (les requêtes partent en `/api/v1/...`). Si le front est
hébergé ailleurs, indiquer l'API au build :

```bash
VITE_API_URL=https://edt.exemple.fr npm run build
```

## Ce que fait l'interface

- **Premier lancement** : choix de l'année (BUT 1/2/3) puis du groupe. Les
  groupes proposés sont ceux réellement présents dans l'emploi du temps — l'API
  ne publie pas de liste, ils se lisent dans les créneaux. Le choix est gardé
  sur l'appareil (`localStorage`).
- **Deux affichages**, réglables dans les paramètres, tous deux avec un jour par
  colonne :
  - *grille horaire* — échelle 8 h – 19 h 30, blocs proportionnels à la durée,
    couloirs quand deux créneaux se chevauchent, repère de l'heure courante ;
  - *liste par jour* — les cours à la suite, horaires écrits sur chaque carte.
- **Mobile** : les colonnes de la semaine restent toutes affichées, avec
  défilement horizontal ; l'en-tête des jours et la colonne des heures sont
  figées.
- **Navigation** par semaine (flèches, `←` / `→` au clavier), bouton
  *Aujourd'hui*, semaine courante déduite des périodes publiées.
- **Bandeau** du cours en cours ou du prochain cours, calculé sur la semaine
  courante même quand on consulte une autre semaine.
- **Détail** : une case cliquée s'agrandit sur place (intitulé complet,
  enseignant, salle, groupes, durée, type).
- **Export .ics** de la semaine affichée, et **PWA installable** qui garde les
  dernières semaines consultées hors connexion.
- Habillage sombre à accent rouge, dans l'esprit de
  [timetable.unilim.vexcited.com](https://timetable.unilim.vexcited.com/) :
  bandeau d'accueil, cartes arrondies, pastilles de type et de salle, trous de
  l'emploi du temps annoncés (« 1h30 de vide »). Thème clair également
  disponible. La pastille de salle porte le rouge de l'application, celle du
  type garde la couleur de sa nature (CM, TD, TP, SAÉ, contrôle).

## Organisation

| Fichier | Rôle |
| --- | --- |
| `src/App.tsx` | assemblage : en-tête, navigation, chargements, états vides |
| `src/lib/api.ts` | appels à l'API, types, mémorisation des réponses |
| `src/lib/time.ts` | dates, horaires, horloge partagée, semaine courante |
| `src/lib/groups.ts` | groupes et jours relevés dans l'emploi du temps |
| `src/lib/layout.ts` | amplitude de la grille, placement en couloirs |
| `src/lib/ics.ts` | export iCalendar |
| `src/lib/settings.ts` | réglages persistés et thème |
| `src/components/WeekView.tsx` | la semaine, en grille ou en liste |
| `src/components/CourseBlock.tsx` | une case de cours, repliée ou agrandie |
| `src/app.css` | toute la feuille de style : jetons, thèmes, base, mise en page, composants |
| `public/sw.js` | service worker (coquille + données hors ligne) |

## Style

Le style tient dans un seul fichier, `src/app.css`, découpé en cinq sections :
jetons (couleurs, rayons, ombres, tailles), thèmes clair et sombre — un simple
jeu de jetons réécrits —, base, mise en page, puis un bloc par composant. Les
composants ne portent que des classes sémantiques (`day-card`, `course-row`,
`badge badge--kind`) : aucune valeur arbitraire dans le balisage, aucune couleur
écrite ailleurs que dans les jetons.

Deux requêtes par semaine affichée : la semaine filtrée sur le groupe
(`?group=G1a`, ce qui est affiché) et la semaine complète, qui fournit les jours
occupés et la liste des groupes du sélecteur.
