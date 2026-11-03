# EDT · IUT info Limoges — export de la refonte (Proposition 1)

Ce dossier contient le nouveau design de https://edt.smashballoon.fr/ : un fichier de couleurs, un fichier de composants, les 12 écrans en HTML autonome et la documentation pour remplacer l'ancien design.

## Contenu

```
edt-export/
├── README.md                        ← ce fichier
├── css/
│   ├── tokens.css                   ← TOUTES les couleurs, polices (+ thème clair de départ)
│   ├── components.css               ← composants partagés, sans aucune couleur en dur
│   └── tokens-alternatives.css      ← palettes des propositions 2 et 3 (référence)
├── html/
│   ├── index.html                   ← sommaire des maquettes
│   ├── accueil.html                 ← 1  choix de classe
│   ├── semaine-grille.html          ← 2  semaine, grille horaire
│   ├── semaine-liste.html           ← 3  semaine, liste par jour
│   ├── comparer.html                ← 4  comparer deux classes
│   ├── enseignants.html             ← 5  semaine d'un enseignant
│   ├── salles.html                  ← 6  salles libres / occupées
│   ├── analyse.html                 ← 7  analyse de PDF
│   ├── devoirs.html                 ← 8  devoirs
│   ├── compte.html                  ← 9  compte Discord
│   ├── reglages.html                ← 10 réglages (modale)
│   ├── mobile-accueil.html          ← 11 mobile, accueil
│   ├── mobile-semaine.html          ← 12 mobile, semaine
│   └── alternatives/                ← propositions 2 et 3 (autonomes, non intégrées)
└── docs/
    ├── INTEGRATION.md               ← comment remplacer l'ancien design
    ├── DESIGN-TOKENS.md             ← liste des couleurs, polices, tailles
    ├── COMPONENTS.md                ← classes CSS et extraits de HTML
    └── SCREENS.md                   ← fiche par écran : données, états, comportements
```

## Voir les maquettes

Ouvre `html/index.html` dans un navigateur : aucun serveur ni outil de build n'est nécessaire. Les pages s'appuient sur `css/` et sur Google Fonts (connexion internet requise pour les polices ; sans elles, le navigateur utilise une police système).

## À savoir avant de commencer

- **Les données des maquettes sont réelles pour la classe G7B, semaine 8 (19–23 oct. 2026)**, relevées sur le site actuel : cours, salles, enseignants. Les écrans *Analyse PDF* et *Devoirs* contiennent des champs `[Matière]`, `[Salle]`… car aucun exemple réel n'était disponible.
- **Les maquettes sont des pages statiques à largeur fixe** (1440 px bureau, 390 px mobile). L'adaptation responsive est décrite dans `docs/INTEGRATION.md`, elle n'est pas encore codée.
- **Je n'ai pas eu accès au code source de l'ancien site**, seulement à son rendu. `INTEGRATION.md` donne donc une méthode et des correspondances écran par écran ; les noms de fichiers et de fonctions de ton code sont à adapter.
- **Le thème clair** (`:root[data-theme="light"]` dans `tokens.css`) est un point de départ calculé pour le contraste, mais il n'a pas été maquetté : à vérifier visuellement.
