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
- **Mobile** : la semaine se feuillette, un jour par page. La page suit le
  doigt au pixel près, résiste aux deux bouts de la semaine et part vers le
  jour que le geste visait, à la vitesse où il a été lâché. Le sélecteur de
  jour au-dessus fait la même chose, en un toucher.
- **Navigation** par semaine (flèches, `←` / `→` au clavier), bouton
  *Aujourd'hui*, semaine courante déduite des périodes publiées.
- **Ce qui a changé** : la semaine affichée est comparée à la version vue la
  fois précédente (gardée dans le navigateur). Un bandeau annonce ce qui a
  bougé, les cases concernées portent une pastille, et la liste détaille chaque
  écart — « salle : 205 → 208 ». Les définitions sont celles de l'API pour ses
  messages Discord (`src/notify.rs`), pour qu'un changement se lise pareil des
  deux côtés.
- **Enseignants** (`#enseignants`) : la semaine d'un enseignant, les trois
  années réunies dans une même grille, avec la classe sur chaque case. Son nom,
  dans le détail d'un cours, y mène directement. L'API rattache « LD » à
  « Dubreuil L. » (voir `data/teachers.csv` côté API), sans quoi la moitié de
  ses cours manquerait à l'appel ; les écritures qu'elle ne nomme pas encore
  apparaissent telles quelles dans le sélecteur.
- **Comparer** (`#comparer`) : deux classes côte à côte, colonne contre
  colonne, sur la même échelle horaire. Chaque côté se choisit librement — son
  année et son groupe —, donc un BUT 1 se compare à un BUT 2 ; chacun est lu
  séparément, une semaine non publiée d'un côté ne prive pas de l'autre. Les
  cours que les deux suivent ensemble (même créneau, même ressource) sont
  cerclés, et les moments où personne n'a cours traversent les deux colonnes :
  c'est la réponse à « quand est-ce qu'on est libres tous les deux ? ».
- **Bandeau** du cours en cours ou du prochain cours, calculé sur la semaine
  courante même quand on consulte une autre semaine.
- **Détail** : une case cliquée ouvre une feuille (intitulé complet,
  enseignant, salle, groupes, durée, type). Au doigt elle monte du bas et se
  renvoie d'un geste vers le bas ; à la souris elle se pose au centre.
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
| `src/lib/motion.ts` | ressorts, projection de l'élan, résistance aux bords |
| `src/lib/changes.ts` | comparaison d'une semaine avec la version déjà vue |
| `src/lib/media.ts` | requêtes média lues comme des signaux |
| `src/components/WeekView.tsx` | la semaine, en grille ou en liste, feuilletée au doigt |
| `src/components/Pager.tsx` | les pages côte à côte que le doigt fait défiler |
| `src/components/Sheet.tsx` | le plateau des boîtes de dialogue, tirable |
| `src/components/WeekBar.tsx` | la semaine affichée, et de quoi en changer |
| `src/components/ComparePage.tsx` | le choix des deux classes à comparer |
| `src/components/TeachersPage.tsx` | la semaine d'un enseignant, toutes années |
| `src/components/ChangesBar.tsx` | l'annonce des changements, et leur détail |
| `src/components/CompareGrid.tsx` | les deux emplois du temps, colonne contre colonne |
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

Le mouvement ne passe pas par des animations CSS dès qu'un doigt peut s'en
saisir : un `@keyframes` a une durée écrite d'avance et ne se rattrape pas en
pleine course. Ce qui se touche est mené par les ressorts de `src/lib/motion.ts`,
qui repartent toujours de la position affichée et reprennent la vitesse du
geste. Restent en CSS les changements d'état — l'appui, le survol, la sélection
—, donnés au toucher plutôt qu'au relâchement. `prefers-reduced-motion` coupe
les ressorts (l'objet se pose directement), `prefers-reduced-transparency`
rend les matières opaques et `prefers-contrast` durcit les bords.

Deux requêtes par semaine affichée : la semaine filtrée sur le groupe
(`?group=G1a`, ce qui est affiché) et la semaine complète, qui fournit les jours
occupés et la liste des groupes du sélecteur. La comparaison en fait donc
quatre, deux par classe — toutes mémorisées, donc revenir sur une semaine déjà
lue ne coûte rien.
