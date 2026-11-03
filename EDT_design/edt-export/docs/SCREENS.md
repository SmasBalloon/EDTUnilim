# Fiches des écrans

Pour chaque écran : fichier de maquette, rôle, données à fournir, états à prévoir. Les maquettes montrent un état « rempli » ; les états vides sont à styler avec `.empty`.

---

## 1 · Accueil / choix de classe — `accueil.html`, `mobile-accueil.html`
Affiché au premier lancement ou quand aucune classe n'est mémorisée. Deux colonnes : accroche à gauche, formulaire à droite (une seule colonne sur mobile).
- **Données :** liste des années (BUT 1 à 3), des groupes principaux et des sous-groupes de l'année choisie.
- **Comportement :** le choix est mémorisé sur l'appareil ; « Voir mon emploi du temps » mène à la Semaine. Les sous-groupes et groupes dépendent de l'année sélectionnée.
- **Aperçu « demain » :** montre le prochain cours dès qu'une classe est connue ; sinon masque la carte.
- **Liens secondaires :** connexion Discord, analyse de PDF (accessibles sans choisir de classe).

## 2 · Semaine — grille — `semaine-grille.html`
Cours posés sur une échelle horaire 08:00 → 20:00, 5 colonnes (lun–ven).
- **Données :** pour chaque cours : code, type (SAÉ/CM/TD/TP), début, fin, matière (facultative), salle, enseignant ou groupe.
- **En-tête :** titre « Bonsoir » + classe, bascule Grille/Liste, export .ics, semaine (précédente/suivante, « Aujourd'hui », numéro, dates, total d'heures).
- **Bandeau :** nombre de cours de la semaine + légende des quatre types.
- **États :** semaine sans cours (`.empty`), cours après 19:30 (échelle prévue jusqu'à 20:00), cours d'une heure (carte compacte), journée en cours (en-tête `today`).
- **Pied :** « N cours · lus dans le PDF du département » + lien « PDF d'origine ».

## 3 · Semaine — liste — `semaine-liste.html`, `mobile-semaine.html`
Même contenu, présenté jour par jour : un cours par carte, dans l'ordre, avec les « de vide » entre les cours. Bureau : 5 colonnes côte à côte. Mobile : un jour à la fois, sélecteur Lun–Ven, séparé en matin / après-midi.
- **Calcul des vides :** écart entre la fin d'un cours et le début du suivant ; affiché s'il est positif.
- **Résumé de jour :** nombre de cours et durée totale.

## 4 · Comparer deux classes — `comparer.html`
Deux colonnes (classe de gauche = ta classe), un jour à la fois.
- **Données :** cours des deux classes pour la semaine ; classe de droite choisie via une fenêtre (année → groupe → sous-groupe).
- **Marquages :** contour rouge (`both`) sur les cours identiques dans les deux colonnes ; bande verte `.free` sur les créneaux où ni l'une ni l'autre n'a cours ; bouton d'inversion des deux classes.
- **État vide :** tant qu'aucune seconde classe n'est choisie, texte « Choisis une seconde classe pour la mettre en regard de la tienne ».

## 5 · Enseignants — `enseignants.html`
Semaine d'un enseignant, les trois années réunies.
- **Données :** liste d'enseignants avec leur nombre de cours cette semaine (fenêtre de recherche), puis leurs cours avec année et groupe.
- **État vide :** « Choisis un enseignant pour voir sa semaine, les trois années réunies. »
- **Détail affiché par carte :** code, type, horaire, salle, année · groupe.

## 6 · Salles — `salles.html`
Salles libres ou occupées sur une date et une plage horaire.
- **Données :** toutes les salles relevées dans les emplois du temps de toutes les années pour ce jour.
- **Contrôles :** date, heure de début, heure de fin, bouton « Maintenant » ; bascule « Libres / occupées » ↔ « Planning » (le planning n'est pas maquetté).
- **Sections :** « Libres sur toute la plage » (avec « libre jusqu'à HH:MM » ou « libre jusqu'au soir »), « Occupées sur la plage » (horaire, type, code du cours, groupes). Regroupement par étage : R (rez-de-chaussée), 100, 200, Autres (hors étages).
- **État vide :** « Aucun emploi du temps publié pour le [jour] ».

## 7 · Analyse de PDF — `analyse.html`
Dépôt de un à trois PDF d'une même semaine pour repérer les salles réservées deux fois.
- **Comportement :** rien n'est conservé ; bouton « Analyser » désactivé tant qu'aucun fichier n'est choisi.
- **Résultat :** une ligne par conflit : salle, jour, créneau, et les deux cours en conflit (année, groupe, horaire).
- **Mention à conserver :** un logiciel peut se tromper, un conflit non détecté n'est pas impossible.
- **Données de la maquette :** les champs `[Salle]`, `[Matière]` sont des emplacements à remplir.

## 8 · Devoirs — `devoirs.html`
Devoirs de l'utilisateur ou de sa classe. Liés au compte Discord.
- **Non connecté :** message « Les devoirs sont liés à ton compte. Connecte-toi avec Discord pour les noter. » (état d'origine du site).
- **Connecté :** liste « À faire » regroupée par date d'échéance, case à cocher, étiquette « perso » ou type du cours ; section « Terminé » atténuée ; formulaire « Noter un devoir » à droite (matière, date, description, portée : ma classe / moi).
- **Données de la maquette :** les intitulés `[Intitulé du devoir]` sont des emplacements.

## 9 · Compte Discord — `compte.html`
Présentation de ce que fait la connexion (envoi de chaque nouvelle semaine avec son .ics, alertes de changement de cours), mention sur les données conservées (identifiant, pseudo, avatar), bouton « Se connecter avec Discord ».
- **À prévoir :** état connecté (pseudo, avatar, classe, bouton de déconnexion) — non maquetté.

## 10 · Réglages — `reglages.html`
Modale : affichage de la semaine, apparence (Système / Clair / Sombre), couleur d'accent, année, groupe, sous-groupe, abonnement au calendrier (S'abonner, Copier l'adresse) et texte d'aide pour Google Agenda.
- **À prévoir :** fermeture par la touche Échap, piégeage du focus dans la modale, retour du focus au bouton Réglages.
