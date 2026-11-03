# Composants

Toutes les classes ci-dessous sont dans `css/components.css` et utilisent uniquement les variables de `tokens.css`. Les styles propres à un seul écran (listes de salles, formulaire de devoir…) restent dans le `<style>` de la page correspondante dans `html/` ; pour la production, regroupe-les dans un fichier `screens.css` en préfixant leurs classes par écran (par ex. `.salles-rm`) car certains noms courts (`.gp`, `.crs`) existent dans plusieurs écrans avec un style légèrement différent.

## Coquille

| Classe | Rôle |
|---|---|
| `.app` | Grille à deux colonnes : menu 96 px + contenu |
| `.rail` | Menu vertical |
| `.logo` | Pastille « EDT » (accent) |
| `.ni` / `.ni.on` | Entrée de menu / entrée active (icône + libellé 11 px) |
| `.sp` | Espaceur : pousse Compte et Réglages en bas du menu |
| `.main` | Zone de contenu, marges 32/40 px |
| `.top` | En-tête de page : titre à gauche, contrôles à droite |
| `.row` | Ligne flex avec espacement 12 px |
| `.foot` | Pied de page discret (source des données, lien PDF) |

## Contrôles

```html
<button class="btn">Défaut</button>
<button class="btn pri">Principal</button>
<button class="btn icon" aria-label="Semaine suivante">…svg class="ic"…</button>
<button class="btn discord">Se connecter avec Discord</button>

<div class="seg"><button class="on">Grille</button><button>Liste</button></div>

<button class="opt on">BUT 3</button>            <!-- puces année / groupe / sous-groupe -->

<div class="wk"><b>Semaine 8</b><span>19 – 23 oct. · 41h30</span>…boutons…</div>

<label class="field">Date <input type="text"></label>
```

Hauteur minimale de tous les contrôles : 44 px.

## Tags

```html
<span class="tag t-sae">SAÉ</span>   <!-- t-cm, t-td, t-tp -->
<span class="tag t-room">111-2</span>
<span class="tag t-gray">R</span>    <!-- étage, neutre -->
<span class="tag y1">BUT 1</span>    <!-- y2, y3 -->
```

## Carte de cours

Deux éléments à combiner : la **couleur du type** (`k-sae`, `k-cm`, `k-td`, `k-tp`) et la **structure** (`.blk` pour la grille, `.crs` pour la liste, `.rm` pour les salles).

```html
<!-- Grille : positionnée en absolu dans .sched -->
<div class="blk k-cm" style="top: 422px; height: 80px">
  <div class="hd"><b>R5A.12</b><span class="tag t-cm">CM</span></div>
  <small>Modélisations mathématiques</small>
  <small class="m">15:30 – 17:00 · 205 · Crosio G.</small>
</div>
```

`k-*` impose le fond et la bordure avec `!important` : c'est volontaire, pour gagner sur le fond vert par défaut de `.blk`.

## Grille horaire

```html
<div style="display:grid; grid-template-columns:56px repeat(5, minmax(0,1fr))">
  <div></div>
  <div class="dh today"><span>Lun</span><b>19 oct.</b></div> …
  <div class="hrs"><div>08:00</div>…<div>19:00</div></div>
  <div class="sched"> …blocs .blk… </div> …
</div>
```

`.sched` = 672 px (12 h × 56 px) avec une ligne par heure ; `.hrs` = les libellés d'heures alignés dessus.

## Surfaces et états

| Classe | Rôle |
|---|---|
| `.card` | Surface arrondie (18 px) avec bordure |
| `.status` + `.dot` / `.dot.live` | Bandeau d'état (ex. « Journée terminée ») |
| `.empty` | État vide : bordure en pointillés, titre `h3`, texte atténué |
| `.lbl` | Libellé de section en capitales |
| `.hl` | Mise en avant accent (« demain à 08:30 ») |
| `a.lnk` | Lien souligné dans le texte |

## Icônes

Icônes SVG « trait » de 22 px, `stroke: currentColor`, tracé 1,7 px (classe `.ic` ou dans `.ni`). Les icônes utilisées sont dans le HTML des maquettes ; elles sont dessinées à la main pour la maquette : remplace-les par la bibliothèque d'icônes de ton choix si tu en utilises déjà une, en gardant la taille et l'épaisseur de trait.
