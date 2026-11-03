# Intégration — remplacer l'ancien design par la Proposition 1

> **Limite importante :** je n'ai vu que le rendu de l'ancien site, pas son code. Ce guide décrit une méthode et des correspondances ; remplace les noms de fichiers et de fonctions par ceux de ton projet. Les points marqués **(à vérifier)** sont des hypothèses.

## 0. Principe

L'ancien design a une barre rouge avec des icônes en haut. Le nouveau a un **menu vertical à gauche** (`.rail`) et un contenu à droite (`.main`) sur fond sombre. On **garde ta logique** (chargement des cours, semaine courante, classe mémorisée, Discord, export .ics…) et on **remplace uniquement le HTML et le CSS** qui l'affichent.

## 1. Préparer

1. Crée une branche : `git checkout -b refonte-design`.
2. Garde l'ancien CSS dans un dossier `old/` le temps de la migration. Ne le supprime qu'à l'étape 9.
3. Copie `css/tokens.css` et `css/components.css` dans le dossier de styles du site.

## 2. Charger CSS et polices

Dans le `<head>` de la page unique (ou de chaque page), dans cet ordre :

```html
<meta name="theme-color" content="#FF5252">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,500;600;700&family=Instrument+Sans:wght@400;500;600;700&display=swap">
<link rel="stylesheet" href="/css/tokens.css">
<link rel="stylesheet" href="/css/components.css">
```

- Pour éviter d'appeler Google (RGPD, performance), télécharge les deux polices en `woff2` et déclare-les avec `@font-face` ; les noms à garder sont `Bricolage Grotesque` et `Instrument Sans`.
- `theme-color` change de `#ff4b4b` à `#FF5252` (le nouvel accent). Il ne peut pas utiliser une variable CSS : si tu rends la couleur personnalisable, mets-le à jour en JavaScript (§ 6).

## 3. Remplacer la coquille

Remplace l'en-tête rouge et sa barre d'icônes par :

```html
<div class="app">
  <nav class="rail" aria-label="Navigation">
    <div class="logo">EDT</div>
    <a class="ni on" href="#/semaine">…icône…Semaine</a>
    <a class="ni" href="#/comparer">…Comparer</a>
    <a class="ni" href="#/enseignants">…Enseignants</a>
    <a class="ni" href="#/salles">…Salles</a>
    <a class="ni" href="#/analyse">…Analyse</a>
    <a class="ni" href="#/devoirs">…Devoirs</a>
    <div class="sp"></div>
    <a class="ni" href="#/compte">…Compte</a>
    <button class="ni" id="open-settings">…Réglages</button>
  </nav>
  <main class="main"><!-- contenu de l'écran courant --></main>
</div>
```

Les icônes SVG sont dans les fichiers de `html/` (copie-les depuis `semaine-grille.html`). La classe `on` marque l'entrée active : ta logique de routage doit la déplacer.

## 4. Correspondance ancien → nouveau

| Ancien bouton (infobulle) | Nouvel écran | Fichier maquette |
|---|---|---|
| (sélection au premier lancement) | Accueil / choix de classe | `accueil.html` |
| Emploi du temps + « Grille horaire » | Semaine — grille | `semaine-grille.html` |
| Emploi du temps + « Liste par jour » | Semaine — liste | `semaine-liste.html` |
| Exporter la semaine (.ics) | Bouton « Exporter .ics » de l'en-tête | `semaine-*.html` |
| Comparer deux classes | Comparer | `comparer.html` |
| Emploi du temps d'un enseignant | Enseignants | `enseignants.html` |
| Salles libres | Salles | `salles.html` |
| Analyser des PDF | Analyse | `analyse.html` |
| Devoirs à faire | Devoirs | `devoirs.html` |
| Compte Discord | Compte | `compte.html` |
| Réglages | Modale Réglages | `reglages.html` |

Routes constatées sur l'ancien site : `#compte`, `#salles`, `#comparer`, `#enseignants`. Les routes de *Analyse* et *Devoirs* sont **(à vérifier)**. Si ton routeur utilise déjà ces hash, garde-les : seuls les `href` du menu changent.

## 5. Écran par écran

Pour chaque écran : ouvre sa maquette dans `html/`, copie le HTML situé dans `<div class="app">…`, puis remplace les textes d'exemple par tes données.

1. **Semaine (grille et liste)** — voir § 5.1 ci-dessous.
2. **Comparer, Enseignants** — même logique de blocs que la grille (§ 5.1), avec deux colonnes (Comparer) ou cinq (Enseignants). Dans Comparer, ajoute la classe `both` aux cours identiques dans les deux colonnes et affiche un `<div class="free">` sur les créneaux libres pour les deux classes.
3. **Salles** — une carte `.rm` par salle, regroupées par étage. Libre : carte verte par défaut. Occupée : ajoute `k-cm`, `k-td`, `k-tp` ou `k-sae` selon le type du cours.
4. **Analyse PDF, Devoirs, Compte, Accueil** — formulaires et listes simples : reprends le HTML tel quel, rattache tes gestionnaires d'événements aux boutons.
5. **Réglages** — la maquette montre la modale posée sur la page. En production, enveloppe-la dans un `<dialog>` (ou ton composant modal) et garde la même structure interne ; le fond assombri utilise `var(--edt-scrim)`.

### 5.1 Afficher les cours dans la grille

Échelle fixe : **56 px par heure, la grille commence à 08:00** (`.sched` fait 672 px = 12 h, jusqu'à 20:00). Position d'un cours :

```
top    = (début − 08:00) en heures × 56 + 2   px
height = durée en heures × 56 − 4              px
```

Exemple : 13:30–15:30 → `top: 310px; height: 108px`.

Type de cours → classes :

| Type | Classe de la carte | Classe du tag |
|---|---|---|
| SAÉ | `k-sae` | `t-sae` |
| CM | `k-cm` | `t-cm` |
| TD | `k-td` | `t-td` |
| TP | `k-tp` | `t-tp` |

Extrait JavaScript prêt à adapter (construit le DOM sans `innerHTML`, donc sans risque d'injection venant des données) :

```js
const HOUR_PX = 56, START_MIN = 8 * 60;
const TYPE = { 'SAÉ': 'sae', 'CM': 'cm', 'TD': 'td', 'TP': 'tp' };
const toMin = (hhmm) => { const [h, m] = hhmm.split(':').map(Number); return h * 60 + m; };

function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}

// course = { code, type, start:'08:00', end:'10:00', subject, room, who }
function courseBlock(c) {
  const t = TYPE[c.type] ?? 'sae';
  const b = el('div', `blk k-${t}`);
  b.style.top = `${(toMin(c.start) - START_MIN) / 60 * HOUR_PX + 2}px`;
  b.style.height = `${(toMin(c.end) - toMin(c.start)) / 60 * HOUR_PX - 4}px`;

  const hd = el('div', 'hd');
  hd.append(el('b', null, c.code), el('span', `tag t-${t}`, c.type));
  b.append(hd);
  if (c.subject) b.append(el('small', null, c.subject));
  b.append(el('small', 'm', `${c.start} – ${c.end} · ${c.room} · ${c.who}`));
  return b;
}
// puis : colonneDuJour.append(...cours.map(courseBlock));
```

Cours d'une heure ou moins (ex. 15:30–16:30, `height: 52px`) : n'affiche que le titre, le tag et la ligne `.m`, sans la matière.

**Vue liste** : une carte `.day` par jour, avec une `.crs` par cours (`k-*` selon le type) ; entre deux cours, ajoute `<div class="gp"><span>1h30 de vide</span><i></i></div>` quand l'écart est positif. Le calcul des « de vide » existe déjà dans l'ancien site : réutilise-le.

**Aujourd'hui** : ajoute `today` sur l'en-tête du jour (`.dh.today`) et garde le bouton « Aujourd'hui » qui ramène à la semaine courante.

## 6. Réglages et personnalisation

| Réglage de l'ancien site | Comment le brancher |
|---|---|
| Affichage de la semaine (grille / liste) | Les deux cartes de la modale ; ta préférence stockée comme aujourd'hui |
| Apparence (Système / Clair / Sombre) | `document.documentElement.dataset.theme = 'light'` ou `removeAttribute('data-theme')` ; « Système » : `matchMedia('(prefers-color-scheme: light)')` |
| Couleur | `document.documentElement.style.setProperty('--edt-accent', valeur)` — `--edt-accent-soft` suit automatiquement (calculé avec `color-mix`). Mets aussi à jour `<meta name="theme-color">` |
| Année, groupe, sous-groupe | Les mêmes valeurs mémorisées que maintenant (stockage local) ; boutons `.opt` avec la classe `on` pour la sélection |
| Calendrier (S'abonner / Copier l'adresse) | Inchangé : seuls les boutons changent de style (`btn pri`, `btn`) |

Si l'utilisateur choisit une couleur d'accent claire, `--edt-on-accent` (texte sur l'accent, `#1A0708`) reste lisible ; avec un accent très foncé, passe-le à `#FFFFFF`. Vérifie le contraste (≥ 4,5:1) dans la fonction qui applique la couleur.

## 7. Responsive (à coder, non fourni)

Les maquettes sont à largeur fixe. Règles recommandées pour `components.css` :

```css
@media (max-width: 900px) {
  .app { grid-template-columns: 1fr; padding-bottom: 76px; }
  .rail { position: fixed; inset: auto 0 0 0; flex-direction: row; justify-content: space-around;
          border-right: 0; border-top: 1px solid var(--edt-line); background: var(--edt-surface-1);
          padding: 6px 8px 14px; z-index: 10; }
  .logo, .sp { display: none; }
  .main { padding: 20px 16px; }
  .app h1 { font-size: 28px; }
}
```

Sur mobile, la maquette `mobile-semaine.html` montre la cible : un jour à la fois (sélecteur Lun–Ven), cours en liste, barre d'onglets en bas (Semaine, Salles, Devoirs, Plus). Les écrans les moins utilisés (Comparer, Enseignants, Analyse, Compte) passent dans l'entrée « Plus ». La grille horaire sur petit écran : défilement horizontal dans un conteneur, ou bascule automatique en liste.

## 8. Accessibilité à finir

- Ajoute un style `:focus-visible` (non présent dans les maquettes), par exemple `outline: 2px solid var(--edt-accent); outline-offset: 2px;`.
- Les boutons qui n'ont qu'une icône ont déjà un `aria-label` ; garde-le.
- Les couleurs de texte respectent 4,5:1 sur leurs fonds (thème sombre). Ne les remplace pas par des valeurs plus pâles sans re-vérifier.
- Le type de cours n'est jamais indiqué par la couleur seule : le libellé (SAÉ, CM, TD, TP) est toujours écrit.
- Respecte `prefers-reduced-motion` si tu ajoutes des transitions.

## 9. Nettoyage

Quand tous les écrans sont migrés :

1. Supprime l'ancien CSS (`old/`) et les anciennes icônes devenues inutiles.
2. Cherche les couleurs en dur restantes : `grep -rnE "#[0-9a-fA-F]{3,8}|rgba?\(" src/` — il ne doit en rester que dans `tokens.css`.
3. Mets à jour le manifeste / la balise `theme-color` et le titre « EDT · IUT info Limoges » si tu le souhaites.

## 10. Liste de vérification

- [ ] Les 10 entrées du menu mènent au bon écran et l'entrée active est surlignée.
- [ ] Une semaine à 5 cours/jour (ex. semaine 8) s'affiche sans chevauchement dans la grille.
- [ ] Un cours se terminant après 19:30 reste visible (échelle jusqu'à 20:00).
- [ ] Les quatre types de cours ont leur couleur dans la grille, la liste, Salles et Comparer.
- [ ] Les états vides (aucun cours, aucun enseignant choisi, aucun devoir, aucun PDF) sont stylés avec `.empty`.
- [ ] Changer la couleur dans les réglages met à jour boutons, onglet actif et `theme-color`.
- [ ] Mode clair et mode sombre testés sur tous les écrans.
- [ ] Mobile 390 px : aucun défilement horizontal de la page.
- [ ] Navigation clavier : chaque bouton et lien est atteignable et visible au focus.
